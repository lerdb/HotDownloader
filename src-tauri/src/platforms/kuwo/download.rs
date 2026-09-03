//! 酷我音乐下载链接获取模块。
//!
//! 通过 `http://mobi.kuwo.cn/mobi.s` 获取指定歌曲指定品质的下载 URL 与解密密钥。
//!
//! 关键点：
//! - 酷我的下载接口**不需要真实文件名**，而是用 `rid` + `br` + `format` 定位资源。
//! - `filename` 字段被复用：编码为 `{bitrate}.{format}` 形式（如 `320.mp3`、`20900.mflac`），
//!   本模块从扩展名解析出 `format`，从 stem 解析出 `bitrate`，无需新增任何字段。
//! - 必须校验响应中 `data.bitrate` 与请求 `br` 一致，否则视为失败。
//! - 解密密钥 `ekey` 是 Base64 编码的 kwDES 密文，需要先 Base64 解码，再用 kwDES 解密，
//!   最后去掉开头的 `user` 前缀（本项目统一使用 `1234`），才是真正可用的密钥。
//! - 是否需要解密取决于 `format`：`mflac`/`mgg` 是加密格式，其他为明文。
//! - 文件解密使用通用的 `umc_qmc` 库。

use std::path::Path;

use serde_json::Value;

use crate::utils::http::CLIENT;
use crate::utils::kwdes;
use rand::Rng;

/// 为每次调用生成随机 `user`（32 位无符号整数）和 `android_id`（16 位小写 hex）。
fn generate_request_params() -> (u32, String) {
    let mut rng = rand::rng();
    // user: 0 到 2^32 之间的随机整数
    let user: u32 = rng.random();
    // android_id: 16 位小写 hex，每字符 4 bit
    let android_id_bytes: [u8; 8] = rng.random();
    let android_id: String = android_id_bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    (user, android_id)
}

/// 获取下载链接与解密密钥（对外统一入口）。
///
/// 调用酷我 `mobi.s` 接口获取指定歌曲指定品质的下载 URL 和解密密钥。
///
/// 参考实现：<https://github.com/Qi-2007/NodeMusicApi/blob/4102619c9541e7d3d8be252c98fec61e67935559/services/kuwoMusic.js#L44>
///
/// # 参数
/// - `_app_handle`: Tauri 应用句柄（保留用于将来登录态接入，当前未使用）。
/// - `song_mid`: 歌曲的数字 ID 字符串（酷我无字符串 mid）。
/// - `filename`: 编码为 `{bitrate}.{format}` 形式的文件名（如 `320.mp3`、`20900.mflac`）。
///   本模块从扩展名解析出 `format`，从 stem 解析出 `bitrate`。
///
/// # 返回
/// - `Ok((String, String))`：元组 `(完整下载链接, 解密密钥)`。
///   加密格式（`mflac`/`mgg`）密钥非空，其他格式密钥为空字符串。
/// - `Err(String)`：错误信息。
pub(crate) async fn get_download_link(
    _app_handle: &tauri::AppHandle,
    song_mid: &str,
    filename: &str,
) -> Result<(String, String), String> {
    // 解析歌曲数字 ID
    let rid: u64 = song_mid
        .parse()
        .map_err(|_| format!("无效的歌曲 ID: {}", song_mid))?;
    if rid == 0 {
        return Err("歌曲 ID 无效".into());
    }

    // 从 filename 解析 bitrate 和 format
    // filename 约定为 `{bitrate}.{format}`（如 `320.mp3`、`20900.mflac`）
    let (bitrate, format) = parse_quality_filename(filename)?;

    // 每次请求生成随机 user 和 android_id
    let (user, android_id) = generate_request_params();
    let user_str = user.to_string();

    // 构造请求 URL
    let br = format!("{}k{}", bitrate, format);
    let url = format!(
        "http://mobi.kuwo.cn/mobi.s?f=web&user={}&android_id={}&source=kwplayer_ar_5.1.0.0_B_jiakong_vh.apk\
         &type=convert_url_with_sign&from=PC&rid={}&br={}&format={}",
         user_str, android_id, rid, br, format
    );

    let resp = CLIENT
        .get(&url)
        .header("User-Agent", "okhttp/4.10.0")
        .header("Referer", "http://www.kuwo.cn/")
        .send()
        .await
        .map_err(|e| format!("网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取响应失败: {}", e))?;
    let data: Value = serde_json::from_str(&text).map_err(|e| format!("解析响应失败: {}", e))?;

    // 检查整体 code
    let code = data["code"].as_i64().unwrap_or(-1);
    if code != 200 {
        let msg = data["msg"].as_str().unwrap_or("");
        return Err(format!("酷我接口错误: code={}, msg={}", code, msg));
    }

    let data_obj = &data["data"];
    if data_obj.is_null() {
        return Err("响应缺少 data 字段".into());
    }

    // 校验响应中的 bitrate 与请求一致
    let resp_bitrate = data_obj["bitrate"].as_u64().unwrap_or(0);
    if resp_bitrate != bitrate as u64 {
        return Err(format!(
            "bitrate 不匹配: 请求 {}，响应 {}（可能该音质不可用）",
            bitrate, resp_bitrate
        ));
    }

    // 校验响应中的 format 与请求一致
    let resp_format = data_obj["format"].as_str().unwrap_or("");
    if resp_format != format {
        return Err(format!(
            "format 不匹配: 请求 {}，响应 {}",
            format, resp_format
        ));
    }

    // 提取下载 URL
    let url = data_obj["url"]
        .as_str()
        .ok_or("响应缺少 url 字段")?
        .to_string();
    if url.is_empty() {
        return Err("下载链接为空".into());
    }

    // 解密密钥处理：
    // 1. 仅对加密格式（.mflac/.mgg）提取 ekey
    // 2. ekey 是 Base64 编码的 kwDES 密文，先 Base64 解码
    // 3. 用 kwDES 解密
    // 4. 去掉开头的 user 前缀（user 与本次请求一致），得到真正的解密密钥
    let ekey = if is_encrypted_format(&format) {
        let raw_ekey = data_obj["ekey"].as_str().unwrap_or("");
        if raw_ekey.is_empty() {
            log::warn!("酷我加密音质但 ekey 为空，rid={}", rid);
            String::new()
        } else {
            match decrypt_ekey(raw_ekey, &user_str) {
                Ok(decrypted) => decrypted,
                Err(e) => {
                    log::error!("酷我 ekey 解密失败: {} (rid={})", e, rid);
                    return Err(e);
                }
            }
        }
    } else {
        String::new()
    };

    Ok((url, ekey))
}

/// 解析 `{bitrate}.{format}` 形式的文件名，返回 `(bitrate, format)` 元组。
///
/// 示例：
/// - `320.mp3`     → `(320, "mp3")`
/// - `20900.mflac` → `(20900, "mflac")`
/// - `100.ogg`     → `(100, "ogg")`
///
/// # 参数
/// - `filename`: 形如 `{bitrate}.{format}` 的字符串。
///
/// # 返回
/// `(bitrate, format)` 元组；解析失败返回错误。
fn parse_quality_filename(filename: &str) -> Result<(u32, String), String> {
    let path = Path::new(filename);

    // 扩展名 = format
    let format = path
        .extension()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("filename 缺少扩展名: {}", filename))?
        .to_string();

    // stem = bitrate
    let bitrate_str = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("filename 缺少 stem: {}", filename))?;
    let bitrate: u32 = bitrate_str
        .parse()
        .map_err(|_| format!("filename 的 stem 不是数字: {}", bitrate_str))?;

    Ok((bitrate, format))
}

/// 判断给定的 `format` 字符串是否是酷我加密格式。
///
/// 加密格式：`mflac`、`mgg`（其他格式 mp3/flac/ogg/aac 都是明文）。
fn is_encrypted_format(format: &str) -> bool {
    matches!(format, "mflac" | "mgg")
}

/// 解密酷我 `ekey` 字段：
/// Base64 解码 → kwDES 解密 → 去除 `user` 前缀。
///
/// # 参数
/// - `raw_ekey`: 接口返回的 Base64 编码密文。
/// - `user`: 本次请求使用的随机 user 字符串，ekey 前缀与之对应。
///
/// # 返回
/// 去前缀后的真实解密密钥字符串。
fn decrypt_ekey(raw_ekey: &str, user: &str) -> Result<String, String> {
    // kwDES 解密（内部包含 Base64 解码）
    let decrypted = kwdes::base64_decrypt(raw_ekey, kwdes::DEFAULT_KEY)?;

    // 验证并去掉 user 前缀
    if let Some(stripped) = decrypted.strip_prefix(user) {
        Ok(stripped.to_string())
    } else {
        Err(format!(
            "ekey 解密结果不以 {} 开头：{}",
            user,
            decrypted.chars().take(20).collect::<String>()
        ))
    }
}
