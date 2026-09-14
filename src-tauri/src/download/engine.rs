use std::collections::{HashMap, VecDeque};
use std::io::ErrorKind;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use log;
use tauri::AppHandle;
use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
use tokio::sync::{Mutex, Notify};
use tokio_util::sync::CancellationToken;

use futures_util::FutureExt;

use super::progress;
use super::task::{download_task, TaskContext};
use crate::platforms::Platform;

/// 引擎中没有对应任务上下文时的错误码。
/// 前端据此判断需要“重新注册任务”，而不是把失败当成普通错误。
pub const ERR_TASK_CONTEXT_MISSING: &str = "TASK_CONTEXT_MISSING";

/// 并发槽位守卫：任务无论以何种方式结束（正常/取消/panic）都会归还槽位并唤醒调度器。
struct ActiveDownloadGuard {
    counter: Arc<AtomicU32>,
    notify: Arc<Notify>,
}

impl Drop for ActiveDownloadGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
        self.notify.notify_one();
    }
}

/// 从 panic 载荷中提取可读信息，用于上报给前端。
// 使用更符合 Rust 惯例的动态 trait 引用，避免对 Box 的额外引用层级。
fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "未知 panic".to_string()
    }
}

#[derive(Clone)]
pub struct TaskController {
    pub cancel_token: CancellationToken,
    pub pause_flag: Arc<AtomicBool>,
    pub resume_notify: Arc<Notify>,
    pub url_ready: Arc<Notify>,
    /// 取消时是否需要删除文件
    pub delete_file_on_cancel: Arc<AtomicBool>,
    /// 下载线程确定的最终文件路径（供外部删除使用）
    pub final_path: Arc<Mutex<Option<String>>>,
    /// 下载线程确定的最终 LRC 文件路径（供外部删除使用）
    pub lrc_final_path: Arc<Mutex<Option<String>>>,
    /// 任务是否已由调度器启动
    pub started: Arc<AtomicBool>,
    /// 任务完成或退出的通知（用于 remove 等待）
    pub done: Arc<Notify>,
}

#[derive(Clone)]
pub struct DownloadEngine {
    pub app_handle: AppHandle,
    ready_tasks: Arc<Mutex<VecDeque<TaskContext>>>,
    active_controllers: Arc<Mutex<HashMap<String, TaskController>>>,
    max_concurrent: Arc<AtomicU32>,
    active_downloads: Arc<AtomicU32>,
    scheduler_notify: Arc<Notify>,
    task_contexts: Arc<Mutex<HashMap<String, TaskContext>>>,
    /// 存储任务最终文件路径（普通路径或 SAF URI），用于删除文件
    final_paths: Arc<Mutex<HashMap<String, String>>>,
    /// 存储任务对应的 LRC 歌词文件路径/URI，用于删除时一并清理
    lrc_paths: Arc<Mutex<HashMap<String, String>>>,
    /// 保护任务启动与移除的互斥锁，避免竞态
    start_lock: Arc<Mutex<()>>,
}

impl DownloadEngine {
    pub fn new(app_handle: AppHandle) -> Self {
        DownloadEngine {
            app_handle,
            ready_tasks: Arc::new(Mutex::new(VecDeque::new())),
            active_controllers: Arc::new(Mutex::new(HashMap::new())),
            max_concurrent: Arc::new(AtomicU32::new(3)),
            active_downloads: Arc::new(AtomicU32::new(0)),
            scheduler_notify: Arc::new(Notify::new()),
            task_contexts: Arc::new(Mutex::new(HashMap::new())),
            final_paths: Arc::new(Mutex::new(HashMap::new())),
            lrc_paths: Arc::new(Mutex::new(HashMap::new())),
            start_lock: Arc::new(Mutex::new(())),
        }
    }

    /// 异步添加任务，同时预计算最终保存路径
    pub async fn add_task(
        &self,
        task_id: String,
        platform: Platform,
        song_id: u64,
        song_mid: String,
        url: String,
        save_path: String,
        quality: String,
        filename: String,
        key: String,
        file_size: u64,
        song_title: String,
        artist: String,
        album: String,
        cover_url: String,
    ) {
        let controller = TaskController {
            cancel_token: CancellationToken::new(),
            pause_flag: Arc::new(AtomicBool::new(false)),
            resume_notify: Arc::new(Notify::new()),
            url_ready: Arc::new(Notify::new()),
            delete_file_on_cancel: Arc::new(AtomicBool::new(false)),
            final_path: Arc::new(Mutex::new(None)),
            lrc_final_path: Arc::new(Mutex::new(None)),
            started: Arc::new(AtomicBool::new(false)),
            done: Arc::new(Notify::new()),
        };

        let ctx = TaskContext {
            task_id: task_id.clone(),
            platform,
            song_id,
            song_mid,
            url,
            save_path,
            quality: quality.clone(), // 保留原 quality 字段
            quality_filename: filename,
            key,
            file_size,
            downloaded_offset: 0,
            app_handle: self.app_handle.clone(),
            song_info: super::task::SongInfo {
                title: song_title,
                artist,
                album,
                quality,   // 传入品质
                cover_url, // 传入封面链接
            },
            final_path: controller.final_path.clone(), // 共享路径
        };

        // 继承可能已有的最终路径（例如任务重试后路径不变）
        if let Some(p) = self.final_paths.lock().await.get(&task_id).cloned() {
            *controller.final_path.lock().await = Some(p.clone());
            *ctx.final_path.lock().await = Some(p);
        }

        self.task_contexts
            .lock()
            .await
            .insert(task_id.clone(), ctx.clone());
        self.active_controllers
            .lock()
            .await
            .insert(task_id, controller);
        self.ready_tasks.lock().await.push_back(ctx);
        self.scheduler_notify.notify_one();
    }

    /// 异步更新任务 URL 并移入就绪队列（重试时调用）。
    ///
    /// 若引擎中不存在该任务的上下文（典型场景：应用重启后从磁盘恢复的任务，
    /// 引擎是全新的、没有任何上下文），返回 [`ERR_TASK_CONTEXT_MISSING`] 而不是
    /// 静默返回 —— 否则前端会以为入队成功，任务将永远停留在“等待中”。
    /// 调用方收到该错误后可以用任务记录重新注册任务。
    pub async fn enqueue_task(&self, task_id: &str, offset: u64) -> Result<(), String> {
        // 获取任务上下文（克隆后修改）
        let mut ctx = match self.task_contexts.lock().await.get(task_id).cloned() {
            Some(c) => c,
            None => {
                log::warn!("任务 {} 不存在于引擎上下文中，无法入队", task_id);
                return Err(ERR_TASK_CONTEXT_MISSING.to_string());
            }
        };
        ctx.downloaded_offset = offset;
        ctx.url.clear(); // 强制在下载线程中重新获取链接

        // 如果任务之前已经结束，控制器可能已被移除；
        // 为了能让调度器再次启动该任务，需要重新创建一个控制器。
        // 保留与上下文共享的 final_path，确保后续删除文件能找到路径。
        let controller = TaskController {
            cancel_token: CancellationToken::new(),
            pause_flag: Arc::new(AtomicBool::new(false)),
            resume_notify: Arc::new(Notify::new()),
            url_ready: Arc::new(Notify::new()),
            delete_file_on_cancel: Arc::new(AtomicBool::new(false)),
            final_path: ctx.final_path.clone(), // 共享同一个 final_path
            lrc_final_path: Arc::new(Mutex::new(None)),
            started: Arc::new(AtomicBool::new(false)),
            done: Arc::new(Notify::new()),
        };

        // 保留已有的 final_paths 条目（如果存在）
        if let Some(p) = self.final_paths.lock().await.get(task_id).cloned() {
            *controller.final_path.lock().await = Some(p);
        }

        // 重新插入控制器（覆盖可能存在的旧控制器）
        self.active_controllers
            .lock()
            .await
            .insert(task_id.to_string(), controller);
        // 更新上下文（保存偏移量等修改）
        self.task_contexts
            .lock()
            .await
            .insert(task_id.to_string(), ctx.clone());
        // 放入就绪队列
        self.ready_tasks.lock().await.push_back(ctx);
        self.scheduler_notify.notify_one();
        Ok(())
    }

    /// 异步暂停任务
    pub async fn pause(&self, task_id: &str) {
        if let Some(ctrl) = self.active_controllers.lock().await.get(task_id) {
            ctrl.pause_flag.store(true, Ordering::SeqCst);
        }
    }

    /// 异步恢复任务
    pub async fn resume(&self, task_id: &str) {
        if let Some(ctrl) = self.active_controllers.lock().await.get(task_id) {
            ctrl.pause_flag.store(false, Ordering::SeqCst);
            ctrl.resume_notify.notify_one();
        }
    }

    /// 取消任务（下载线程自行处理文件删除）
    pub async fn cancel(&self, task_id: &str, delete_file: bool) {
        log::info!("取消任务 {} (delete_file={})", task_id, delete_file);
        if let Some(ctrl) = self.active_controllers.lock().await.get(task_id) {
            ctrl.cancel_token.cancel();
            // 将删除意图传递给下载线程
            ctrl.delete_file_on_cancel
                .store(delete_file, Ordering::SeqCst);
            // 如果任务处于暂停等待状态，需要唤醒它以便退出循环
            ctrl.resume_notify.notify_one();
            ctrl.url_ready.notify_one();
        }

        // 清理队列
        self.ready_tasks
            .lock()
            .await
            .retain(|t| t.task_id != task_id);

        // 注意：不再在此处删除文件，改为 download_task 完成后自行处理
    }

    /// 设置并发数（同步，无需 Tokio 上下文）
    pub fn set_concurrency(&self, max: u32) {
        self.max_concurrent.store(max, Ordering::SeqCst);
        self.scheduler_notify.notify_one();
    }

    /// 调度循环（在后台 Tokio 任务中运行）
    pub async fn run_scheduler(&self) {
        loop {
            // 启动尽可能多的就绪任务
            while let Some(ctx) = {
                let mut ready = self.ready_tasks.lock().await;
                ready.pop_front()
            } {
                let current = self.active_downloads.load(Ordering::SeqCst);
                let max = self.max_concurrent.load(Ordering::SeqCst);
                if current >= max {
                    self.ready_tasks.lock().await.push_front(ctx);
                    break;
                }

                let mut should_start = false;
                let mut stale_task_id: Option<String> = None;
                let ctrl = {
                    // 先获取 start_lock，再在锁内获取控制器并设置 started
                    let _guard = self.start_lock.lock().await;

                    // 获取控制器（克隆，不持有 active_controllers 锁过久）
                    let ctrl_opt = {
                        let controllers = self.active_controllers.lock().await;
                        controllers.get(&ctx.task_id).cloned()
                    };

                    // 使用 match 确保类型一致
                    match ctrl_opt {
                        Some(ctrl) => {
                            if ctrl.cancel_token.is_cancelled() {
                                // 任务已被取消，通知 done 并返回 None
                                ctrl.done.notify_one();
                                None
                            } else if ctrl.started.load(Ordering::SeqCst) {
                                // 该任务已经在下载中，说明就绪队列里存在重复条目，
                                // 直接忽略，避免同一任务被并发下载两次（会互相覆盖文件）
                                log::warn!("任务 {} 已在运行，忽略重复的就绪条目", ctx.task_id);
                                None
                            } else {
                                // 设置 started 并标记可以启动
                                ctrl.started.store(true, Ordering::SeqCst);
                                should_start = true;
                                Some(ctrl)
                            }
                        }
                        None => {
                            // 没有控制器，无法启动。记录下来并通知前端，
                            // 否则任务会永远停留在“等待中”且没有任何提示。
                            stale_task_id = Some(ctx.task_id.clone());
                            None
                        }
                    }
                }; // 锁在此处释放

                if let Some(stale_id) = stale_task_id {
                    log::warn!("任务 {} 缺少控制器，已通知前端标记失败", stale_id);
                    progress::emit_error(
                        &self.app_handle,
                        &stale_id,
                        "任务已失效，请重新添加或重试",
                    );
                    continue;
                }

                if let Some(ctrl) = ctrl {
                    if !should_start {
                        continue;
                    }

                    self.active_downloads.fetch_add(1, Ordering::SeqCst);
                    let active_downloads = self.active_downloads.clone();
                    let scheduler_notify = self.scheduler_notify.clone();
                    let app_handle = self.app_handle.clone();
                    let task_id = ctx.task_id.clone();
                    let engine = self.clone();
                    let ctrl_clone = ctrl.clone();

                    let notify_song_title = ctx.song_info.title.clone();
                    let notify_artist = ctx.song_info.artist.clone();
                    let notify_app_handle = app_handle.clone();

                    tokio::spawn(async move {
                        // 并发槽位守卫：无论任务是正常结束、被取消还是中途 panic，
                        // 都保证 active_downloads 会被归还并唤醒调度器。
                        // （旧实现把 fetch_sub 写在函数末尾，一旦下载任务 panic 就永远不会执行，
                        //   槽位会永久泄漏，最终所有任务都停在“等待中”。）
                        let _slot = ActiveDownloadGuard {
                            counter: active_downloads.clone(),
                            notify: scheduler_notify.clone(),
                        };

                        // 捕获 download_task 内部的 panic（例如第三方库按字节切分多字节字符串），
                        // 把 panic 转成任务级错误，避免整个应用被 panic = "abort" 直接终止，
                        // 也避免任务永远停在“处理中”。
                        let panic_task_id = task_id.clone();
                        let panic_app_handle = notify_app_handle.clone();
                        let completed_ok = match std::panic::AssertUnwindSafe(download_task(
                            ctx,
                            ctrl_clone.clone(),
                            app_handle,
                        ))
                        .catch_unwind()
                        .await
                        {
                            Ok(ok) => ok,
                            Err(payload) => {
                                let msg = panic_payload_message(payload.as_ref());
                                log::error!("任务 {} 执行中发生 panic: {}", panic_task_id, msg);
                                progress::emit_error(
                                    &panic_app_handle,
                                    &panic_task_id,
                                    &format!("任务内部错误: {}", msg),
                                );
                                false
                            }
                        };

                        // 提取最终路径并存入 final_paths（在通知 done 之前）
                        let final_path = ctrl_clone.final_path.lock().await.clone();
                        if let Some(p) = final_path {
                            engine.final_paths.lock().await.insert(task_id.clone(), p);
                        }
                        // 保存 LRC 歌词文件路径（若存在）
                        let lrc_path = ctrl_clone.lrc_final_path.lock().await.clone();
                        if let Some(lp) = lrc_path {
                            engine.lrc_paths.lock().await.insert(task_id.clone(), lp);
                        }

                        // 通知任务完成（供 remove 等待）
                        ctrl_clone.done.notify_one();

                        // 下载结束（完成/错误），仅移除控制器，保留任务上下文供删除文件使用
                        engine.active_controllers.lock().await.remove(&task_id);

                        if completed_ok {
                            // 下载完成后发送系统通知
                            let notify_enabled = crate::storage::store_wrapper::load_string(
                                &notify_app_handle,
                                "settings",
                            )
                            .ok()
                            .and_then(|json_str| {
                                serde_json::from_str::<serde_json::Value>(&json_str).ok()
                            })
                            .and_then(|settings| {
                                settings.get("notifyOnComplete").and_then(|v| v.as_bool())
                            })
                            .unwrap_or(false); // 默认不发送通知

                            if notify_enabled {
                                crate::commands::notify::send_download_complete_notification(
                                    &notify_app_handle,
                                    &notify_song_title,
                                    &notify_artist,
                                );
                            } else {
                                log::info!("用户已关闭下载完成通知，跳过发送");
                            }

                            // 任务成功完成后自动清理 task_contexts，减少内存占用
                            engine.task_contexts.lock().await.remove(&task_id);
                        }
                    });
                }
                // 如果 ctrl 不存在，跳过该任务（可能已被取消）
            }

            self.scheduler_notify.notified().await;
        }
    }

    /// 删除单个文件路径/URI。SAF 与普通文件统一处理。
    /// `NotFound` 视为删除成功。
    async fn delete_file_path(&self, path: &str) -> Result<(), String> {
        // 判断是否为 SAF URI
        if path.starts_with("content://") || path.starts_with("saf://") {
            // SAF 模式：使用插件 API 删除
            let fs_uri = FsUri::from_uri(path.to_string());
            let api = self.app_handle.android_fs();
            api.remove_file(&fs_uri)
                .map_err(|e| format!("删除 SAF 文件失败: {}", e))
        } else {
            // 普通模式：使用标准库删除
            match tokio::fs::remove_file(path).await {
                Ok(()) => Ok(()),
                // 文件不存在视为删除成功，避免误报错误
                Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
                Err(e) => Err(format!("删除文件失败: {}", e)),
            }
        }
    }

    /// 移除任务记录，并可选删除文件。返回 Result 以便删除失败时通知用户。
    pub async fn remove(&self, task_id: &str, delete_file: bool) -> Result<(), String> {
        // 1. 清理就绪队列，防止任务再次启动
        self.ready_tasks
            .lock()
            .await
            .retain(|t| t.task_id != task_id);

        let (controller, should_wait) = {
            // 2. 先获取 start_lock，再移除控制器并读取 started
            let _guard = self.start_lock.lock().await;

            // 移除控制器（所有权转移）
            let controller = self.active_controllers.lock().await.remove(task_id);

            let should_wait = if let Some(ctrl) = &controller {
                ctrl.started.load(Ordering::SeqCst)
            } else {
                false
            };

            (controller, should_wait)
        }; // 锁在此处释放

        // 3. 发送取消信号并唤醒（如果有控制器）
        if let Some(ctrl) = &controller {
            ctrl.cancel_token.cancel();
            ctrl.resume_notify.notify_one();
            ctrl.url_ready.notify_one();
        }

        // 4. 如果任务已启动，等待其退出
        if should_wait {
            if let Some(ctrl) = &controller {
                ctrl.done.notified().await;
            }
        }

        // 5. 获取最终文件路径：优先 final_paths，回退到任务上下文（避免嵌套锁）
        let final_path = {
            let paths = self.final_paths.lock().await;
            paths.get(task_id).cloned()
        };
        let final_path = if final_path.is_some() {
            final_path
        } else {
            // 先克隆 Arc，再释放 task_contexts 锁
            let final_path_arc = {
                let ctx_map = self.task_contexts.lock().await;
                ctx_map.get(task_id).map(|ctx| ctx.final_path.clone())
            };
            if let Some(fp) = final_path_arc {
                let guard = fp.lock().await;
                guard.clone()
            } else {
                None
            }
        };

        // 用 delete_error 暂存主文件删除错误，继续执行 LRC 删除和 final_paths/task_contexts 清理，最后统一返回错误。
        let mut delete_error: Option<String> = None;

        // 6. 删除文件（如果要求）
        if delete_file {
            if let Some(path) = &final_path {
                if let Err(e) = self.delete_file_path(path).await {
                    log::error!("删除文件失败 {}: {}", path, e);
                    // 删除主文件失败时不再提前返回，确保 LRC 文件删除与路径映射清理仍会执行。
                    delete_error = Some(e);
                } else {
                    log::info!("已删除文件: {}", path);
                }
            }
        }

        // 删除对应的 LRC 歌词文件（如果存在且要求删除文件）
        if delete_file {
            if let Some(lrc) = self.lrc_paths.lock().await.remove(task_id) {
                if let Err(e) = self.delete_file_path(&lrc).await {
                    // LRC 删除失败不阻塞主流程，仅记录日志
                    log::warn!("删除 LRC 文件失败 {}: {}", lrc, e);
                } else {
                    log::info!("已删除 LRC 文件: {}", lrc);
                }
            }
        } else {
            // 不删除文件时，也需要清理 LRC 路径映射，避免残留
            self.lrc_paths.lock().await.remove(task_id);
        }

        // 7. 清除 final_paths 和 task_contexts
        self.final_paths.lock().await.remove(task_id);
        self.task_contexts.lock().await.remove(task_id);

        if let Some(e) = delete_error {
            Err(e)
        } else {
            Ok(())
        }
    }
}
