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
    store_wrapper::save_string(&app, "settings", &settings_json).map_err(|e| e.to_string())
}
