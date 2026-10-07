//! 合成音频、真实持久化错误和独立子进程崩溃恢复；不启动真实下载执行器。
use super::*;
use hotdownloader_core::{
    adapters::local::task_repository::JsonTaskRepository, task::state::TaskRepository,
};
use lofty::{
    file::TaggedFileExt,
    tag::{ItemValue, TagItem},
};
use std::{
    io::Write,
    process::{Child, Command, Stdio},
    time::Instant,
};

const MP3: &[u8] = include_bytes!("../../testdata/audio/silence.mp3");
const FLAC: &[u8] = include_bytes!("../../testdata/audio/silence.flac");
const CHILD_TEST: &str = "monitor::tests::reliability::crash_child";

fn tagged_audio(path: &Path, bytes: &[u8], kind: TagType, title: &str) {
    std::fs::write(path, bytes).unwrap();
    let mut tag = Tag::new(kind);
    tag.insert_text(ItemKey::TrackTitle, title.into());
    // ID3v2.4 在唯一 TPE1 帧内以 NUL 分隔；Vorbis Comments 使用重复 ARTIST 字段。
    if kind == TagType::Id3v2 {
        tag.insert_text(ItemKey::TrackArtist, "虚构乙\0虚构甲".into());
    } else {
        for artist in ["虚构乙", "虚构甲"] {
            assert!(tag.push(TagItem::new(
                ItemKey::TrackArtist,
                ItemValue::Text(artist.into())
            )));
        }
    }
    tag.save_to_path(path, WriteOptions::default()).unwrap();
    let audio = lofty::read_from_path(path).unwrap();
    let read = audio.primary_tag().or_else(|| audio.first_tag()).unwrap();
    assert_eq!(
        read.get_strings(&ItemKey::TrackArtist).collect::<Vec<_>>(),
        ["虚构乙", "虚构甲"]
    );
}

#[test]
fn mp3_id3_and_flac_vorbis_multi_artist_tags_are_indexed_incrementally() {
    let dir = Fixture::new();
    let store = Store::open(&dir.0.join("library.sqlite3")).unwrap();
    let mp3 = dir.0.join("mp3-fixture.mp3");
    let flac = dir.0.join("flac-fixture.flac");
    tagged_audio(&mp3, MP3, TagType::Id3v2, "合成甲曲");
    tagged_audio(&flac, FLAC, TagType::VorbisComments, "合成乙曲");
    let first = library::scan(&store, vec![root(&dir.0)], HashSet::new()).unwrap();
    assert_eq!(
        (
            first.file_count,
            first.updated_count,
            first.unresolved_count
        ),
        (2, 2, 0)
    );
    for title in ["合成甲曲", "合成乙曲"] {
        assert!(matches!(
            store
                .match_indexed(&Identity::new(title, ["虚构甲".into(), "虚构乙".into()]))
                .unwrap(),
            Match::Found(_)
        ));
    }
    assert_eq!(
        library::scan(&store, vec![root(&dir.0)], HashSet::new())
            .unwrap()
            .updated_count,
        0
    );
    tagged_audio(&flac, FLAC, TagType::VorbisComments, "修订后的合成乙曲");
    assert_eq!(
        library::scan(&store, vec![root(&dir.0)], HashSet::new())
            .unwrap()
            .updated_count,
        1
    );
    assert!(matches!(
        store
            .match_indexed(&Identity::new(
                "合成乙曲",
                ["虚构甲".into(), "虚构乙".into()]
            ))
            .unwrap(),
        Match::Missing
    ));
    assert!(matches!(
        store
            .match_indexed(&Identity::new(
                "修订后的合成乙曲",
                ["虚构甲".into(), "虚构乙".into()]
            ))
            .unwrap(),
        Match::Found(_)
    ));
}

#[test]
fn mp3_and_flac_tag_filename_conflicts_stay_pending_confirmation() {
    for (ext, bytes, kind) in [
        ("mp3", MP3, TagType::Id3v2),
        ("flac", FLAC, TagType::VorbisComments),
    ] {
        let dir = Fixture::new();
        let store = Store::open(&dir.0.join("library.sqlite3")).unwrap();
        let path = dir.0.join(format!("文件名曲 - 虚构甲、虚构乙.{ext}"));
        tagged_audio(&path, bytes, kind, "标签曲");
        library::scan(&store, vec![root(&dir.0)], HashSet::new()).unwrap();
        for title in ["标签曲", "文件名曲"] {
            assert!(
                matches!(store.match_indexed(&Identity::new(title, ["虚构甲".into(), "虚构乙".into()])).unwrap(), Match::Confirm(files, _) if files.len() == 1 && files[0].conflict)
            );
        }
        assert_eq!(
            store.issue_page(&queries::PageQuery::default()).unwrap()["total"],
            1
        );
    }
}

#[tokio::test]
async fn task_file_write_failure_rolls_back_creation_and_can_be_retried() {
    let dir = Fixture::new();
    let blocked = dir.0.join("blocked");
    let repository = Arc::new(JsonTaskRepository::new(blocked.join("tasks.json")));
    let monitors = MonitorService::with_remote(&dir.0, FakeRemote::new(vec![])).unwrap();
    let rt = ServerRuntime::test_runtime_with_repository(&dir.0, monitors, repository.clone());
    let mut m = rt.monitors.save(None, config("写入失败")).await.unwrap();
    rt.monitors
        .ingest(&mut m, vec![song("crashA", "虚构曲")], "、")
        .unwrap();
    // 真实文件占据父目录路径，确保所有平台都产生 OS 写入错误。
    std::fs::write(&blocked, b"fixture sentinel").unwrap();
    assert!(!rt
        .monitors
        .dispatch(&rt, entry(&rt.monitors, "crashA"), &Ok(()))
        .await
        .unwrap());
    assert!(rt.tasks.list().is_empty());
    assert_eq!(entry(&rt.monitors, "crashA").state, State::DownloadFailed);
    assert!(entry(&rt.monitors, "crashA").task_id.is_none());
    assert_eq!(std::fs::read(&blocked).unwrap(), b"fixture sentinel");
    std::fs::remove_file(&blocked).unwrap();
    rt.monitors
        .decide(&rt, "crashA", "retry", None, Some(&m.id))
        .await
        .unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(repository.load().unwrap().len(), 1);
    assert_eq!(entry(&rt.monitors, "crashA").state, State::Queued);
}

#[tokio::test]
async fn sqlite_full_preserves_ledger_stops_dispatch_and_recovers_from_task_snapshot() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![song("crashA", "虚构曲")]);
    let rt = runtime(&dir.0, remote.clone());
    rt.monitors.save(None, config("磁盘配额")).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    let original = entry(&rt.monitors, "crashA");
    let id = original.task_id.as_ref().unwrap();
    let maximum: u64 = {
        let db = rt.monitors.store.0.lock().unwrap();
        let maximum = db
            .query_row("PRAGMA max_page_count", [], |r| r.get(0))
            .unwrap();
        let pages: u64 = db.query_row("PRAGMA page_count", [], |r| r.get(0)).unwrap();
        db.execute_batch(&format!("PRAGMA max_page_count={pages};"))
            .unwrap();
        maximum
    };
    // 长路径只作为合成任务字段写入 JSON/SQLite，不在磁盘创建此路径。
    let final_path = format!("synthetic/{}.flac", "x".repeat(128 * 1024));
    rt.tasks.completed(id, &final_path);
    assert_eq!(rt.tasks.get(id).unwrap().status, TaskStatus::Completed);
    assert!(rt.monitors.persistence_failed.load(Ordering::Relaxed));
    assert!(rt
        .monitors
        .observe_task(&rt.tasks.get(id).unwrap())
        .unwrap_err()
        .contains("database or disk is full"));
    assert_eq!(json!(entry(&rt.monitors, "crashA")), json!(original));
    assert!(rt
        .monitors
        .tick(&rt)
        .await
        .unwrap_err()
        .contains("持久化失败"));
    {
        let db = rt.monitors.store.0.lock().unwrap();
        assert_eq!(
            db.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        db.execute_batch(&format!("PRAGMA max_page_count={maximum};"))
            .unwrap();
    }
    drop(rt);
    let restored = runtime(&dir.0, remote);
    restored.monitors.reconcile(&restored).await.unwrap();
    assert_eq!(entry(&restored.monitors, "crashA").state, State::Downloaded);
    assert_eq!(
        entry(&restored.monitors, "crashA").path.as_deref(),
        Some(final_path.as_str())
    );
    assert_eq!(restored.tasks.list().len(), 1);
}

/// 仅单独启动的测试子进程设置这些变量。正常测试和发布版不会暂停。
pub(crate) fn checkpoint(point: &str, db: Option<&rusqlite::Connection>) {
    if std::env::var("HOTDOWNLOADER_TEST_CRASH_POINT")
        .ok()
        .as_deref()
        != Some(point)
    {
        return;
    }
    let dir =
        PathBuf::from(std::env::var_os("HOTDOWNLOADER_TEST_CRASH_DIR").expect("child fixture"));
    let root = std::env::temp_dir()
        .join("hotdownloader-monitor-tests")
        .canonicalize()
        .unwrap();
    assert!(dir.starts_with(&root) && dir != root);
    if let Some(db) = db {
        assert!(
            !db.is_autocommit(),
            "checkpoint must be inside the ledger transaction"
        );
        db.cache_flush().unwrap();
    }
    let mut marker = std::fs::File::create(dir.join("checkpoint.tmp")).unwrap();
    marker
        .write_all(
            json!({"point": point, "pid": std::process::id()})
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    marker.sync_all().unwrap();
    drop(marker);
    std::fs::rename(dir.join("checkpoint.tmp"), dir.join("checkpoint.json")).unwrap();
    loop {
        std::thread::park();
    }
}

pub(crate) fn after_ledger_write(db: &rusqlite::Connection, kind: &str, key: &str, data: &str) {
    if kind != "entry"
        || key != "crashA"
        || std::env::var_os("HOTDOWNLOADER_TEST_CRASH_POINT").is_none()
    {
        return;
    }
    let value: Value = serde_json::from_str(data).unwrap();
    if value["state"] == "ignored" {
        checkpoint("ledger_batch_written", Some(db));
    }
    if value["state"] == "queued" {
        checkpoint("ledger_task_committed", None);
    }
}

struct CheckpointRepository(JsonTaskRepository);
impl TaskRepository for CheckpointRepository {
    fn load(&self) -> Result<Vec<TaskRecord>> {
        self.0.load()
    }
    fn save(&self, tasks: &[TaskRecord]) -> Result<()> {
        if tasks
            .iter()
            .any(|t| t.song_mid == "crashA" && t.status == TaskStatus::Waiting)
        {
            checkpoint("before_task_save", None);
            self.0.save(tasks)?;
            checkpoint("after_task_save", None);
            Ok(())
        } else {
            self.0.save(tasks)
        }
    }
}

#[test]
#[ignore = "only invoked as an isolated crash/recovery child by the parent tests"]
fn crash_child() {
    let dir =
        PathBuf::from(std::env::var_os("HOTDOWNLOADER_TEST_CRASH_DIR").expect("child fixture"));
    let mode = std::env::var("HOTDOWNLOADER_TEST_CRASH_POINT").expect("child mode");
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    executor.block_on(async {
        let monitors = MonitorService::with_remote(&dir, FakeRemote::new(vec![])).unwrap();
        let rt = ServerRuntime::test_runtime_with_repository(&dir, monitors, Arc::new(CheckpointRepository(JsonTaskRepository::new(dir.join("tasks.json")))));
        if mode == "recover" {
            for _ in 0..2 { rt.monitors.reconcile(&rt).await.unwrap(); rt.monitors.tick(&rt).await.unwrap(); }
            let entries = rt.monitors.store.all::<Entry>("entry").unwrap();
            let report = json!({"entries":entries,"tasks":rt.tasks.list(),"integrity":rt.monitors.store.0.lock().unwrap().query_row("PRAGMA integrity_check",[],|r| r.get::<_,String>(0)).unwrap()});
            std::fs::write(dir.join("recovered.json"), report.to_string()).unwrap();
            return;
        }
        let mut m = rt.monitors.save(None, config("崩溃恢复测试")).await.unwrap();
        let mut songs = vec![song("crashA", "虚构甲曲")];
        if mode.starts_with("ledger_batch") {
            songs.push(song("crashB", "虚构乙曲"));
            for song in &mut songs { song["album"] = json!("synthetic".repeat(2048)); }
        }
        rt.monitors.ingest(&mut m, songs, "、").unwrap();
        if mode.starts_with("ledger_batch") {
            for mid in ["crashA", "crashB"] { rt.monitors.store.change::<Entry>("entry",mid,|e|e.state=State::PendingConfirmation).unwrap(); }
            rt.monitors.store.0.lock().unwrap().execute_batch("PRAGMA cache_size=1;").unwrap();
            rt.monitors.decide_batch(&rt, &m.id, BatchDecision { mids: vec!["crashA".into(), "crashB".into()], action: "ignore".into() }).await.unwrap();
        } else {
            rt.monitors.dispatch(&rt, entry(&rt.monitors,"crashA"), &Ok(())).await.unwrap();
        }
        panic!("child did not reach checkpoint {mode}");
    });
}

struct ChildProcess(Child);
impl Drop for ChildProcess {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn spawn_child(dir: &Path, mode: &str) -> ChildProcess {
    let log = std::fs::File::create(dir.join(format!("{mode}.log"))).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            CHILD_TEST,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("HOTDOWNLOADER_TEST_CRASH_DIR", dir)
        .env("HOTDOWNLOADER_TEST_CRASH_POINT", mode)
        .env_remove("HOTDOWNLOADER_DOWNLOAD_DIR")
        .env_remove("HOTDOWNLOADER_DOWNLOAD_MOUNT_MARKER")
        .env_remove("HOTDOWNLOADER_SCAN_DIRS")
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    ChildProcess(command.spawn().unwrap())
}

fn crash_and_recover(point: &str) -> (Fixture, Value, Vec<TaskRecord>) {
    let dir = Fixture::new();
    let mut child = spawn_child(&dir.0, point);
    let deadline = Instant::now() + Duration::from_secs(15);
    while !dir.0.join("checkpoint.json").exists() {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "child exited before checkpoint: {}",
            std::fs::read_to_string(dir.0.join(format!("{point}.log"))).unwrap()
        );
        assert!(Instant::now() < deadline, "checkpoint timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    let checkpoint: Value =
        serde_json::from_slice(&std::fs::read(dir.0.join("checkpoint.json")).unwrap()).unwrap();
    assert_eq!(checkpoint["pid"], child.0.id());
    assert_eq!(checkpoint["point"], point);
    if point == "ledger_batch_written" {
        assert!(
            std::fs::metadata(dir.0.join("library.sqlite3-journal"))
                .unwrap()
                .len()
                > 0
        );
    }
    child.0.kill().unwrap();
    assert!(!child.0.wait().unwrap().success());
    let saved = JsonTaskRepository::new(dir.0.join("tasks.json"))
        .load()
        .unwrap();
    let mut recovery = spawn_child(&dir.0, "recover");
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = recovery.0.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "recovery timed out");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        status.success(),
        "recovery failed: {}",
        std::fs::read_to_string(dir.0.join("recover.log")).unwrap()
    );
    let report: Value =
        serde_json::from_slice(&std::fs::read(dir.0.join("recovered.json")).unwrap()).unwrap();
    assert_eq!(report["integrity"], "ok");
    (dir, report, saved)
}

#[test]
fn killed_before_task_save_requires_confirmation_without_duplicate_queue() {
    let (_dir, report, saved) = crash_and_recover("before_task_save");
    assert!(saved.is_empty());
    assert!(report["tasks"].as_array().unwrap().is_empty());
    assert_eq!(report["entries"][0]["state"], "pending_confirmation");
}

#[test]
fn killed_after_task_save_recovers_dispatch_window_with_same_task_id() {
    let (_dir, report, saved) = crash_and_recover("after_task_save");
    assert_eq!(saved.len(), 1);
    assert_eq!(report["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(report["tasks"][0]["id"], saved[0].id);
    assert_eq!(report["entries"][0]["taskId"], saved[0].id);
    assert_eq!(report["entries"][0]["state"], "queued");
    assert_eq!(report["entries"][0]["owned"], true);
}

#[test]
fn killed_after_ledger_task_commit_reuses_persisted_task() {
    let (_dir, report, saved) = crash_and_recover("ledger_task_committed");
    assert_eq!(saved.len(), 1);
    assert_eq!(report["tasks"].as_array().unwrap().len(), 1);
    assert_eq!(report["entries"][0]["taskId"], saved[0].id);
    assert_eq!(report["entries"][0]["state"], "queued");
}

#[test]
fn killed_during_ledger_transaction_rolls_back_entire_batch() {
    let (_dir, report, saved) = crash_and_recover("ledger_batch_written");
    assert!(saved.is_empty());
    let entries = report["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e["state"] == "pending_confirmation"));
}

#[test]
fn killed_after_ledger_transaction_retains_entire_batch() {
    let (_dir, report, saved) = crash_and_recover("ledger_batch_committed");
    assert!(saved.is_empty());
    let entries = report["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e["state"] == "ignored"));
}
