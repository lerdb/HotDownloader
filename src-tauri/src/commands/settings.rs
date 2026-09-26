//! Tauri 设置命令。与独立服务共用字段验证和冲突判定，存储仍使用现有 data.json。

use hotdownloader_core::settings_patch::{
    apply_patch, snapshot, SettingsPatch, SettingsPatchError, SettingsScope, SettingsSnapshot,
};
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{command, AppHandle, Emitter, Manager};

use crate::download::engine::DownloadEngine;
use crate::storage::store_wrapper;

// 覆盖完整的写盘、调度器更新和事件广播，保持同一进程内的修订顺序。
static PATCH_COMMAND_LOCK: Mutex<()> = Mutex::new(());

fn read_stored(raw: &str) -> Result<Value, String> {
    if raw.is_empty() {
        Ok(json!({}))
    } else {
        serde_json::from_str(raw).map_err(|error| format!("解析设置失败: {error}"))
    }
}

/// 冲突包含服务端当前快照，前端才能让用户逐字段选择保留哪一个值。
fn patch_error(error: SettingsPatchError) -> String {
    match error {
        SettingsPatchError::Invalid(message) => message,
        SettingsPatchError::Conflict { fields, snapshot } => json!({
            "error": "设置已被其他页面更新",
            "fields": fields,
            "snapshot": snapshot,
        })
        .to_string(),
    }
}

// store 的写盘是同步操作；命令以 async 暴露，避免在界面主线程执行。
#[command]
pub async fn get_settings_snapshot(app: AppHandle) -> Result<SettingsSnapshot, String> {
    let raw = store_wrapper::load_string(&app, "settings").map_err(|error| error.to_string())?;
    Ok(snapshot(&read_stored(&raw)?))
}

#[command]
pub async fn patch_settings(
    app: AppHandle,
    patch: SettingsPatch,
) -> Result<SettingsSnapshot, String> {
    let _guard = PATCH_COMMAND_LOCK
        .lock()
        .map_err(|_| "设置命令锁已损坏".to_string())?;
    let mut committed: Option<SettingsSnapshot> = None;
    let mut concurrency_changed = false;
    store_wrapper::update_settings(&app, |previous| {
        let current = read_stored(previous)?;
        let applied = apply_patch(&current, patch, SettingsScope::Tauri).map_err(patch_error)?;
        if applied.changed_fields.iter().any(|field| field == "maxConcurrent") {
            concurrency_changed = true;
        }
        if !applied.changed_fields.is_empty() {
            committed = Some(applied.snapshot.clone());
        }
        Ok(applied.stored.to_string())
    })?;

    if concurrency_changed {
        // 两个命令可能在写盘后以相反顺序恢复执行，因此以当前持久化值更新调度器。
        let latest = store_wrapper::load_string(&app, "settings")
            .map_err(|error| error.to_string())?;
        let max = read_stored(&latest)?["maxConcurrent"]
            .as_u64()
            .unwrap_or(3)
            .clamp(1, 32) as u32;
        app.state::<DownloadEngine>().set_concurrency(max);
    }
    if let Some(saved) = committed {
        // 其他窗口的设置页收到同一份快照；当前窗口也可用它确认落盘结果。
        let _ = app.emit("settings-updated", &saved);
        Ok(saved)
    } else {
        let raw = store_wrapper::load_string(&app, "settings")
            .map_err(|error| error.to_string())?;
        Ok(snapshot(&read_stored(&raw)?))
    }
}
