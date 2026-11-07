//! 网易云 HTTP 请求、分页校验及游客会话管理。
use super::crypto::{decode, eapi, weapi};
use base64::{engine::general_purpose::STANDARD, Engine};
use md5::{Digest, Md5};
use serde_json::{json, Value};

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
pub(super) fn validate_id(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0 && value.bytes().all(|b| b.is_ascii_digit()))
        .ok_or_else(|| "无效的网易云 ID".into())
}

pub(super) fn page_offset(page: u32, limit: u32) -> Result<u64, String> {
    if page == 0 || limit == 0 || limit > 100 {
        return Err("无效的分页参数".into());
    }
    let offset = u64::from(page - 1) * u64::from(limit);
    if offset > u64::from(u32::MAX) {
        return Err("分页偏移过大".into());
    }
    Ok(offset)
}

pub(super) fn cookie_value<'a>(cookie: &'a str, name: &str) -> &'a str {
    cookie
        .split(';')
        .filter_map(|p| p.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
        .unwrap_or("")
}

pub(super) async fn request(
    path: &str,
    mut data: Value,
    cookie: &str,
    encrypted_api: bool,
) -> Result<(Value, String), String> {
    let mut cookies = cookie.to_string();
    let (form, url) = if encrypted_api {
        let mut header = json!({
            "os": "android",
            "appver": "8.10.05",
            "osver": "12",
            "versioncode": "140",
            "resolution": "1920x1080",
            "__csrf": cookie_value(cookie,"__csrf"),
            "requestId": format!("{}_{}", now_millis(), rand::random::<u16>())
        });
        for key in ["MUSIC_U", "MUSIC_A"] {
            let value = cookie_value(cookie, key);
            if !value.is_empty() {
                header[key] = json!(value);
            }
        }
        cookies = header
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or("")))
            .collect::<Vec<_>>()
            .join("; ");
        data["header"] = header;
        (
            vec![("params", eapi(path, &data))],
            format!(
                "https://interface.music.163.com/eapi{}",
                path.strip_prefix("/api").ok_or("网易云接口路径无效")?
            ),
        )
    } else {
        data["csrf_token"] = json!(cookie_value(cookie, "__csrf"));
        (
            weapi(&data),
            format!(
                "https://music.163.com/weapi{}",
                path.strip_prefix("/api").ok_or("网易云接口路径无效")?
            ),
        )
    };
    let response = super::super::CLIENT
        .post(url)
        .header("Referer", "https://music.163.com/")
        .header("Cookie", cookies)
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("网易云网络请求失败: {e}"))?
        .error_for_status()
        .map_err(|e| format!("网易云 HTTP 请求失败: {e}"))?;
    let set_cookie = response
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|v| v.split(';').next())
        .collect::<Vec<_>>()
        .join("; ");
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取网易云响应失败: {e}"))?;
    let value = decode(&bytes)?;
    let code = value["code"].as_i64().unwrap_or(200);
    if code != 200 && ![800, 801, 802, 803].contains(&code) {
        let message = value["message"]
            .as_str()
            .or(value["msg"].as_str())
            .unwrap_or("平台拒绝请求");
        return Err(format!("网易云请求失败 ({code}): {message}"));
    }
    Ok((value, set_cookie))
}

pub(super) async fn api(path: &str, data: Value, cookie: &str) -> Result<Value, String> {
    request(path, data, cookie, false).await.map(|(v, _)| v)
}

/// The reference client supplies MUSIC_A for guest audio requests. Keep this
/// anonymous session in memory, separately from the user's persisted MUSIC_U.
pub(super) async fn anonymous_cookie() -> Result<String, String> {
    static COOKIE: once_cell::sync::Lazy<tokio::sync::Mutex<Option<(String, std::time::Instant)>>> =
        once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(None));
    let mut cached = COOKIE.lock().await;
    if let Some((cookie, created)) = cached.as_ref() {
        if created.elapsed() < std::time::Duration::from_secs(3600) {
            return Ok(cookie.clone());
        }
    }
    let device = b"NMUSIC";
    let salt = b"3go8&$833h0k(2)2";
    let xor: Vec<_> = device
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ salt[i % salt.len()])
        .collect();
    let digest = STANDARD.encode(Md5::digest(xor));
    let username = STANDARD.encode(format!("NMUSIC {digest}"));
    let (_, cookie) = request(
        "/api/register/anonimous",
        json!({
            "username": username
        }),
        "os=iOS",
        false,
    )
    .await?;
    if cookie_value(&cookie, "MUSIC_A").is_empty() {
        return Err("网易云未返回游客会话，请登录后重试".into());
    }
    *cached = Some((cookie.clone(), std::time::Instant::now()));
    Ok(cookie)
}
