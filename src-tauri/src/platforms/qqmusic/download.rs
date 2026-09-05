//! 下载链接与解密密钥获取模块。
//!
//! 该模块提供统一的下载链接获取接口，替代旧的加密/非加密分离接口。
//! 核心函数 [`get_download_link`] 会根据文件扩展名判断是否需要解密密钥，
//! 并返回完整的下载 URL 和对应的密钥（非加密文件密钥为空）。
//! 同时提供 Tauri 命令 [`fetch_download_link`] 供前端调用。

use serde_json::{json, Value};
use std::path::Path;
use tauri::Emitter;

use super::login::get_login_credentials;
use crate::utils::guid::get_guid;
use crate::utils::http::CLIENT;

/// 统一获取下载链接与解密密钥（新接口：`vkey.GetVkeyServer.CgiGetVkey`）。
///
/// 替换旧的加密/非加密分离接口，统一使用一个接口获取所有品质的下载链接。
/// 登录态通过 `comm` 和 `param` 中的 `uin` 字段传递，未登录时为空字符串。
/// 响应中的 `sip` 数组为优先 CDN 列表，若不为空则使用第一个作为下载 URL 前缀，
/// 否则使用默认 CDN `https://wx.music.tc.qq.com/`。
/// `purl` 为不带 CDN 的相对路径，`ekey` 为解密密钥（可能为空）。
/// 最终返回值中，`ekey` 是否生效由调用方根据文件后缀决定，本函数原样返回响应中的 `ekey`。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-source/blob/55eb9881dad6ca895505352f3a0a7d1dfa3444e0/src/apis/tx.js#L42>
///
/// # 参数
/// - `song_mid`: 歌曲的唯一标识（mid）。
/// - `filename`: 品质文件名（例如 `M800001abc.mp3`），决定下载的具体文件。
/// - `uin`: 可选用户 QQ 号，用于登录态传递。
/// - `authst`: 可选登录授权令牌，用于登录态传递。
///
/// # 返回
/// - `Ok((String, String))`：元组 `(完整下载链接, 解密密钥)`。
///   解密密钥 `ekey` 可能为空，其是否生效由调用方根据文件后缀决定。
/// - `Err(String)`：错误信息，包括网络错误、接口错误、文件不可下载等。
async fn fetch_vkey_link(
    song_mid: &str,
    filename: &str,
    uin: Option<&str>,
    authst: Option<&str>,
) -> Result<(String, String), String> {
    // 构造请求体，comm 和 param 中携带登录态，guid 动态生成
    let request_body = json!({
        "loginUin": uin.unwrap_or(""),
        "comm": {
            "format": "json",
            "ct": 24,
            "cv": 0,
            "tmeAppID": "qqmusic",
            "uin": uin.unwrap_or(""),
            "qq": uin.unwrap_or(""),
            "authst": authst.unwrap_or("")
        },
        "vkey.GetVkeyServer.CgiGetVkey": {
            "module": "vkey.GetVkeyServer",
            "method": "CgiGetVkey",
            "param": {
                "guid": get_guid(),
                "filename": [filename],
                "songmid": [song_mid],
                "songtype": [0],
                "uin": uin.unwrap_or(""),
                "loginflag": 1,
                "platform": "20"
            }
        }
    });

    // 发送 POST 请求到统一接口
    let resp = CLIENT
        .post("https://u.y.qq.com/cgi-bin/musicu.fcg")
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 提取 vkey.GetVkeyServer.CgiGetVkey 子响应
    let vkey_resp = &data["vkey.GetVkeyServer.CgiGetVkey"];
    if vkey_resp["code"].as_i64().unwrap_or(-1) != 0 {
        return Err(format!("CgiGetVkey 错误: code={}", vkey_resp["code"]));
    }

    // 提取 midurlinfo 数组
    let midurlinfo = vkey_resp["data"]["midurlinfo"]
        .as_array()
        .ok_or("缺少 midurlinfo")?;
    let item = midurlinfo.first().ok_or("未获取到文件信息")?;

    // 提取 purl 和 ekey
    let purl = item["purl"].as_str().unwrap_or("");
    let ekey = item["ekey"].as_str().unwrap_or("").to_string();

    // 检查 purl 是否为空或 result 是否非0
    let result_code = item["result"].as_i64().unwrap_or(0);
    if purl.is_empty() || result_code != 0 {
        let err_msg = match result_code {
            104003 => "无法获取下载链接".to_string(),
            104004 => "该歌曲已下架或禁止下载".to_string(),
            _ => format!(
                "获取下载链接失败，错误码: {}，详情: {:?}",
                result_code,
                item["tips"].as_str().unwrap_or("")
            ),
        };
        return Err(err_msg);
    }

    // 确定下载 URL 前缀：优先使用响应中的 sip 列表第一个地址，若为空则使用默认 CDN
    let default_cdn = "https://wx.music.tc.qq.com/";
    let cdn_prefix = vkey_resp["data"]["sip"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(default_cdn);

    // 拼接完整下载 URL（使用选定的 CDN 前缀 + 相对路径）
    let full_url = format!("{}{}", cdn_prefix, purl);
    Ok((full_url, ekey))
}

/// 获取下载链接与解密密钥（对外统一入口）。
///
/// 该函数从应用设置中读取登录态（uin 和 authst），然后调用 [`fetch_vkey_link`] 获取原始链接和密钥。
/// 根据文件扩展名判断是否为加密文件（`.mgg` 或 `.mflac`），
/// 若是则返回解密密钥，否则强制密钥为空字符串。
///
/// # 参数
/// - `app_handle`: Tauri 应用句柄，用于读取登录态。
/// - `song_mid`: 歌曲的唯一标识（mid）。
/// - `filename`: 品质文件名（例如 `M800001abc.mp3`）。
///
/// # 返回
/// - `Ok((String, String))`：元组 `(完整下载链接, 解密密钥)`。
///   对于非加密文件，密钥始终为空字符串。
/// - `Err(String)`：错误信息，来自 [`fetch_vkey_link`] 或读取登录态的过程。
pub(crate) async fn get_download_link(
    app_handle: &tauri::AppHandle,
    song_mid: &str,
    filename: &str,
) -> Result<(String, String), String> {
    // 读取登录态（未登录时返回 None）
    // 从 settings 获取 loginUin 与 authst
    let (mut uin, mut authst) = get_login_credentials(app_handle).await;

    // 若存在登录态，则自动检查凭证是否过期，过期则尝试刷新
    if let (Some(u), Some(a)) = (&uin, &authst) {
        if !u.is_empty() && !a.is_empty() {
            let expired = super::login::check_credential_expired(app_handle)
                .await
                .unwrap_or(false); // 如果调用出错，例如网络错误，不认为是过期
            if expired {
                log::warn!("QQ音乐凭证已过期，尝试自动刷新");
                match super::login::refresh_credential(app_handle).await {
                    Ok(creds) => {
                        uin = Some(creds.uin);
                        authst = Some(creds.authst);
                        log::info!("QQ音乐凭证自动刷新成功");
                    }
                    Err(e) => {
                        log::warn!("QQ音乐凭证自动刷新失败，继续使用旧凭证: {}", e);
                        // 发射登录刷新失败事件，通知前端弹窗提示用户
                        let _ = app_handle.emit(
                            crate::events::LOGIN_REFRESH_FAILED,
                            format!("QQ音乐登录已过期，自动刷新失败：{}", e),
                        );
                    }
                }
            }
        }
    }

    // 统一调用新接口获取链接和密钥
    let (url, ekey) =
        fetch_vkey_link(song_mid, filename, uin.as_deref(), authst.as_deref()).await?;

    // 根据文件扩展名判断是否为加密文件，决定是否使用 ekey
    let ext = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if ext == "mgg" || ext == "mflac" {
        Ok((url, ekey))
    } else {
        // 非加密文件，强制密钥为空
        Ok((url, String::new()))
    }
}
