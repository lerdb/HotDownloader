use crate::storage::store_wrapper;
use tauri::{command, AppHandle};

// 非 async 的 command 在主线程执行，而 store 的读写会整体序列化并写盘 data.json，
// 这里统一声明为 async，避免阻塞主线程造成界面卡死。
#[command]
pub async fn load_settings(app: AppHandle) -> Result<String, String> {
    store_wrapper::load_string(&app, "settings").map_err(|e| e.to_string())
}

#[command]
pub async fn save_settings(app: AppHandle, settings_json: String) -> Result<(), String> {
    let mut incoming: serde_json::Value =
        serde_json::from_str(&settings_json).map_err(|e| format!("无效设置数据: {e}"))?;
    let incoming_map = incoming.as_object_mut().ok_or("设置必须是 JSON 对象")?;
    let previous = store_wrapper::load_string(&app, "settings").map_err(|e| e.to_string())?;
    let previous: serde_json::Value = serde_json::from_str(&previous).unwrap_or_default();
    // 登录与自动刷新由 Rust 管理，普通设置保存不能用前端的旧凭据覆盖它们。
    // 先从提交值中移除，再从当前持久化值恢复，避免防抖写盘回退新令牌。
    for key in [
        "loginUin",
        "authst",
        "refreshToken",
        "refreshKey",
        "accessToken",
        "openid",
        "loginResponseData",
    ] {
        incoming_map.remove(key);
        if let Some(value) = previous.get(key) {
            incoming_map.insert(key.to_string(), value.clone());
        }
    }
    store_wrapper::save_string(&app, "settings", &incoming.to_string()).map_err(|e| e.to_string())
}
