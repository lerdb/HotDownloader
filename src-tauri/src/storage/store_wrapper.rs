use tauri::AppHandle;
use tauri_plugin_store::StoreExt;
use std::sync::Mutex;

static SETTINGS_WRITE_LOCK: Mutex<()> = Mutex::new(());

/// 从默认 data.json 存储加载字符串
pub fn load_string(app: &AppHandle, key: &str) -> Result<String, Box<dyn std::error::Error>> {
    let store = app.store("data.json")?;
    let value = store
        .get(key)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    Ok(value)
}

/// 保存字符串到默认 data.json 存储
pub fn save_string(
    app: &AppHandle,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let store = app.store("data.json")?;
    store.set(
        key.to_string(),
        serde_json::Value::String(value.to_string()),
    );
    store.save()?;
    Ok(())
}

/// 将 settings 的读取、转换和保存放在同一把锁下。设置命令与登录凭据刷新
/// 共用此入口，避免双方各自读取旧 JSON 后覆盖另一方刚写入的字段。
pub fn update_settings(
    app: &AppHandle,
    update: impl FnOnce(&str) -> Result<String, String>,
) -> Result<String, String> {
    let _guard = SETTINGS_WRITE_LOCK
        .lock()
        .map_err(|_| "设置写入锁已损坏".to_string())?;
    let store = app.store("data.json").map_err(|error| error.to_string())?;
    let previous = store
        .get("settings")
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    let next = update(&previous)?;
    if next != previous {
        store.set("settings".to_string(), serde_json::Value::String(next.clone()));
        store.save().map_err(|error| error.to_string())?;
    }
    Ok(next)
}
