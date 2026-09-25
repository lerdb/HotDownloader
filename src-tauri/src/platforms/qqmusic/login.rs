//! QQ 登录的 Tauri 存储适配器。扫码协议、会话状态和刷新策略位于共享核心。

use std::sync::Arc;

use hotdownloader_core::qq_login::{self, LoginCredentialStore};
use serde_json::Value;
use tauri::AppHandle;

use crate::storage::store_wrapper;

pub use hotdownloader_core::qq_login::LoginCredentials;

struct TauriLoginStore {
    app: AppHandle,
}

impl TauriLoginStore {
    fn new(app: &AppHandle) -> Self {
        Self { app: app.clone() }
    }
}

impl LoginCredentialStore for TauriLoginStore {
    fn load(&self) -> Result<Value, String> {
        // 旧客户端把整份 settings JSON 存在 Tauri Store 的 settings 键中。
        // 缺少键时按空设置处理；JSON 损坏应明确报错，避免覆盖其他设置。
        let raw = store_wrapper::load_string(&self.app, "settings")
            .map_err(|error| format!("读取登录设置失败: {error}"))?;
        if raw.is_empty() {
            return Ok(serde_json::json!({}));
        }
        serde_json::from_str(&raw).map_err(|error| format!("解析登录设置失败: {error}"))
    }

    fn save(&self, settings: &Value) -> Result<(), String> {
        store_wrapper::save_string(&self.app, "settings", &settings.to_string())
            .map_err(|error| error.to_string())
    }
}

pub(crate) async fn get_login_credentials(app: &AppHandle) -> (Option<String>, Option<String>) {
    qq_login::get_login_credentials(&TauriLoginStore::new(app)).await
}

pub(crate) async fn check_credential_expired(app: &AppHandle) -> Result<bool, String> {
    qq_login::check_credential_expired(&TauriLoginStore::new(app)).await
}

pub(crate) async fn refresh_credential(app: &AppHandle) -> Result<LoginCredentials, String> {
    qq_login::refresh_credential(&TauriLoginStore::new(app)).await
}

pub(crate) async fn create_qr_login(app: AppHandle) -> Result<String, String> {
    // 后台 MQTT 会话持有存储接口；窗口关闭不会终止核心中的会话任务。
    qq_login::create_qr_login(Arc::new(TauriLoginStore { app })).await
}

pub(crate) async fn check_qr_login(qrcode_id: String) -> Result<String, String> {
    qq_login::check_qr_login(qrcode_id).await
}

pub(crate) async fn login_with_uin_authst(
    app: AppHandle,
    uin: String,
    authst: String,
    refresh_token: Option<String>,
    refresh_key: Option<String>,
    access_token: Option<String>,
    openid: Option<String>,
) -> Result<String, String> {
    qq_login::login_with_uin_authst(
        &TauriLoginStore { app },
        uin,
        authst,
        refresh_token,
        refresh_key,
        access_token,
        openid,
    )
    .await
}

pub(crate) async fn logout(app: AppHandle) -> Result<(), String> {
    qq_login::logout(&TauriLoginStore { app }).await
}

pub(crate) async fn get_login_status(app: AppHandle) -> Result<String, String> {
    qq_login::get_login_status(&TauriLoginStore { app }).await
}
