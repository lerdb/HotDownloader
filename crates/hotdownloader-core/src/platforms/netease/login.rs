//! Credentials stay in a runtime-owned file, never in settings/task records or API responses.
use super::client::{api, cookie_value, request};
use crate::platforms::qqmusic::login::{FileLoginStore, LoginCredentialStore};
use base64::{engine::general_purpose::STANDARD, Engine};
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

pub struct Store {
    path: PathBuf,
    file: FileLoginStore,
}
impl Store {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            file: FileLoginStore::new(&path),
            path,
        }
    }
    fn load(&self) -> Result<Value, String> {
        self.file.load().map_err(|e| e.replace("QQ", "网易云"))
    }
    fn save(&self, value: &Value) -> Result<(), String> {
        self.file.save(value).map_err(|e| e.replace("QQ", "网易云"))
    }
    pub fn cookie(&self) -> Result<String, String> {
        let value = self.load()?;
        let cookie = value["cookie"].as_str().unwrap_or("");
        if !cookie.is_empty() {
            validate_cookie(cookie)?;
        }
        Ok(cookie.to_string())
    }
}

#[derive(Deserialize)]
pub struct ManualCredentials {
    pub cookie: String,
}
#[derive(Clone)]
struct Session {
    path: PathBuf,
    generation: u64,
    key: String,
    cookie: String,
    created: Instant,
}
#[derive(Default)]
struct LoginState {
    generations: HashMap<PathBuf, u64>,
    sessions: HashMap<String, Session>,
}
static STATE: Lazy<Mutex<LoginState>> = Lazy::new(|| Mutex::new(LoginState::default()));

fn generation(state: &mut LoginState, path: &PathBuf) -> u64 {
    state
        .sessions
        .retain(|_, s| s.path != *path && s.created.elapsed() < Duration::from_secs(180));
    let value = state.generations.entry(path.clone()).or_default();
    *value += 1;
    *value
}
fn validate_cookie(cookie: &str) -> Result<(), String> {
    if cookie.len() > 16384
        || cookie_value(cookie, "MUSIC_U").is_empty()
        || cookie.bytes().any(|b| !(32..127).contains(&b))
    {
        return Err("请输入包含 MUSIC_U 的有效网易云 Cookie".into());
    }
    Ok(())
}
fn merge_cookie(left: &str, right: &str) -> String {
    let mut map = BTreeMap::new();
    for pair in left.split(';').chain(right.split(';')) {
        if let Some((key, value)) = pair.trim().split_once('=') {
            map.insert(key.to_string(), value.to_string());
        }
    }
    map.into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}
async fn account(cookie: &str) -> Result<Value, String> {
    let data = api("/api/w/nuser/account/get", json!({}), cookie).await?;
    let uid = data["profile"]["userId"]
        .as_u64()
        .filter(|v| *v > 0)
        .ok_or("网易云登录已失效，请重新登录")?;
    Ok(json!({
        "logged_in": true,
        "userid": uid.to_string(),
        "nickname": data["profile"]["nickname"].as_str().unwrap_or("")
    }))
}
fn qr_image(url: &str) -> Result<String, String> {
    let code = qrcode::QrCode::new(url.as_bytes()).map_err(|e| e.to_string())?;
    let width = code.width();
    let mut svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" shape-rendering=\"crispEdges\"><rect width=\"100%\" height=\"100%\" fill=\"white\"/><g fill=\"black\">",width+8,width+8);
    for (index, color) in code.to_colors().iter().enumerate() {
        if *color == qrcode::Color::Dark {
            svg.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"1\" height=\"1\"/>",
                index % width + 4,
                index / width + 4
            ));
        }
    }
    svg.push_str("</g></svg>");
    Ok(format!(
        "data:image/svg+xml;base64,{}",
        STANDARD.encode(svg)
    ))
}

pub async fn execute(
    store: &Store,
    action: &str,
    session_id: &str,
    credentials: Option<ManualCredentials>,
) -> Result<Value, String> {
    match action {
        "status" => {
            let cookie = store.cookie()?;
            if cookie.is_empty() {
                return Ok(json!({
                    "logged_in": false,
                    "userid": "",
                    "nickname": ""
                }));
            }
            account(&cookie).await
        }
        "logout" => {
            let mut state = STATE.lock().await;
            generation(&mut state, &store.path);
            store.save(&json!({}))?;
            Ok(json!({
                "logged_in": false,
                "userid": ""
            }))
        }
        "manual" | "refresh" => {
            let current = {
                let mut state = STATE.lock().await;
                generation(&mut state, &store.path)
            };
            let cookie = if action == "manual" {
                let value = credentials
                    .ok_or("请填写网易云 Cookie")?
                    .cookie
                    .trim()
                    .to_string();
                validate_cookie(&value)?;
                value
            } else {
                let cookie = store.cookie()?;
                validate_cookie(&cookie)?;
                let (_, set) =
                    request("/api/login/token/refresh", json!({}), &cookie, false).await?;
                merge_cookie(&cookie, &set)
            };
            let status = account(&cookie).await?;
            let state = STATE.lock().await;
            if state.generations.get(&store.path) != Some(&current) {
                return Err("网易云登录操作已被新的操作替代".into());
            }
            store.save(&json!({
                "cookie": cookie,
                "userid": status["userid"],
                "nickname": status["nickname"]
            }))?;
            Ok(status)
        }
        "qr" => {
            let current = {
                let mut state = STATE.lock().await;
                generation(&mut state, &store.path)
            };
            let (data, cookie) = request(
                "/api/login/qrcode/unikey",
                json!({
                    "type": 1
                }),
                "",
                false,
            )
            .await?;
            let key = data["unikey"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("网易云未返回二维码 key")?;
            let url = format!("https://music.163.com/login?codekey={key}");
            let image = qr_image(&url)?;
            let session_id = super::super::get_guid();
            let mut state = STATE.lock().await;
            if state.generations.get(&store.path) != Some(&current) {
                return Err("网易云二维码已被新的操作替代".into());
            }
            state.sessions.insert(
                session_id.clone(),
                Session {
                    path: store.path.clone(),
                    generation: current,
                    key: key.to_string(),
                    cookie,
                    created: Instant::now(),
                },
            );
            Ok(json!({
                "qrcode_id": session_id,
                "qr_image": image
            }))
        }
        "check" => {
            let session = {
                let state = STATE.lock().await;
                state
                    .sessions
                    .get(session_id)
                    .filter(|s| {
                        s.path == store.path && s.created.elapsed() < Duration::from_secs(180)
                    })
                    .cloned()
            };
            let Some(session) = session else {
                return Ok(json!({
                    "status": "expired"
                }));
            };
            let (data, set) = request(
                "/api/login/qrcode/client/login",
                json!({
                    "key": session.key,
                    "type": 1
                }),
                &session.cookie,
                false,
            )
            .await?;
            let code = data["code"].as_u64().unwrap_or(0);
            let cookie = merge_cookie(&session.cookie, &set);
            let status = if code == 803 {
                validate_cookie(&cookie)?;
                Some(account(&cookie).await?)
            } else {
                None
            };
            let mut state = STATE.lock().await;
            if state.generations.get(&store.path) != Some(&session.generation)
                || !state.sessions.contains_key(session_id)
            {
                return Ok(json!({
                    "status": "expired"
                }));
            }
            match code {
                803 => {
                    let mut status = status.unwrap();
                    store.save(&json!({
                        "cookie": cookie,
                        "userid": status["userid"],
                        "nickname": status["nickname"]
                    }))?;
                    generation(&mut state, &store.path);
                    status["status"] = json!("confirmed");
                    Ok(status)
                }
                800 => {
                    state.sessions.remove(session_id);
                    Ok(json!({
                        "status": "expired"
                    }))
                }
                801 | 802 => Ok(json!({
                    "status": if code==802 {"scanned"} else {"waiting"}
                })),
                _ => Err("网易云返回未知扫码状态".into()),
            }
        }
        _ => Err("未知网易云登录操作".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn logout_invalidates_sessions_and_persisted_credentials() {
        let path =
            std::env::temp_dir().join(format!("netease-test-{}.json", rand::random::<u64>()));
        let store = Store::new(&path);
        store
            .save(&json!({
                "cookie": "MUSIC_U=test"
            }))
            .unwrap();
        let mut state = STATE.lock().await;
        let current = generation(&mut state, &path);
        state.sessions.insert(
            "test-session".into(),
            Session {
                path: path.clone(),
                generation: current,
                key: "key".into(),
                cookie: String::new(),
                created: Instant::now(),
            },
        );
        drop(state);
        let result = execute(&store, "logout", "", None).await.unwrap();
        assert_eq!(result["logged_in"], false);
        assert_eq!(store.cookie().unwrap(), "");
        assert_eq!(
            execute(&store, "check", "test-session", None)
                .await
                .unwrap()["status"],
            "expired"
        );
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn cookies_preserve_equals_and_reject_header_injection() {
        assert_eq!(
            merge_cookie("MUSIC_U=old; __csrf=x", "MUSIC_U=new=="),
            "MUSIC_U=new==; __csrf=x"
        );
        assert!(validate_cookie("MUSIC_U=abc\r\nHeader: x").is_err());
        assert!(validate_cookie("__csrf=x").is_err());
    }
}
