//! 酷我音乐歌词获取模块。
//!
//! 负责构造酷我歌词请求参数、调用接口获取加密响应，并调用
//! [`crate::utils::kwlyric`] 进行解密和格式化，最终返回 [`LyricData`]。

use crate::platforms::lyric::LyricData;
use crate::utils::http::CLIENT;
use crate::utils::kwlyric;
use base64::{engine::general_purpose::STANDARD, Engine as _};

/// XOR 加密密钥，酷我歌词接口固定使用。
const BUF_KEY: &[u8; 7] = b"yeelion";

/// 构造酷我歌词请求参数。
///
/// 将用户与歌曲信息按酷我私有格式拼接，使用 XOR 密钥加密后 Base64 编码。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/kw/lyric.js#L72>
///
/// # 参数
/// - `music_id`: 酷我歌曲数字 ID（如 `6802907`）。
/// - `is_get_lyricx`: 是否获取逐字歌词（`lrcx=1`），目前始终为 `true`。
///
/// # 返回
/// Base64 编码的加密参数字符串，直接作为 URL query 部分。
fn build_params(music_id: u64, is_get_lyricx: bool) -> String {
    let mut params = format!(
        "user=12345,web,web,web&requester=localhost&req=1&rid=MUSIC_{}",
        music_id
    );
    if is_get_lyricx {
        params.push_str("&lrcx=1");
    }

    let buf_str = params.as_bytes();
    let mut output = Vec::with_capacity(buf_str.len());
    let key_len = BUF_KEY.len();
    let mut i = 0;
    while i < buf_str.len() {
        let mut j = 0;
        while j < key_len && i < buf_str.len() {
            output.push(BUF_KEY[j] ^ buf_str[i]);
            i += 1;
            j += 1;
        }
    }

    STANDARD.encode(output)
}

/// 通过酷我歌曲 ID 获取歌词。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/kw/lyric.js#L236>
///
/// # 参数
/// - `song_id`: 酷我歌曲数字 ID。
///
/// # 返回
/// - `Ok(LyricData)`：包含 `lrc`、`elrc`、`raw` 和 `instrumental`。
/// - `Err(String)`：错误信息。
pub(crate) async fn get_lyric_by_id(song_id: u64) -> Result<LyricData, String> {
    let params = build_params(song_id, true);
    let url = format!("https://newlyric.kuwo.cn/newlyric.lrc?{}", params);

    let resp = CLIENT
        .get(&url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .map_err(|e| format!("歌词请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("歌词接口返回 HTTP {}", resp.status()));
    }

    let data = resp
        .bytes()
        .await
        .map_err(|e| format!("读取歌词响应失败: {}", e))?;

    // 调用 utils 模块解密响应体
    let decoded = kwlyric::decode_lyric(&data, true);
    if decoded.is_empty() {
        return Err("歌词解密失败".into());
    }

    // 调用 utils 模块解析歌词文本
    let (tags, lyric_lines, enhanced_lines) = kwlyric::parse_lyric(&decoded);

    // 调用 utils 模块构建 LRC 和 ELRC 文本
    let lrc = kwlyric::build_lrc_text(&tags, &lyric_lines);
    let elrc = kwlyric::build_enhanced_lrc_text(&tags, &enhanced_lines);

    // 简单的纯音乐检测（可选）
    let instrumental = lrc.contains("纯音乐") || lrc.contains("Instrumental");

    Ok(LyricData {
        lrc: Some(lrc),
        elrc: Some(elrc),
        raw: Some(decoded),
        instrumental,
    })
}
