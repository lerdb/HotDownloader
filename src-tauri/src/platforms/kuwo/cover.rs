//! 酷我音乐封面获取模块。
//!
//! 酷我的封面需要通过独立的 `artistpicserver.kuwo.cn/pic.web` 接口获取，
//! 接口响应体是直接的图片 URL。

use crate::utils::http::CLIENT;

/// 获取歌曲封面 URL。
///
/// 调用酷我封面接口获取指定 `rid`（数字歌曲 ID）的封面图片 URL。
/// 响应体为纯文本 URL。
///
/// 参考实现：<https://github.com/lyswhut/lx-music-desktop/blob/9c364b482e5621a1d38b50e8610d2fb974457e6e/src/renderer/utils/musicSdk/kw/pic.js#L5>
///
/// # 参数
/// - `rid`: 歌曲数字 ID（搜索结果中 `MUSIC_6802907` 去掉前缀后的部分）。
///
/// # 返回
/// - `Ok(String)`：封面图片完整 URL。
/// - `Err(String)`：错误信息。
pub(crate) async fn fetch_cover(rid: u64) -> Result<String, String> {
    let url = format!(
        "http://artistpicserver.kuwo.cn/pic.web?corp=kuwo&type=rid_pic&pictype=500&size=500&rid={}",
        rid
    );

    let resp = CLIENT
        .get(&url)
        .header("Referer", "http://www.kuwo.cn/")
        .send()
        .await
        .map_err(|e| format!("封面请求网络错误: {}", e))?;

    let text = resp
        .text()
        .await
        .map_err(|e| format!("读取封面响应失败: {}", e))?;

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("封面响应为空".into());
    }

    // 简单的有效性检查：以 http 开头
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err(format!("封面响应无效: {}", trimmed));
    }

    Ok(trimmed.to_string())
}
