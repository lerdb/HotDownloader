/// 取链错误分类：区分「平台拒绝」和「瞬时网络抖动」。
///
/// `CgiGetVkey` 对未登录 / 无版权音质返回 104003，对下架歌曲返回 104004。
/// 这两类再试也拿不到链接，不应当成网络错误重试，也不应盖成「网络错误，请稍后重试」。

/// 该音质当前拿不到链接，应立刻把原因交给前端。
pub fn is_unavailable_link_error(err: &str) -> bool {
    (err.contains("无法获取") && err.contains("下载链接"))
        || err.contains("104003")
        || err.contains("已下架")
        || err.contains("禁止下载")
}

/// 可重试的瞬时网络错误。
pub fn is_retryable_link_error(err: &str) -> bool {
    err.starts_with("网络错误") || err.starts_with("读取响应失败")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_blocks_are_unavailable() {
        assert!(is_unavailable_link_error(
            "无法获取该音质的下载链接（可能需要登录，或该歌曲暂无此音质）"
        ));
        assert!(is_unavailable_link_error("该歌曲已下架或禁止下载"));
        assert!(is_unavailable_link_error(
            "获取下载链接失败，错误码: 104003"
        ));
        assert!(!is_unavailable_link_error("网络错误: timeout"));
    }

    #[test]
    fn only_network_errors_are_retryable() {
        assert!(is_retryable_link_error("网络错误: timeout"));
        assert!(is_retryable_link_error("读取响应失败: eof"));
        assert!(!is_retryable_link_error(
            "无法获取该音质的下载链接（可能需要登录，或该歌曲暂无此音质）"
        ));
        assert!(!is_retryable_link_error("该歌曲已下架或禁止下载"));
        assert!(!is_retryable_link_error("接口错误: code=1"));
    }
}
