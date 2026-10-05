use super::*;
use crate::{
    adapters::local::{
        download_file::LocalDownloadFileOpener, file_deleter::LocalFileDeleter,
        task_repository::JsonTaskRepository,
    },
    download::{
        context::SongInfo,
        engine::{DownloadEngine, DownloadTaskRunner, TaskController},
        link::DownloadLinkProvider,
        ports::{
            DownloadProgressSink, DownloadWorkerPorts, NoopCompletionNotifier,
            NoopDownloadPostprocessor,
        },
        worker::download_task,
    },
    platforms::Platform,
    task::{
        contract::{CreateTaskRequest, CreateTaskResult, SongInput, TaskRecord},
        service::TaskService,
        state::{TaskEventSink, TaskRepository},
    },
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{Mutex as AsyncMutex, Notify},
};
use tokio_util::sync::CancellationToken;

struct Environment {
    directory: PathBuf,
    settings: Value,
}
impl TaskEnvironment for Environment {
    fn task_rules(&self) -> TaskRules {
        TaskRules::from_settings(&self.settings)
    }
    fn download_config(&self) -> DownloadConfig {
        DownloadConfig::from_settings(&self.settings, self.directory.to_str().unwrap())
    }
    fn file_exists(&self, path: &str, _: bool, _: Option<&str>) -> bool {
        Path::new(path).exists()
    }
    fn file_len(&self, path: &str, _: bool) -> Option<u64> {
        std::fs::metadata(path).ok().map(|m| m.len())
    }
}

#[derive(Default)]
struct Events(Mutex<Vec<TaskRecord>>);
impl TaskEventSink for Events {
    fn updated(&self, task: TaskRecord) {
        self.0.lock().unwrap().push(task);
    }
    fn removed(&self, _: &str) {}
}
#[derive(Default)]
struct Runner(Mutex<Vec<String>>);
impl DownloadTaskRunner for Runner {
    fn run(&self, ctx: TaskContext, _: TaskController) -> BoxFuture<'static, bool> {
        self.0.lock().unwrap().push(ctx.quality);
        Box::pin(async { false })
    }
    fn report_error(&self, _: &str, _: &str) {}
}
struct Sink<'a>(&'a TaskState);
impl DownloadProgressSink for Sink<'_> {
    fn progress(&self, id: &str, downloaded: u64, total: u64, speed: u64) {
        self.0.progress(id, downloaded, total, speed);
    }
    fn file_complete(&self, id: &str) {
        self.0.file_complete(id);
    }
    fn completed(&self, id: &str, path: &str, _: Option<String>) {
        self.0.completed(id, path);
    }
    fn error(&self, id: &str, message: &str) {
        self.0.failed(id, message, None);
    }
    fn link_expired(&self, id: &str, offset: u64) {
        self.0.failed(id, "链接过期", Some(offset));
    }
    fn metadata_error(&self, _: &str, _: &str) {}
}

struct Links {
    calls: Mutex<Vec<String>>,
    success_file: String,
    url: String,
    error: String,
    called: Notify,
}
impl Links {
    fn rejected() -> Self {
        Self {
            calls: Mutex::new(vec![]),
            success_file: String::new(),
            url: String::new(),
            error: "平台拒绝: 模拟音质链接不可用".into(),
            called: Notify::new(),
        }
    }
}
impl DownloadLinkProvider for Links {
    fn fetch<'a>(
        &'a self,
        _: Platform,
        _: &'a str,
        filename: &'a str,
    ) -> BoxFuture<'a, Result<(String, String), String>> {
        self.calls.lock().unwrap().push(filename.into());
        self.called.notify_one();
        Box::pin(async move {
            if filename == self.success_file {
                Ok((self.url.clone(), String::new()))
            } else {
                Err(self.error.clone())
            }
        })
    }
}

struct Fixture {
    env: Environment,
    state: TaskState,
    events: Arc<Events>,
    runner: Arc<Runner>,
    engine: DownloadEngine,
    context: TaskContext,
    controller: TaskController,
}
impl Fixture {
    async fn new(settings: Value) -> Self {
        let root = std::env::temp_dir().join("hotdownloader-fallback-tests");
        let directory = root.join(format!("{:x}", rand::random::<u64>()));
        std::fs::create_dir_all(&directory).unwrap();
        let env = Environment {
            directory: directory.canonicalize().unwrap(),
            settings,
        };
        let events = Arc::new(Events::default());
        let state = TaskState::load(
            Arc::new(JsonTaskRepository::new(env.directory.join("tasks.json"))),
            events.clone(),
        )
        .unwrap();
        let runner = Arc::new(Runner::default());
        let engine = DownloadEngine::new(
            runner.clone(),
            Arc::new(LocalFileDeleter),
            Arc::new(NoopCompletionNotifier),
        );
        let song: SongInput = serde_json::from_value(json!({
            "platform":"qqmusic", "id":1, "mid":"fictionalMid", "title":"Synthetic", "artist":"Fixture", "album":"Test",
            "qualities":[
                {"quality":"臻品母带", "filename":"master.mflac", "size":100},
                {"quality":"hires", "filename":"hires.mflac", "size":80},
                {"quality":"flac", "filename":"fallback.flac", "size":4},
                {"quality":"320kmp3", "filename":"fallback.mp3", "size":4}
            ]
        })).unwrap();
        let CreateTaskResult::Created { task } = TaskService::new(&state, &engine, &env)
            .create_download_task(CreateTaskRequest {
                song,
                desired_quality: "臻品母带".into(),
                duplicate_action: None,
            })
            .await
            .unwrap()
        else {
            panic!("expected task")
        };
        let final_path = Arc::new(AsyncMutex::new(None));
        let context = TaskContext {
            task_id: task.id.clone(),
            platform: Platform::QqMusic,
            song_mid: task.song_mid,
            song_id: task.song_id,
            url: String::new(),
            save_path: task.save_path.unwrap(),
            quality: task.quality.clone(),
            key: String::new(),
            file_size: task.file_size,
            downloaded_offset: 0,
            song_info: SongInfo {
                title: task.song_title,
                artist: task.artist,
                album: task.album,
                quality: task.quality,
                cover_url: String::new(),
            },
            quality_filename: task.filename,
            final_path: final_path.clone(),
        };
        let controller = TaskController {
            cancel_token: CancellationToken::new(),
            pause_flag: Arc::new(AtomicBool::new(false)),
            resume_notify: Arc::new(Notify::new()),
            url_ready: Arc::new(Notify::new()),
            delete_file_on_cancel: Arc::new(AtomicBool::new(false)),
            final_path,
            lrc_final_path: Arc::new(AsyncMutex::new(None)),
            started: Arc::new(AtomicBool::new(true)),
            done: Arc::new(Notify::new()),
        };
        Self {
            env,
            state,
            events,
            runner,
            engine,
            context,
            controller,
        }
    }
    async fn run(&self, links: &Links) -> bool {
        let fallback = TaskQualityFallback::new(&self.state, &self.env);
        download_task(
            self.context.clone(),
            self.controller.clone(),
            self.env.download_config(),
            DownloadWorkerPorts {
                quality_fallback: Some(&fallback),
                link_provider: links,
                progress_sink: &Sink(&self.state),
                file_opener: &LocalDownloadFileOpener,
                file_deleter: &LocalFileDeleter,
                postprocessor: &NoopDownloadPostprocessor,
            },
        )
        .await
    }
    fn task(&self) -> TaskRecord {
        self.state.get(&self.context.task_id).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let root = std::env::temp_dir()
            .join("hotdownloader-fallback-tests")
            .canonicalize()
            .unwrap();
        assert!(self.env.directory.starts_with(&root) && self.env.directory != root);
        std::fs::remove_dir_all(&self.env.directory).unwrap();
    }
}

async fn local_audio() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/fixture", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        let count = stream.read(&mut request).await.unwrap();
        assert!(!String::from_utf8_lossy(&request[..count])
            .to_lowercase()
            .contains("range:"));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nTEST")
            .await
            .unwrap();
    });
    (url, handle)
}

#[tokio::test]
async fn master_and_hires_rejected_then_flac_completes_same_task() {
    let f = Fixture::new(json!({"namingTemplate":"{song} - {quality}"})).await;
    let (url, server) = local_audio().await;
    let links = Links {
        success_file: "fallback.flac".into(),
        url,
        ..Links::rejected()
    };
    assert!(f.run(&links).await);
    server.await.unwrap();
    assert_eq!(
        *links.calls.lock().unwrap(),
        ["master.mflac", "hires.mflac", "fallback.flac"]
    );
    let task = f.task();
    assert_eq!(task.status, TaskStatus::Completed);
    assert_eq!(task.quality, "flac");
    assert_eq!(task.filename, "fallback.flac");
    assert_eq!(task.file_size, 4);
    assert_eq!(task.retry_count, 0);
    assert!(task
        .save_path
        .as_ref()
        .unwrap()
        .ends_with("Synthetic - flac.flac"));
    assert_eq!(std::fs::read(task.file_path.unwrap()).unwrap(), b"TEST");
    assert_eq!(f.state.list().len(), 1);
    assert!(!f
        .events
        .0
        .lock()
        .unwrap()
        .iter()
        .any(|t| t.status == TaskStatus::Error));
    let saved = JsonTaskRepository::new(f.env.directory.join("tasks.json"))
        .load()
        .unwrap();
    assert_eq!(saved[0].quality, "flac");
}

#[tokio::test]
async fn disabled_fallback_stops_on_master_failure() {
    let f = Fixture::new(json!({"autoDowngrade":false})).await;
    let links = Links::rejected();
    assert!(!f.run(&links).await);
    assert_eq!(*links.calls.lock().unwrap(), ["master.mflac"]);
    assert_eq!(f.task().quality, "臻品母带");
    assert!(!f.task().error_msg.unwrap().starts_with(QUALITY_EXHAUSTED));
}

#[tokio::test]
async fn rejected_candidates_are_exhausted_once_without_looping() {
    let f = Fixture::new(json!({})).await;
    let links = Links::rejected();
    assert!(!f.run(&links).await);
    assert_eq!(
        *links.calls.lock().unwrap(),
        [
            "master.mflac",
            "hires.mflac",
            "fallback.flac",
            "fallback.mp3"
        ]
    );
    assert!(f.task().error_msg.unwrap().starts_with(QUALITY_EXHAUSTED));
}

#[tokio::test]
async fn custom_order_is_used_and_earlier_qualities_are_not_revisited() {
    let f =
        Fixture::new(json!({"qualityDowngradeOrder":["flac","臻品母带","320kmp3","hires"]})).await;
    let links = Links::rejected();
    assert!(!f.run(&links).await);
    assert_eq!(
        *links.calls.lock().unwrap(),
        ["master.mflac", "fallback.mp3", "hires.mflac"]
    );
}

#[tokio::test]
async fn credential_errors_do_not_walk_quality_candidates() {
    let f = Fixture::new(json!({})).await;
    let links = Links {
        error: "登录凭据已失效".into(),
        ..Links::rejected()
    };
    assert!(!f.run(&links).await);
    assert_eq!(links.calls.lock().unwrap().len(), 1);
    assert!(f.task().error_msg.unwrap().contains("登录凭据已失效"));
}

#[tokio::test]
async fn fallback_keeps_old_partial_file_and_reserved_targets() {
    let mut f = Fixture::new(json!({})).await;
    std::fs::write(&f.context.save_path, b"OLD PARTIAL").unwrap();
    f.context.downloaded_offset = 11;
    let mut reserved = f.task();
    reserved.id = "other-task".into();
    reserved.save_path = Some(
        f.env
            .directory
            .join("Synthetic - Fixture (1).flac")
            .to_string_lossy()
            .into(),
    );
    f.state.insert(reserved).unwrap();
    let (url, server) = local_audio().await;
    let links = Links {
        success_file: "fallback.flac".into(),
        url,
        ..Links::rejected()
    };
    assert!(f.run(&links).await);
    server.await.unwrap();
    assert_eq!(std::fs::read(&f.context.save_path).unwrap(), b"OLD PARTIAL");
    assert!(f
        .task()
        .save_path
        .unwrap()
        .ends_with("Synthetic - Fixture (2).flac"));
    assert_eq!(std::fs::read(f.task().file_path.unwrap()).unwrap(), b"TEST");
}

#[tokio::test]
async fn cancellation_interrupts_fallback_waiting_for_creation_lock() {
    let f = Fixture::new(json!({})).await;
    let links = Links::rejected();
    let _guard = f.state.creation_lock.lock().await;
    let (result, _) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        tokio::join!(f.run(&links), async {
            links.called.notified().await;
            f.controller.cancel_token.cancel();
        })
    })
    .await
    .expect("取消不能与创建锁形成死锁");
    assert!(!result);
    assert_eq!(links.calls.lock().unwrap().len(), 1);
    assert_ne!(f.task().status, TaskStatus::Error);
}

#[tokio::test]
async fn retry_rebuilds_context_from_last_persisted_quality() {
    let f = Fixture::new(json!({})).await;
    let engine = f.engine.clone();
    let scheduler = tokio::spawn(async move { engine.run_scheduler().await });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while f.runner.0.lock().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
        f.engine.wait_for_task_exit(&f.context.task_id).await;
    })
    .await
    .unwrap();
    assert!(!f.run(&Links::rejected()).await);
    // 引擎仍保存最初入队时的母带上下文，重试必须改用已持久化的 MP3。
    TaskService::new(&f.state, &f.engine, &f.env)
        .retry_task(f.context.task_id.clone())
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if f.runner.0.lock().unwrap().iter().any(|q| q == "320kmp3") {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    scheduler.abort();
}

#[tokio::test]
async fn paused_worker_does_not_start_quality_requests_until_resumed() {
    let f = Fixture::new(json!({})).await;
    f.controller.pause_flag.store(true, Ordering::SeqCst);
    let links = Links::rejected();
    let (completed, _) = tokio::join!(f.run(&links), async {
        tokio::task::yield_now().await;
        assert!(links.calls.lock().unwrap().is_empty());
        f.controller.pause_flag.store(false, Ordering::SeqCst);
        f.controller.resume_notify.notify_one();
    });
    assert!(!completed);
    assert_eq!(links.calls.lock().unwrap().len(), 4);
}
