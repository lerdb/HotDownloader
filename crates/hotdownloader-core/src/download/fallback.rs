//! 运行中的任务按固定顺序切换品质；不创建新任务，也不占用额外调度槽。
use futures_util::future::BoxFuture;

use super::{config::DownloadConfig, context::TaskContext, path::resolve_download_path};
use crate::task::{
    contract::TaskStatus, rules::TaskRules, service::TaskEnvironment, state::TaskState,
};

pub const QUALITY_EXHAUSTED: &str = "音质候选已耗尽";

#[cfg(test)]
mod tests;

pub enum FallbackOutcome {
    Disabled,
    Exhausted,
    Next(Box<TaskContext>),
}

pub trait DownloadQualityFallback: Send + Sync {
    fn next<'a>(
        &'a self,
        context: &'a TaskContext,
        config: &'a DownloadConfig,
    ) -> BoxFuture<'a, Result<FallbackOutcome, String>>;
}

/// 每次 worker 启动时固定降级顺序，运行中修改设置不会形成往返切换。
pub struct TaskQualityFallback<'a> {
    state: &'a TaskState,
    environment: &'a dyn TaskEnvironment,
    rules: TaskRules,
}

impl<'a> TaskQualityFallback<'a> {
    pub fn new(state: &'a TaskState, environment: &'a dyn TaskEnvironment) -> Self {
        Self {
            state,
            environment,
            rules: environment.task_rules(),
        }
    }
}

impl DownloadQualityFallback for TaskQualityFallback<'_> {
    fn next<'a>(
        &'a self,
        context: &'a TaskContext,
        config: &'a DownloadConfig,
    ) -> BoxFuture<'a, Result<FallbackOutcome, String>> {
        Box::pin(async move {
            if !self.rules.auto_downgrade {
                return Ok(FallbackOutcome::Disabled);
            }
            // 调用方以取消信号包裹此等待，避免取消命令持有创建锁等待 worker 的死锁。
            let _guard = self.state.creation_lock.lock().await;
            let task = self.state.get(&context.task_id).ok_or("任务已移除")?;
            if task.quality != context.quality
                || !matches!(
                    task.status,
                    TaskStatus::Waiting | TaskStatus::Downloading | TaskStatus::Paused
                )
            {
                return Err("任务状态已变化，停止切换音质".into());
            }
            let Some(available) = task.available_qualities.as_deref() else {
                return Ok(FallbackOutcome::Disabled);
            };
            let Some(quality) = self.rules.next_quality(&context.quality, available) else {
                return Ok(FallbackOutcome::Exhausted);
            };
            let mut next = context.clone();
            next.quality = quality.quality.clone();
            next.quality_filename = quality.filename.clone();
            next.file_size = quality.size;
            next.downloaded_offset = 0;
            next.url.clear();
            next.key.clear();
            next.song_info.quality = quality.quality.clone();
            let (is_saf, original, uri) = resolve_download_path(
                &config.download_dir,
                &config.naming_template,
                config.saf_folder_uri.as_deref(),
                &next.song_info,
                &next.quality_filename,
            )?;
            let available_path = |path: &str| {
                !self.state.path_reserved_except(path, &context.task_id)
                    && !self.environment.file_exists(path, is_saf, uri.as_deref())
            };
            next.save_path = original.clone();
            if !available_path(&original) {
                // 降级自动保留已有文件（包括其他品质的部分文件），不会覆盖或混写。
                let mut selected = None;
                for number in 1..10_000 {
                    let candidate = super::path::numbered_path(&original, number, is_saf)?;
                    if available_path(&candidate) {
                        selected = Some(candidate);
                        break;
                    }
                }
                next.save_path = selected.ok_or("无法生成不重复的降级文件名")?;
            }
            // 先持久化再请求下一品质，重启、重试和 UI 都使用同一份真实参数。
            self.state.update(&context.task_id, true, |record| {
                record.quality = next.quality.clone();
                record.filename = next.quality_filename.clone();
                record.file_size = next.file_size;
                record.save_path = Some(next.save_path.clone());
                record.path_warning = super::path::truncation_warning(&next.save_path);
                record.downloaded = 0;
                record.retry_count = 0;
                record.file_path = None;
                record.speed = None;
                record.error_msg = None;
            })?;
            Ok(FallbackOutcome::Next(Box::new(next)))
        })
    }
}
