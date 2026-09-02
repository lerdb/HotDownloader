//! QQ音乐扫码登录、uin+authst 手动登录、退出登录、登录状态查询模块。
//!
//! 该模块提供登录态管理功能，用于为下载链接等接口提供登录凭据，解锁更高音质或会员歌曲。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_tungstenite::tokio::{connect_async, ConnectStream};
use async_tungstenite::tungstenite::client::IntoClientRequest;
use async_tungstenite::tungstenite::Message;
use bytes::BytesMut;
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use rand::Rng;
use rumqttc::v5::mqttbytes::v5::{
    Connect, ConnectProperties, ConnectReturnCode, Filter, Packet, PingResp, Publish, Subscribe,
    SubscribeProperties, SubscribeReasonCode,
};
use rumqttc::v5::mqttbytes::QoS;
use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::sync::Mutex;

use crate::storage::store_wrapper;
use crate::utils::http::CLIENT;

/// MQTT 服务器主机名。
const MQTT_HOST: &str = "mu.y.qq.com";
/// MQTT WebSocket 端口。
const MQTT_PORT: u16 = 443;
/// MQTT WebSocket 握手路径。
const MQTT_PATH: &str = "/ws/handshake";
/// MQTT 心跳间隔（秒）。
const MQTT_KEEP_ALIVE: u16 = 45;
/// 建立 WebSocket 连接的超时时间。
const MQTT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// 等待 SUBACK 的超时时间。
const MQTT_SUBACK_TIMEOUT: Duration = Duration::from_secs(5);
/// 等待 MQTT 事件的超时时间。
const MQTT_EVENT_WAIT_TIMEOUT: Duration = Duration::from_millis(1500);
/// 连接失败后重试的间隔时间。
const MQTT_DEFAULT_INTERVAL: Duration = Duration::from_millis(1500);

/// 登录成功后保存的凭据信息。
#[derive(Debug, Clone)]
pub struct LoginCredentials {
    /// 用户 QQ 号。
    pub uin: String,
    /// 登录授权令牌（authst）。
    pub authst: String,
    /// 刷新令牌。
    pub refresh_token: String,
    /// 刷新密钥。
    pub refresh_key: String,
    /// 访问令牌。
    pub access_token: String,
    /// OpenID。
    pub openid: String,
}

/// MQTT 扫码登录过程中接收到的事件类型。
#[derive(Debug, Clone)]
enum MqttLoginEvent {
    /// 等待扫码。
    #[allow(dead_code)]
    WaitingScan,
    /// 已扫码，等待确认。
    WaitingConfirm,
    /// 二维码已过期。
    QrCodeExpired,
    /// 用户取消登录。
    Canceled,
    /// 登录失败。
    LoginFailed,
    /// 收到 Cookie 数据（包含音乐 ID 和音乐 Key）。
    Cookies {
        /// 音乐 ID（即 uin 数字形式）。
        music_id: u64,
        /// 音乐 Key（即 authst）。
        music_key: String,
    },
}

/// 单个二维码登录会话的当前状态。
struct LoginSessionState {
    /// 登录状态。
    status: LoginStatus,
}

/// 二维码登录的详细状态。
#[derive(Debug, Clone)]
enum LoginStatus {
    /// 等待扫码。
    WaitingScan,
    /// 已扫码，等待用户确认。
    WaitingConfirm,
    /// 登录成功，包含凭据。
    Confirmed(LoginCredentials),
    /// 二维码已过期。
    Expired,
    /// 用户取消。
    Canceled,
    /// 发生错误，携带错误信息。
    Error(String),
}

/// 共享的单个登录会话状态（可被多个任务并发访问）。
type SharedLoginSessionState = Arc<Mutex<LoginSessionState>>;
/// 二维码 ID 到共享会话状态的映射。
type LoginSessionMap = HashMap<String, SharedLoginSessionState>;
/// 全局登录会话表的共享映射。
type SharedLoginSessionMap = Arc<Mutex<LoginSessionMap>>;

/// 全局登录会话表，以 qrcode_id 为键。
static LOGIN_SESSIONS: Lazy<SharedLoginSessionMap> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

/// `music.login.LoginServer.Login` 接口返回的数据结构。
#[derive(serde::Deserialize)]
struct TLoginInfoData {
    /// 音乐 ID（数字 uin）。
    musicid: u64,
    /// 音乐 Key。
    musickey: String,
    /// 刷新令牌。
    refresh_token: String,
    /// 刷新密钥。
    refresh_key: String,
    /// 访问令牌（可能为空）。
    #[serde(default)]
    access_token: String,
    /// OpenID（可能为空）。
    #[serde(default)]
    openid: String,
    /// 字符串形式的音乐 ID（可能不存在）。
    #[serde(default)]
    str_musicid: Option<String>,
    /// Key 过期时间（秒），暂未使用。
    #[allow(dead_code)]
    #[serde(rename = "keyExpiresIn", default)]
    key_expires_in: i64,
    /// Key 创建时间戳（秒），暂未使用。
    #[allow(dead_code)]
    #[serde(rename = "musickeyCreateTime", default)]
    musickey_create_time: i64,
}

impl TLoginInfoData {
    /// 将接口返回的数据转换为 [`LoginCredentials`]。
    ///
    /// 优先使用 `str_musicid` 作为 uin，如果缺失则使用数字 `musicid` 转换。
    fn into_credentials(self) -> LoginCredentials {
        let uin = self
            .str_musicid
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.musicid.to_string());
        LoginCredentials {
            uin,
            authst: self.musickey,
            refresh_token: self.refresh_token,
            refresh_key: self.refresh_key,
            access_token: self.access_token,
            openid: self.openid,
        }
    }
}

/// 构造 QQ 音乐统一接口请求体。
///
/// # 参数
/// - `module`: 接口模块名。
/// - `method`: 接口方法名。
/// - `param`: 接口参数。
/// - `comm`: 公共参数（会合并到默认参数中）。
///
/// # 返回
/// 序列化为 JSON 值的请求体。
fn build_http_body(module: &str, method: &str, param: Value, comm: Value) -> Value {
    json!({
        "comm": comm,
        format!("{module}.{method}"): {
            "module": module,
            "method": method,
            "param": param,
        }
    })
}

/// 调用 QQ 音乐统一接口。
///
/// # 参数
/// - `module`: 接口模块名。
/// - `method`: 接口方法名。
/// - `param`: 接口参数。
/// - `comm_extra`: 额外的公共参数，会与默认参数合并。
///
/// # 返回
/// - `Ok(Value)`: 接口返回的 `data` 字段。
/// - `Err(String)`: 错误信息。
async fn login_api_call(
    module: &str,
    method: &str,
    param: Value,
    comm_extra: Value,
) -> Result<Value, String> {
    let req_key = format!("{module}.{method}");
    let mut comm = json!({
        "ct": "11",
        "cv": "13020508",
        "v": "13020508",
        "tmeAppID": "qqmusic",
        "format": "json",
        "inCharset": "utf-8",
        "outCharset": "utf-8",
    });
    if let Some(obj) = comm_extra.as_object() {
        for (k, v) in obj {
            comm[k] = v.clone();
        }
    }

    let body = build_http_body(module, method, param, comm);
    let resp = CLIENT
        .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
        .header("Referer", "https://y.qq.com/")
        .header("Origin", "https://y.qq.com")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;
    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    let req_data = data.get(&req_key).ok_or("响应缺少对应模块")?;
    if req_data["code"].as_i64().unwrap_or(-1) != 0 {
        log::warn!(
            "[登录] API 调用失败: module={}, method={}, code={}",
            module,
            method,
            req_data["code"]
        );
        return Err(format!("接口错误: code={}", req_data["code"]));
    }
    log::debug!("[登录] API 调用成功: module={}, method={}", module, method);
    Ok(req_data["data"].clone())
}

/// 从 Cookie 对象中提取指定键的值。
///
/// Cookie 值可能为字符串、数字或包含 `value` 字段的对象。
///
/// # 参数
/// - `cookies`: Cookie 对象。
/// - `key`: 要提取的键名。
///
/// # 返回
/// 提取到的字符串值，若不存在则返回 `None`。
fn extract_cookie_value(cookies: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    let value = cookies.get(key)?;
    if let Some(s) = value.as_str() {
        return Some(s.to_owned());
    }
    if let Some(n) = value.as_u64() {
        return Some(n.to_string());
    }
    if let Some(obj) = value.as_object() {
        if let Some(inner) = obj.get("value") {
            if let Some(s) = inner.as_str() {
                return Some(s.to_owned());
            }
            if let Some(n) = inner.as_u64() {
                return Some(n.to_string());
            }
        }
    }
    None
}

/// 解析 MQTT PUBLISH 报文中的登录事件。
///
/// # 参数
/// - `publish`: MQTT Publish 报文。
///
/// # 返回
/// 解析出的 [`MqttLoginEvent`]，若无法识别则返回 `None`。
fn parse_publish_event(publish: &Publish) -> Option<MqttLoginEvent> {
    let event_type = publish.properties.as_ref().and_then(|props| {
        props
            .user_properties
            .iter()
            .find_map(|(k, v)| (k == "type").then_some(v.as_str()))
    })?;

    match event_type {
        "scanned" => Some(MqttLoginEvent::WaitingConfirm),
        "canceled" => Some(MqttLoginEvent::Canceled),
        "timeout" => Some(MqttLoginEvent::QrCodeExpired),
        "loginFailed" => Some(MqttLoginEvent::LoginFailed),
        "cookies" => {
            let payload = publish.payload.as_ref();
            let data: Value = serde_json::from_slice(payload).ok()?;
            let cookies = data.get("cookies")?.as_object()?;
            let music_id = extract_cookie_value(cookies, "qqmusic_uin")?
                .parse::<u64>()
                .ok()?;
            let music_key = extract_cookie_value(cookies, "qqmusic_key")?;
            if music_key.is_empty() {
                Some(MqttLoginEvent::LoginFailed)
            } else {
                Some(MqttLoginEvent::Cookies {
                    music_id,
                    music_key,
                })
            }
        }
        _ => None,
    }
}

/// MQTT over WebSocket 会话封装。
///
/// 负责建立 WebSocket 连接、发送 MQTT 控制报文、处理重定向及事件监听。
struct MqttWebSocket {
    /// WebSocket 流。
    stream: async_tungstenite::WebSocketStream<ConnectStream>,
    /// 缓存的事件（例如在订阅阶段可能收到 PUBLISH）。
    pending_event: Option<MqttLoginEvent>,
    /// 当前 WebSocket 握手路径，重定向时会更新。
    path: String,
}

/// 根据 `server_reference` 生成重定向后的握手路径。
///
/// 如果当前路径最后一个段已经包含端口号，则替换为新的 `server_reference`；
/// 否则将 `server_reference` 追加到路径末尾。
///
/// # 参数
/// - `current_path`: 当前握手路径。
/// - `server_reference`: 服务器参考信息（通常为 `host:port`）。
///
/// # 返回
/// 新的握手路径。
fn build_redirect_path(current_path: &str, server_reference: &str) -> String {
    let parts: Vec<&str> = current_path.trim_end_matches('/').split('/').collect();
    if let Some(last) = parts.last() {
        if last.contains(':') {
            let mut parts = parts.clone();
            let len = parts.len();
            parts[len - 1] = server_reference;
            return parts.join("/");
        }
    }
    format!(
        "{}/{}",
        current_path.trim_end_matches('/'),
        server_reference
    )
}

impl MqttWebSocket {
    /// 建立到指定路径的 WebSocket 连接。
    ///
    /// # 参数
    /// - `path`: 握手路径（以 `/` 开头）。
    ///
    /// # 返回
    /// - `Ok(Self)`: 连接成功。
    /// - `Err(String)`: 错误信息。
    async fn connect(path: &str) -> Result<Self, String> {
        // 构造 wss URL 并设置必要的请求头。
        let url = format!("wss://{MQTT_HOST}:{MQTT_PORT}{path}");
        let mut request = url
            .into_client_request()
            .map_err(|e| format!("构建请求失败: {e}"))?;
        let headers = request.headers_mut();
        headers.insert(
            "Sec-WebSocket-Protocol",
            "mqtt".parse().expect("valid static header"),
        );
        headers.insert(
            "Origin",
            "https://y.qq.com".parse().expect("valid static header"),
        );
        headers.insert(
            "Referer",
            "https://y.qq.com/".parse().expect("valid static header"),
        );
        headers.insert(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36"
                .parse()
                .expect("valid static header"),
        );

        let (stream, _) = tokio::time::timeout(MQTT_CONNECT_TIMEOUT, connect_async(request))
            .await
            .map_err(|_| "WebSocket 连接超时".to_string())?
            .map_err(|e| format!("WebSocket 连接失败: {e}"))?;

        log::info!(
            "[登录] WebSocket 已连接: {}:{}{}",
            MQTT_HOST,
            MQTT_PORT,
            path
        );
        Ok(Self {
            stream,
            pending_event: None,
            path: path.to_string(),
        })
    }

    /// 发送 MQTT CONNECT 报文并处理可能的服务器重定向。
    ///
    /// # 参数
    /// - `qrcode_id`: 二维码 ID，用于连接认证。
    ///
    /// # 返回
    /// - `Ok(())`: MQTT 连接成功建立。
    /// - `Err(String)`: 错误信息。
    async fn connect_mqtt(&mut self, qrcode_id: &str) -> Result<(), String> {
        // 处理 MQTT 服务端重定向（ServerMoved / UseAnotherServer）
        let mut redirect_count = 0;
        let max_redirects = 3;
        let mut current_path = self.path.clone();

        loop {
            // 构造 CONNECT 报文的属性。
            let mut connect_props = ConnectProperties::new();
            connect_props.authentication_method = Some("pass".to_owned());
            connect_props.user_properties = vec![
                ("tmeAppID".to_owned(), "qqmusic".to_owned()),
                ("business".to_owned(), "management".to_owned()),
                ("hashTag".to_owned(), qrcode_id.to_owned()),
                ("clientTag".to_owned(), "management.user".to_owned()),
                ("userID".to_owned(), qrcode_id.to_owned()),
            ];

            // 发送 CONNECT 并等待 CONNACK。
            log::info!("[登录] 发送 MQTT CONNECT 报文，路径: {}", current_path);
            self.send_packet(Packet::Connect(
                Connect {
                    keep_alive: MQTT_KEEP_ALIVE,
                    client_id: build_client_id(),
                    clean_start: true,
                    properties: Some(connect_props),
                },
                None,
                None,
            ))
            .await?;

            let packet = self.next_packet(MQTT_CONNECT_TIMEOUT).await?;
            match packet {
                Some(Packet::ConnAck(connack)) => match connack.code {
                    // 若返回 Success 则完成。
                    ConnectReturnCode::Success => {
                        log::info!("[登录] 收到 CONNACK，连接成功");
                        self.path = current_path;
                        return Ok(());
                    }
                    // 若返回 ServerMoved 或 UseAnotherServer，则从 CONNACK 属性中提取 server_reference，并使用 build_redirect_path 更新路径，重新建立 WebSocket 并重试，最多重试 3 次
                    ConnectReturnCode::UseAnotherServer | ConnectReturnCode::ServerMoved => {
                        // 提取 server_reference。
                        let server_reference = connack
                            .properties
                            .as_ref()
                            .and_then(|props| props.server_reference.clone())
                            .filter(|s| !s.is_empty());

                        // 超过重定向次数或缺少 server_reference 时返回错误。
                        if let Some(server_ref) = server_reference {
                            if redirect_count >= max_redirects {
                                log::error!(
                                    "[登录] MQTT 重定向次数超过 {} 次，最后 server_reference: {}",
                                    max_redirects,
                                    server_ref
                                );
                                return Err(format!(
                                    "MQTT 重定向次数超过限制，最后节点: {}",
                                    server_ref
                                ));
                            }

                            // 更新路径并重新建立 WebSocket。
                            let new_path = build_redirect_path(&current_path, &server_ref);
                            log::warn!(
                                "[登录] 收到 MQTT 重定向，从 {} 重定向到 {}",
                                current_path,
                                new_path
                            );

                            // 关闭当前流并重新连接。
                            self.stream.close(None).await.ok();
                            let mut new_ws = MqttWebSocket::connect(&new_path).await?;
                            new_ws.pending_event = self.pending_event.take();
                            *self = new_ws;

                            current_path = new_path;
                            redirect_count += 1;
                            continue;
                        } else {
                            log::error!("[登录] CONNACK 返回 ServerMoved/UseAnotherServer，但缺少 server_reference");
                            return Err("MQTT 重定向缺少 server_reference".into());
                        }
                    }
                    code => {
                        log::error!("[登录] CONNACK 返回错误: {:?}", code);
                        return Err(format!("MQTT 连接被拒绝: {:?}", code));
                    }
                },
                _ => {
                    log::error!("[登录] 未收到 CONNACK");
                    return Err("未收到 CONNACK".into());
                }
            }
        }
    }

    /// 订阅扫码登录事件主题。
    ///
    /// # 参数
    /// - `qrcode_id`: 二维码 ID，用于确定主题。
    ///
    /// # 返回
    /// - `Ok(())`: 订阅成功。
    /// - `Err(String)`: 错误信息。
    async fn subscribe(&mut self, qrcode_id: &str) -> Result<(), String> {
        let topic = format!("management.qrcode_login/{qrcode_id}");
        let mut subscribe = Subscribe::new(
            Filter::new(topic.clone(), QoS::AtMostOnce),
            Some(SubscribeProperties {
                id: None,
                user_properties: vec![
                    ("authorization".to_owned(), "tmelogin".to_owned()),
                    ("pubsub".to_owned(), "unicast".to_owned()),
                ],
            }),
        );
        subscribe.pkid = 1;
        log::info!("[登录] 发送 MQTT SUBSCRIBE 报文，主题: {}", topic);
        self.send_packet(Packet::Subscribe(subscribe)).await?;

        loop {
            let packet = self.next_packet(MQTT_SUBACK_TIMEOUT).await?;
            match packet {
                Some(Packet::SubAck(suback)) if suback.pkid == 1 => {
                    if suback
                        .return_codes
                        .iter()
                        .any(|code| !matches!(code, SubscribeReasonCode::Success(_)))
                    {
                        log::error!("[登录] SUBACK 返回错误: {:?}", suback.return_codes);
                        return Err(format!("MQTT 订阅被拒绝: {:?}", suback.return_codes));
                    }
                    log::info!("[登录] 收到 SUBACK，订阅成功");
                    return Ok(());
                }
                Some(Packet::Publish(publish)) => {
                    // 订阅期间收到 PUBLISH，缓存事件。
                    log::info!("[登录] 订阅期间收到 PUBLISH 报文，已缓存事件");
                    if let Some(event) = parse_publish_event(&publish) {
                        self.pending_event = Some(event);
                    }
                }
                Some(_) => continue,
                None => {
                    log::error!("[登录] SUBACK 等待超时");
                    return Err("SUBACK 超时".into());
                }
            }
        }
    }

    /// 等待下一个登录事件。
    ///
    /// # 参数
    /// - `timeout`: 等待超时时间。
    ///
    /// # 返回
    /// - `Ok(Some(event))`: 收到事件。
    /// - `Ok(None)`: 超时。
    /// - `Err(String)`: 错误信息。
    async fn next_event(&mut self, timeout: Duration) -> Result<Option<MqttLoginEvent>, String> {
        if let Some(evt) = self.pending_event.take() {
            return Ok(Some(evt));
        }
        loop {
            let packet = self.next_packet(timeout).await?;
            match packet {
                Some(Packet::Publish(publish)) => {
                    if let Some(event) = parse_publish_event(&publish) {
                        log::info!("[登录] 收到 PUBLISH 事件: {:?}", event);
                        return Ok(Some(event));
                    }
                }
                Some(Packet::PingReq(_)) => {
                    log::debug!("[登录] 收到 PINGREQ，回复 PINGRESP");
                    self.send_packet(Packet::PingResp(PingResp)).await?;
                }
                Some(Packet::Disconnect(_)) => {
                    log::warn!("[登录] 收到 DISCONNECT 报文");
                    return Ok(Some(MqttLoginEvent::LoginFailed));
                }
                Some(_) => {}
                None => return Ok(None),
            }
        }
    }

    /// 从 WebSocket 流中读取下一个 MQTT 报文。
    ///
    /// # 参数
    /// - `timeout`: 等待超时时间。
    ///
    /// # 返回
    /// - `Ok(Some(packet))`: 收到 MQTT 报文。
    /// - `Ok(None)`: 超时或连接关闭。
    /// - `Err(String)`: 错误信息。
    async fn next_packet(&mut self, timeout: Duration) -> Result<Option<Packet>, String> {
        loop {
            let frame = match tokio::time::timeout(timeout, self.stream.next()).await {
                Ok(frame) => frame,
                Err(_) => return Ok(None),
            };
            match frame {
                Some(Ok(Message::Binary(payload))) => {
                    let mut bytes = BytesMut::from(payload.as_ref());
                    let packet = Packet::read(&mut bytes, None)
                        .map_err(|e| format!("解码 MQTT 包失败: {e}"))?;
                    return Ok(Some(packet));
                }
                Some(Ok(Message::Close(_))) => return Ok(None),
                Some(Ok(_)) => continue,
                Some(Err(e)) => return Err(format!("读取帧失败: {e}")),
                None => return Ok(None),
            }
        }
    }

    /// 发送 MQTT 报文。
    ///
    /// # 参数
    /// - `packet`: 要发送的 MQTT 报文。
    ///
    /// # 返回
    /// - `Ok(())`: 发送成功。
    /// - `Err(String)`: 错误信息。
    async fn send_packet(&mut self, packet: Packet) -> Result<(), String> {
        let mut bytes = BytesMut::new();
        packet
            .write(&mut bytes, None)
            .map_err(|e| format!("编码 MQTT 包失败: {e}"))?;
        self.stream
            .send(Message::Binary(bytes.freeze()))
            .await
            .map_err(|e| format!("发送 MQTT 包失败: {e}"))
    }
}

/// 生成随机的 MQTT 客户端 ID。
///
/// 格式为当前时间戳（毫秒）+ 4 位随机数。
fn build_client_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let random = rand::rng().random_range(1000..=9999);
    format!("{millis}{random}")
}

/// 将登录凭据保存到应用设置存储中。
///
/// # 参数
/// - `app`: Tauri 应用句柄。
/// - `creds`: 登录凭据。
///
/// # 返回
/// - `Ok(())`: 保存成功。
/// - `Err(String)`: 错误信息。
async fn save_credentials(app: &AppHandle, creds: &LoginCredentials) -> Result<(), String> {
    let settings_json = store_wrapper::load_string(app, "settings").unwrap_or_default();
    let mut settings: Value = serde_json::from_str(&settings_json).unwrap_or(json!({}));
    settings["loginUin"] = json!(creds.uin);
    settings["authst"] = json!(creds.authst);
    settings["refreshToken"] = json!(creds.refresh_token);
    settings["refreshKey"] = json!(creds.refresh_key);
    settings["accessToken"] = json!(creds.access_token);
    settings["openid"] = json!(creds.openid);
    store_wrapper::save_string(app, "settings", &settings.to_string()).map_err(|e| e.to_string())
}

/// 从应用设置中读取当前登录凭据的 uin 和 authst。
///
/// # 参数
/// - `app`: Tauri 应用句柄。
///
/// # 返回
/// 元组 `(uin, authst)`，若未登录则两者均为 `None`。
pub(crate) async fn get_login_credentials(app: &AppHandle) -> (Option<String>, Option<String>) {
    let settings_json = store_wrapper::load_string(app, "settings").unwrap_or_default();
    let settings: Value = serde_json::from_str(&settings_json).unwrap_or(json!({}));
    let uin = settings
        .get("loginUin")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    let authst = settings
        .get("authst")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());
    (uin, authst)
}

/// 创建二维码登录会话。
///
/// 该命令会生成二维码 ID 和 Base64 编码的二维码图片数据，
/// 并在后台启动 MQTT 监听任务，等待用户扫码登录。
///
/// # 返回
/// JSON 字符串，包含 `qrcode_id` 和 `qr_base64` 字段。
pub(crate) async fn create_qr_login(app: AppHandle) -> Result<String, String> {
    // 安装 rustls 默认 CryptoProvider，避免 TLS 后端不确定导致的 panic。
    let _ = rustls::crypto::ring::default_provider().install_default();

    let param = json!({
        "tmeAppID": "qqmusic",
        "ct": 11,
        "cv": 13020508,
    });
    let data = login_api_call("music.login.LoginServer", "CreateQRCode", param, json!({})).await?;
    let qrcode_id = data["qrcodeID"]
        .as_str()
        .ok_or("缺少 qrcodeID")?
        .to_string();
    let qr_raw = data["qrcode"].as_str().unwrap_or("");
    let qr_base64 = if qr_raw.contains(',') {
        qr_raw.split(',').nth(1).unwrap_or(qr_raw).to_string()
    } else {
        qr_raw.to_string()
    };

    log::info!("[登录] 二维码生成成功，qrcode_id = {}", qrcode_id);

    // 启动后台监听任务。
    let sessions = LOGIN_SESSIONS.clone();
    let qrcode_id_clone = qrcode_id.clone();
    tokio::spawn(async move {
        let session_state = Arc::new(Mutex::new(LoginSessionState {
            status: LoginStatus::WaitingScan,
        }));
        sessions
            .lock()
            .await
            .insert(qrcode_id_clone.clone(), session_state.clone());

        loop {
            // 建立 WebSocket 连接并订阅事件。
            log::info!("[登录] 开始建立 WebSocket 连接");
            let mut mqtt = match MqttWebSocket::connect(MQTT_PATH).await {
                Ok(m) => {
                    log::info!("[登录] WebSocket 连接成功");
                    m
                }
                Err(e) => {
                    log::error!("[登录] WebSocket 连接失败: {}", e);
                    session_state.lock().await.status = LoginStatus::Error(e);
                    break;
                }
            };
            if let Err(e) = mqtt.connect_mqtt(&qrcode_id_clone).await {
                log::error!("[登录] MQTT CONNECT 失败: {}", e);
                session_state.lock().await.status = LoginStatus::Error(e);
                break;
            }
            log::info!("[登录] MQTT CONNECT 成功，已发送 CONNECT 并收到 CONNACK");
            if let Err(e) = mqtt.subscribe(&qrcode_id_clone).await {
                log::error!("[登录] MQTT SUBSCRIBE 失败: {}", e);
                session_state.lock().await.status = LoginStatus::Error(e);
                break;
            }
            log::info!("[登录] MQTT SUBSCRIBE 成功，已订阅登录事件主题");

            // 循环监听事件。
            loop {
                match mqtt.next_event(MQTT_EVENT_WAIT_TIMEOUT).await {
                    Ok(Some(event)) => {
                        log::info!("[登录] 收到 MQTT 事件: {:?}", event);
                        let should_break = match &event {
                            MqttLoginEvent::WaitingScan => {
                                session_state.lock().await.status = LoginStatus::WaitingScan;
                                log::info!("[登录] 状态更新：等待扫码");
                                false
                            }
                            MqttLoginEvent::WaitingConfirm => {
                                session_state.lock().await.status = LoginStatus::WaitingConfirm;
                                log::info!("[登录] 状态更新：已扫码，等待确认");
                                false
                            }
                            MqttLoginEvent::Cookies {
                                music_id,
                                music_key,
                            } => {
                                log::info!(
                                    "[登录] 收到 Cookies 事件，music_id = {}, music_key = {}",
                                    music_id,
                                    music_key
                                );
                                // 调用 Login 接口换取正式凭据。
                                let login_param = json!({
                                    "musicid": music_id,
                                    "qrCodeID": qrcode_id_clone,
                                    "token": music_key,
                                });
                                log::info!("[登录] 开始调用 Login 接口换取正式凭据");
                                match login_api_call(
                                    "music.login.LoginServer",
                                    "Login",
                                    login_param,
                                    json!({"tmeLoginType": "6"}),
                                )
                                .await
                                {
                                    Ok(login_data) => {
                                        log::info!("[登录] Login 接口调用成功，开始解析凭据");
                                        let creds_result =
                                            serde_json::from_value::<TLoginInfoData>(login_data)
                                                .map(|data| data.into_credentials())
                                                .map_err(|e| format!("解析凭据失败: {e}"));
                                        match creds_result {
                                            Ok(creds) => {
                                                if let Err(e) = save_credentials(&app, &creds).await
                                                {
                                                    log::error!("[登录] 保存登录凭据失败: {}", e);
                                                    session_state.lock().await.status =
                                                        LoginStatus::Error("保存凭据失败".into());
                                                } else {
                                                    log::info!(
                                                        "[登录] 登录成功，凭据已保存，uin = {}",
                                                        creds.uin
                                                    );
                                                    session_state.lock().await.status =
                                                        LoginStatus::Confirmed(creds);
                                                }
                                            }
                                            Err(e) => {
                                                log::error!("[登录] 解析凭据失败: {}", e);
                                                session_state.lock().await.status =
                                                    LoginStatus::Error(e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("[登录] Login 接口调用失败: {}", e);
                                        session_state.lock().await.status = LoginStatus::Error(e);
                                    }
                                }
                                true
                            }
                            MqttLoginEvent::QrCodeExpired => {
                                session_state.lock().await.status = LoginStatus::Expired;
                                log::info!("[登录] 状态更新：二维码已过期");
                                true
                            }
                            MqttLoginEvent::Canceled => {
                                session_state.lock().await.status = LoginStatus::Canceled;
                                log::info!("[登录] 状态更新：用户取消登录");
                                true
                            }
                            MqttLoginEvent::LoginFailed => {
                                session_state.lock().await.status =
                                    LoginStatus::Error("登录失败".into());
                                log::error!("[登录] 状态更新：登录失败");
                                true
                            }
                        };
                        if should_break {
                            break;
                        }
                    }
                    Ok(None) => {
                        // 超时，继续监听。
                        log::debug!("[登录] 等待事件超时，继续监听");
                    }
                    Err(e) => {
                        log::error!("[登录] 事件循环异常: {}", e);
                        session_state.lock().await.status = LoginStatus::Error(e);
                        break;
                    }
                }
            }

            // 检查会话状态，若已结束则退出循环。
            let status = session_state.lock().await.status.clone();
            if matches!(
                status,
                LoginStatus::Confirmed(_)
                    | LoginStatus::Expired
                    | LoginStatus::Canceled
                    | LoginStatus::Error(_)
            ) {
                break;
            }
            // 短暂等待后重新连接。
            tokio::time::sleep(MQTT_DEFAULT_INTERVAL).await;
        }
    });

    Ok(json!({
        "qrcode_id": qrcode_id,
        "qr_base64": qr_base64,
    })
    .to_string())
}

/// 查询指定二维码的登录状态。
///
/// # 参数
/// - `qrcode_id`: 二维码 ID。
///
/// # 返回
/// JSON 字符串，包含 `status` 字段，可能的值有：
/// `waiting`、`scanned`、`confirmed`、`expired`、`canceled`、`error`。
pub(crate) async fn check_qr_login(qrcode_id: String) -> Result<String, String> {
    let sessions = LOGIN_SESSIONS.lock().await;
    let state = sessions.get(&qrcode_id).ok_or("二维码会话不存在或已结束")?;
    let state = state.lock().await;
    let result = match &state.status {
        LoginStatus::WaitingScan => json!({"status": "waiting"}),
        LoginStatus::WaitingConfirm => json!({"status": "scanned"}),
        LoginStatus::Confirmed(creds) => json!({
            "status": "confirmed",
            "credentials": {
                "uin": creds.uin,
                "authst": creds.authst,
                "refreshToken": creds.refresh_token,
                "refreshKey": creds.refresh_key,
                "accessToken": creds.access_token,
                "openid": creds.openid,
            }
        }),
        LoginStatus::Expired => json!({"status": "expired"}),
        LoginStatus::Canceled => json!({"status": "canceled"}),
        LoginStatus::Error(e) => json!({"status": "error", "message": e}),
    };
    log::info!("[登录] 查询登录状态: {:?}", state.status);
    Ok(result.to_string())
}

/// 使用 uin 和 authst 手动登录。
///
/// 若只提供 uin 和 authst，则直接保存作为临时凭据；
/// 若还提供了刷新令牌等额外字段，则调用接口进行刷新获取完整凭据。
///
/// # 参数
/// - `app`: Tauri 应用句柄。
/// - `uin`: QQ 号（字符串形式，但刷新时需要可解析为数字）。
/// - `authst`: 登录授权令牌。
/// - `refresh_token`: 可选刷新令牌。
/// - `refresh_key`: 可选刷新密钥。
/// - `access_token`: 可选访问令牌。
/// - `openid`: 可选 OpenID。
///
/// # 返回
/// JSON 字符串，包含保存的完整凭据信息。
pub(crate) async fn login_with_uin_authst(
    app: AppHandle,
    uin: String,
    authst: String,
    refresh_token: Option<String>,
    refresh_key: Option<String>,
    access_token: Option<String>,
    openid: Option<String>,
) -> Result<String, String> {
    let refresh_token = refresh_token.unwrap_or_default();
    let refresh_key = refresh_key.unwrap_or_default();
    let access_token = access_token.unwrap_or_default();
    let openid = openid.unwrap_or_default();

    let has_refresh_fields = !refresh_token.is_empty()
        || !refresh_key.is_empty()
        || !access_token.is_empty()
        || !openid.is_empty();

    if !has_refresh_fields {
        // 直接保存临时凭据，刷新字段留空。
        let creds = LoginCredentials {
            uin: uin.clone(),
            authst: authst.clone(),
            refresh_token: String::new(),
            refresh_key: String::new(),
            access_token: String::new(),
            openid: String::new(),
        };
        save_credentials(&app, &creds).await?;
        log::info!("[登录] 手动登录（仅 uin/authst）成功，已保存 uin = {}", uin);
        return Ok(json!({
            "uin": creds.uin,
            "authst": creds.authst,
            "refreshToken": creds.refresh_token,
            "refreshKey": creds.refresh_key,
            "accessToken": creds.access_token,
            "openid": creds.openid,
        })
        .to_string());
    }

    // 包含刷新字段，调用 Login 接口刷新。
    let music_id = uin.parse::<u64>().map_err(|_| "uin 必须为数字")?;
    let param = json!({
        "musicid": music_id,
        "musickey": authst,
        "refresh_key": refresh_key,
        "refresh_token": refresh_token,
        "access_token": access_token,
        "openid": openid,
        "str_musicid": uin,
        "loginMode": 2,
        "expired_in": 0,
    });
    log::info!("[登录] 手动登录（含刷新字段），开始调用 Login 接口刷新");
    let data = login_api_call(
        "music.login.LoginServer",
        "Login",
        param,
        json!({"tmeLoginType": "6"}),
    )
    .await?;
    let creds: LoginCredentials = serde_json::from_value::<TLoginInfoData>(data)
        .map(|d| d.into_credentials())
        .map_err(|e| format!("解析凭据失败: {e}"))?;
    save_credentials(&app, &creds).await?;
    log::info!("[登录] 手动登录（含刷新）成功，已保存 uin = {}", creds.uin);
    Ok(json!({
        "uin": creds.uin,
        "authst": creds.authst,
        "refreshToken": creds.refresh_token,
        "refreshKey": creds.refresh_key,
        "accessToken": creds.access_token,
        "openid": creds.openid,
    })
    .to_string())
}

/// 退出登录，清除所有登录相关设置。
///
/// # 参数
/// - `app`: Tauri 应用句柄。
///
/// # 返回
/// - `Ok(())`: 清除成功。
/// - `Err(String)`: 错误信息。
pub(crate) async fn logout(app: AppHandle) -> Result<(), String> {
    let settings_json = store_wrapper::load_string(&app, "settings").unwrap_or_default();
    let mut settings: Value = serde_json::from_str(&settings_json).unwrap_or(json!({}));
    if let Some(obj) = settings.as_object_mut() {
        for key in [
            "loginUin",
            "authst",
            "refreshToken",
            "refreshKey",
            "accessToken",
            "openid",
        ] {
            obj.remove(key);
        }
    }
    store_wrapper::save_string(&app, "settings", &settings.to_string()).map_err(|e| e.to_string())
}

/// 查询当前登录状态。
///
/// # 返回
/// JSON 字符串，包含 `logged_in` 布尔值和 `uin` 字符串。
pub(crate) async fn get_login_status(app: AppHandle) -> Result<String, String> {
    let settings_json = store_wrapper::load_string(&app, "settings").unwrap_or_default();
    let settings: Value = serde_json::from_str(&settings_json).unwrap_or(json!({}));
    let uin = settings
        .get("loginUin")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let logged_in = !uin.is_empty();
    Ok(json!({
        "logged_in": logged_in,
        "uin": uin,
    })
    .to_string())
}
