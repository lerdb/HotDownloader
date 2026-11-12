//! 下载队列的共享暂停状态与 QQ 取链节流。连续失败只说明需要暂停检查。
use serde::Serialize;
use std::{collections::HashSet, sync::Mutex, time::Duration};
use tokio::{sync::Notify, time::Instant};

pub const QUEUE_PAUSED: &str = "下载队列已暂停";

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueStatus {
    pub paused: bool,
    pub reason: Option<String>,
    pub consecutive_link_failures: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn queue() -> QueueControl {
        QueueControl {
            interval: Duration::ZERO,
            ..QueueControl::default()
        }
    }

    #[tokio::test]
    async fn repeated_quality_failures_for_one_song_do_not_stop_other_songs() {
        let queue = queue();
        for _ in 0..10 {
            assert!(queue
                .fetch::<(), _, _>("a", || async { Err("无法获取下载链接".into()) })
                .await
                .is_err());
        }
        assert_eq!(queue.status().consecutive_link_failures, 1);
        assert!(!queue.status().paused);
        assert!(queue
            .fetch::<(), _, _>("b", || async { Err("无法获取下载链接".into()) })
            .await
            .is_err());
        assert!(queue
            .fetch::<(), _, _>("c", || async { Err("无法获取下载链接".into()) })
            .await
            .unwrap_err()
            .starts_with(QUEUE_PAUSED));
        assert!(queue.status().paused);
        assert!(queue
            .fetch("d", || async {
                panic!("暂停后不应继续取链");
                #[allow(unreachable_code)]
                Ok::<(), String>(())
            })
            .await
            .is_err());
        queue.remember_paused("c");
        assert_eq!(queue.resume(), vec!["c"]);
        assert!(queue
            .fetch("d", || async { Ok::<_, String>(()) })
            .await
            .is_ok());
        assert_eq!(queue.status().consecutive_link_failures, 0);
    }

    #[tokio::test]
    async fn explicit_limit_pauses_immediately_and_success_resets_failures() {
        let queue = queue();
        assert!(queue
            .fetch::<(), _, _>("a", || async { Err("HTTP 429".into()) })
            .await
            .unwrap_err()
            .starts_with(QUEUE_PAUSED));
        queue.resume();
        for mid in ["a", "b"] {
            let _ = queue
                .fetch::<(), _, _>(mid, || async { Err("网络错误".into()) })
                .await;
        }
        queue
            .fetch("c", || async { Ok::<_, String>(()) })
            .await
            .unwrap();
        let _ = queue
            .fetch::<(), _, _>("d", || async { Err("无法获取下载链接".into()) })
            .await;
        assert_eq!(queue.status().consecutive_link_failures, 1);
        assert!(!queue.status().paused);
    }

    #[tokio::test]
    async fn concurrent_requests_share_one_spacing_budget() {
        let queue = QueueControl {
            interval: Duration::from_millis(25),
            ..QueueControl::default()
        };
        let starts = Mutex::new(Vec::new());
        let results = futures_util::future::join_all((0..4).map(|_| {
            queue.fetch("same", || async {
                starts.lock().unwrap().push(Instant::now());
                Ok::<_, String>(())
            })
        }))
        .await;
        assert!(results.iter().all(Result::is_ok));
        let starts = starts.lock().unwrap();
        assert!(starts
            .windows(2)
            .all(|pair| pair[1].duration_since(pair[0]) >= Duration::from_millis(25)));
    }
}

#[derive(Default)]
struct State {
    status: QueueStatus,
    failed_songs: HashSet<String>,
    paused_tasks: HashSet<String>,
}

pub struct QueueControl {
    state: Mutex<State>,
    request: tokio::sync::Mutex<Option<Instant>>,
    interval: Duration,
    pub changed: Notify,
}

impl Default for QueueControl {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            request: tokio::sync::Mutex::new(None),
            interval: Duration::from_secs(1),
            changed: Notify::new(),
        }
    }
}

impl QueueControl {
    pub fn status(&self) -> QueueStatus {
        self.state.lock().unwrap().status.clone()
    }
    pub fn pause(&self, reason: String) {
        let mut state = self.state.lock().unwrap();
        state.status.paused = true;
        state.status.reason = Some(reason);
        self.changed.notify_one();
    }
    pub fn remember_paused(&self, id: &str) {
        self.state.lock().unwrap().paused_tasks.insert(id.into());
    }
    pub fn resume(&self) -> Vec<String> {
        let mut state = self.state.lock().unwrap();
        state.status = QueueStatus::default();
        state.failed_songs.clear();
        self.changed.notify_one();
        state.paused_tasks.drain().collect()
    }
    fn check(&self) -> Result<(), String> {
        let status = self.status();
        if status.paused {
            Err(format!(
                "{QUEUE_PAUSED}: {}",
                status.reason.unwrap_or_default()
            ))
        } else {
            Ok(())
        }
    }
    pub async fn fetch<T, F, Fut>(&self, mid: &str, fetch: F) -> Result<T, String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, String>>,
    {
        self.check()?;
        let mut next = self.request.lock().await;
        self.check()?;
        if let Some(at) = *next {
            tokio::time::sleep_until(at).await;
        }
        self.check()?;
        // 锁覆盖实际请求，所有下载并发共用请求间隔。
        *next = Some(Instant::now() + self.interval);
        let result = fetch().await;
        let mut pause_reason = None;
        {
            let mut state = self.state.lock().unwrap();
            match &result {
                Ok(_) => {
                    state.status.consecutive_link_failures = 0;
                    state.failed_songs.clear();
                }
                Err(error) => {
                    let lower = error.to_lowercase();
                    let limited = [
                        "429",
                        "验证码",
                        "限流",
                        "请求过于频繁",
                        "访问频繁",
                        "captcha",
                        "rate limit",
                    ]
                    .iter()
                    .any(|part| lower.contains(part));
                    let unavailable = limited
                        || [
                            "无法获取下载链接",
                            "104003",
                            "网络错误",
                            "读取响应失败",
                            "解析响应失败",
                            "取链接口 HTTP",
                            "CgiGetVkey 错误",
                            "取链网关错误",
                        ]
                        .iter()
                        .any(|part| error.contains(part));
                    if unavailable {
                        state.failed_songs.insert(mid.into());
                        state.status.consecutive_link_failures = state.failed_songs.len();
                    }
                    if limited || state.status.consecutive_link_failures >= 3 {
                        let summary = if limited {
                            "QQ 音乐取链接口返回访问限制提示".to_string()
                        } else {
                            format!(
                                "QQ 音乐连续 {} 首歌曲取链失败",
                                state.status.consecutive_link_failures
                            )
                        };
                        pause_reason = Some(format!("{summary}，已停止派发新任务。最后响应：{error}。请检查账号、网络或稍后恢复队列。"));
                    }
                }
            }
        }
        if let Some(reason) = pause_reason {
            self.pause(reason);
        }
        self.check()?;
        result
    }
}
