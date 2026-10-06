use super::*;
use lofty::{
    config::WriteOptions,
    tag::{ItemKey, Tag, TagExt, TagType},
};
use std::{
    path::{Path, PathBuf},
    sync::{atomic::AtomicUsize, Mutex as SyncMutex},
};

struct Fixture(PathBuf);
static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join("hotdownloader-monitor-tests");
        let path = root.join(format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let root = std::env::temp_dir()
            .join("hotdownloader-monitor-tests")
            .canonicalize()
            .unwrap();
        assert!(self.0.starts_with(&root) && self.0 != root);
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn wave(path: &Path, title: Option<&str>, artist: Option<&str>) {
    // 4 个静音采样，完全合成的 PCM WAVE，不含真实歌曲。
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend(44u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(8000u32.to_le_bytes());
    bytes.extend(16000u32.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(8u32.to_le_bytes());
    bytes.extend([0u8; 8]);
    std::fs::write(path, bytes).unwrap();
    if title.is_some() || artist.is_some() {
        let mut tag = Tag::new(TagType::RiffInfo);
        if let Some(title) = title {
            tag.insert_text(ItemKey::TrackTitle, title.into());
        }
        if let Some(artist) = artist {
            tag.insert_text(ItemKey::TrackArtist, artist.into());
        }
        tag.save_to_path(path, WriteOptions::default()).unwrap();
    }
}

fn root(path: &Path) -> library::ScanRoot {
    library::ScanRoot {
        path: path.to_string_lossy().into(),
        template: Some("{song} - {artist}".into()),
        artist_separator: "、".into(),
    }
}
fn song(mid: &str, title: &str) -> Value {
    json!({"platform":"qqmusic", "id":1, "mid":mid, "title":title, "artist":"虚构乙、虚构甲", "album":"测试专辑",
        "artists":[{"name":"虚构乙"},{"name":"虚构甲"}], "qualities":[{"quality":"320kmp3","filename":"fixture.mp3","size":52}]})
}
fn config(name: &str) -> MonitorInput {
    MonitorInput {
        name: name.into(),
        source: Source::Public,
        playlist_id: "123456".into(),
        dirid: "".into(),
        quality: "320kmp3".into(),
        interval_minutes: 60,
        enabled: true,
    }
}

struct FakeRemote {
    songs: SyncMutex<Vec<Value>>,
    auth_error: SyncMutex<Option<String>>,
    calls: AtomicUsize,
}
impl FakeRemote {
    fn new(songs: Vec<Value>) -> Arc<Self> {
        Arc::new(Self {
            songs: SyncMutex::new(songs),
            auth_error: SyncMutex::new(None),
            calls: AtomicUsize::new(0),
        })
    }
}
impl MonitorRemote for FakeRemote {
    fn fetch<'a>(
        &'a self,
        _: &'a Monitor,
        _: &'a ServerRuntime,
    ) -> BoxFuture<'a, Result<Vec<Value>>> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let songs = self.songs.lock().unwrap().clone();
        Box::pin(async move { Ok(songs) })
    }
    fn authenticate<'a>(&'a self, _: &'a ServerRuntime) -> BoxFuture<'a, Result<()>> {
        let error = self.auth_error.lock().unwrap().clone();
        Box::pin(async move { error.map_or(Ok(()), Err) })
    }
}
fn runtime(dir: &Path, remote: Arc<FakeRemote>) -> Arc<ServerRuntime> {
    let monitors = MonitorService::with_remote(dir, remote).unwrap();
    ServerRuntime::test_runtime(dir, monitors)
}
fn entry(service: &MonitorService, mid: &str) -> Entry {
    service.store.get("entry", mid).unwrap().unwrap()
}

#[test]
fn directory_status_counts_nested_roots_and_preserves_index_on_failure() {
    let dir = Fixture::new();
    let child = dir.0.join("nested");
    std::fs::create_dir(&child).unwrap();
    let store = Store::open(&dir.0.join("library.sqlite3")).unwrap();
    wave(&dir.0.join("甲 - 乙.wav"), None, None);
    wave(&child.join("丙 - 丁.wav"), None, None);
    let roots = vec![root(&dir.0), root(&child)];
    let report = library::scan(&store, roots.clone(), HashSet::new()).unwrap();
    assert_eq!(report.file_count, 2);
    assert!(report
        .directories
        .iter()
        .all(|d| d.file_count == 1 && d.state == "healthy" && d.indexed_at > 0));
    std::fs::write(child.join("unknown.mp3"), b"synthetic invalid audio").unwrap();
    let mut failed_roots = roots.clone();
    failed_roots.push(root(&dir.0.join("missing")));
    assert!(library::scan(&store, failed_roots, HashSet::new()).is_err());
    let failed = store.get::<ScanReport>("config", "scan").unwrap().unwrap();
    assert_eq!(failed.file_count, 2);
    assert_eq!(store.all::<LocalFile>("file").unwrap().len(), 2);
    let nested = failed
        .directories
        .iter()
        .find(|d| d.path == child.to_string_lossy())
        .unwrap();
    assert_eq!(nested.file_count, 1);
    assert_eq!(nested.observed_count, Some(2));
    assert_eq!(nested.warning_count, 1);
    assert_eq!(nested.state, "warning");
    let missing = failed
        .directories
        .iter()
        .find(|d| d.state == "unavailable")
        .unwrap();
    assert!(missing.error.is_some());
    assert_eq!(missing.observed_count, None);
    assert_eq!(missing.indexed_at, 0);
    let restored = library::scan(&store, roots, HashSet::new()).unwrap();
    assert_eq!(restored.file_count, 3);
    assert!(restored.error.is_none());
    assert_eq!(
        restored
            .directories
            .iter()
            .map(|d| d.file_count)
            .sum::<usize>(),
        3
    );
}

#[tokio::test]
async fn rounds_record_new_members_links_and_actual_queue_submissions() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(
        (0..25)
            .map(|i| song(&format!("mid{i}"), &format!("曲{i}")))
            .collect(),
    );
    let rt = runtime(&dir.0, remote.clone());
    wave(
        &Path::new(rt.environment.default_download_dir()).join("matched.wav"),
        Some("曲0"),
        Some("虚构甲、虚构乙"),
    );
    let m = rt.monitors.save(None, config("统计")).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    let history = rt.monitors.history(&m.id).unwrap();
    assert_eq!(history[0]["added"], 25);
    assert_eq!(history[0]["linked"], 1);
    assert_eq!(history[0]["enqueued"], 20);
    assert_eq!(history[0]["status"], "completed");
    assert_eq!(
        rt.monitors.list().unwrap()["monitors"][0]["initialProgress"]["completed"],
        1
    );
    rt.monitors.tick(&rt).await.unwrap();
    let history = rt.monitors.history(&m.id).unwrap();
    assert_eq!(history[0]["trigger"], "backfill");
    assert_eq!(history[0]["added"], 0);
    assert_eq!(history[0]["linked"], 0);
    assert_eq!(history[0]["enqueued"], 4);
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(
        rt.monitors
            .history(&m.id)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    remote.songs.lock().unwrap().push(song("new", "新曲"));
    rt.monitors.request_check(&m.id).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    let history = rt.monitors.history(&m.id).unwrap();
    assert_eq!(history[0]["trigger"], "manual");
    assert_eq!(history[0]["added"], 1);
    assert_eq!(history[0]["enqueued"], 1);
    assert_eq!(
        rt.monitors.list().unwrap()["monitors"][0]["initialProgress"]["total"],
        25
    );
    drop(rt);
    let rt = runtime(&dir.0, remote);
    assert_eq!(rt.monitors.history(&m.id).unwrap(), history);
}

#[tokio::test]
async fn history_records_failure_and_retains_bounded_restart_safe_rounds() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![json!({"mid":"invalid"})]);
    let rt = runtime(&dir.0, remote.clone());
    let m = rt.monitors.save(None, config("失败历史")).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(rt.monitors.history(&m.id).unwrap()[0]["status"], "failed");
    assert_eq!(
        rt.monitors.list().unwrap()["monitors"][0]["initialProgress"]["initialized"],
        false
    );
    let seeded: Vec<_> = (0..100)
        .map(|i| CheckRecord {
            started_at: i,
            status: "completed".into(),
            ..Default::default()
        })
        .collect();
    rt.monitors.store.put("history", &m.id, &seeded).unwrap();
    remote.songs.lock().unwrap().clear();
    rt.monitors.request_check(&m.id).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    let history = rt.monitors.history(&m.id).unwrap();
    assert_eq!(history.as_array().unwrap().len(), 100);
    assert_eq!(history[99]["startedAt"], 1);
    assert_eq!(
        rt.monitors.list().unwrap()["monitors"][0]["initialProgress"]["percent"],
        100
    );
    rt.monitors
        .store
        .change::<Vec<CheckRecord>>("history", &m.id, |h| {
            h.last_mut().unwrap().status = "running".into()
        })
        .unwrap();
    drop(rt);
    let rt = runtime(&dir.0, remote);
    assert_eq!(
        rt.monitors.history(&m.id).unwrap()[0]["status"],
        "interrupted"
    );
    rt.monitors.delete(&m.id).await.unwrap();
    assert!(rt
        .monitors
        .store
        .get::<Vec<CheckRecord>>("history", &m.id)
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn initial_progress_tracks_decisions_errors_removals_and_legacy_records() {
    let dir = Fixture::new();
    let service = MonitorService::open(&dir.0).unwrap();
    let mut m = service.save(None, config("进度")).await.unwrap();
    let mut legacy = json!(m);
    legacy.as_object_mut().unwrap().remove("initialMembers");
    let legacy: Monitor = serde_json::from_value(legacy).unwrap();
    assert_eq!(initial_progress(&legacy, &[])["initialized"], false);
    service
        .ingest(
            &mut m,
            (0..5).map(|i| song(&format!("mid{i}"), "曲")).collect(),
            "、",
        )
        .unwrap();
    for (mid, state) in [
        ("mid0", State::Ignored),
        ("mid1", State::Downloaded),
        ("mid2", State::PendingConfirmation),
        ("mid3", State::NoQuality),
    ] {
        service
            .store
            .change::<Entry>("entry", mid, |e| e.state = state)
            .unwrap();
    }
    m.members.retain(|mid| mid != "mid4");
    let progress = initial_progress(&m, &service.store.all::<Entry>("entry").unwrap());
    assert_eq!(progress["total"], 5);
    assert_eq!(progress["completed"], 2);
    assert_eq!(progress["removed"], 1);
    assert_eq!(progress["confirmation"], 1);
    assert_eq!(progress["failed"], 1);
    assert_eq!(progress["percent"], 60);
}

fn batch(mids: &[&str], action: &str) -> BatchDecision {
    BatchDecision {
        mids: mids.iter().map(|s| s.to_string()).collect(),
        action: action.into(),
    }
}

#[tokio::test]
async fn duplicate_sources_and_concurrent_creation_are_rejected() {
    let dir = Fixture::new();
    let service = MonitorService::open(&dir.0).unwrap();
    let (a, b) = tokio::join!(
        service.save(None, config("甲")),
        service.save(None, config("乙"))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let existing = a.or(b).unwrap();
    service
        .save(Some(&existing.id), existing.config.clone())
        .await
        .unwrap();
    let mut created = config("个人歌单");
    created.source = Source::Created;
    created.playlist_id = "000123456".into();
    created.dirid = "1".into();
    service.save(None, created.clone()).await.unwrap();
    created.playlist_id = "123456".into();
    created.dirid = "01".into();
    assert!(service
        .save(None, created.clone())
        .await
        .unwrap_err()
        .contains("已有监控"));
    created.dirid = "2".into();
    service.save(None, created).await.unwrap();
    let mut liked = config("我喜欢");
    liked.source = Source::Liked;
    liked.playlist_id.clear();
    service.save(None, liked.clone()).await.unwrap();
    liked.source = Source::Created;
    liked.dirid = "201".into();
    liked.playlist_id = "98765".into();
    assert!(service.save(None, liked).await.is_err());
}

#[tokio::test]
async fn batch_decisions_validate_all_members_and_preserve_shared_choices() {
    let dir = Fixture::new();
    let rt = runtime(&dir.0, FakeRemote::new(vec![]));
    let mut m = rt.monitors.save(None, config("批量")).await.unwrap();
    rt.monitors
        .ingest(&mut m, vec![song("a", "甲"), song("b", "乙")], "、")
        .unwrap();
    rt.monitors
        .store
        .change::<Entry>("entry", "a", |e| e.state = State::PendingConfirmation)
        .unwrap();
    for request in [
        batch(&[], "ignore"),
        batch(&["a"], "link"),
        batch(&["a", "foreign"], "ignore"),
        batch(&["a", "b"], "ignore"),
    ] {
        assert!(rt.monitors.decide_batch(&rt, &m.id, request).await.is_err());
        assert_eq!(entry(&rt.monitors, "a").state, State::PendingConfirmation);
    }
    assert!(rt
        .monitors
        .decide_batch(&rt, &m.id, batch(&vec!["a"; 201], "ignore"))
        .await
        .is_err());
    rt.monitors
        .store
        .change::<Entry>("entry", "b", |e| e.state = State::PendingConfirmation)
        .unwrap();
    rt.monitors.store.0.lock().unwrap().execute_batch(
        "CREATE TRIGGER reject_batch_write BEFORE UPDATE ON records
         WHEN NEW.kind='entry' AND NEW.key='b' BEGIN SELECT RAISE(ABORT, 'simulated disk failure'); END;"
    ).unwrap();
    assert!(rt
        .monitors
        .decide_batch(&rt, &m.id, batch(&["a", "b"], "ignore"))
        .await
        .is_err());
    assert_eq!(entry(&rt.monitors, "a").state, State::PendingConfirmation);
    assert_eq!(entry(&rt.monitors, "b").state, State::PendingConfirmation);
    rt.monitors
        .store
        .0
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER reject_batch_write;")
        .unwrap();
    assert_eq!(
        rt.monitors
            .decide_batch(&rt, &m.id, batch(&["a", "b", "a"], "ignore"))
            .await
            .unwrap(),
        2
    );
    assert_eq!(entry(&rt.monitors, "a").state, State::Ignored);
    assert_eq!(entry(&rt.monitors, "b").state, State::Ignored);
    assert!(rt
        .monitors
        .decide_batch(&rt, &m.id, batch(&["a"], "download"))
        .await
        .is_err());
}

#[tokio::test]
async fn explicit_download_only_dispatches_selected_songs_after_restart() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![]);
    let rt = runtime(&dir.0, remote.clone());
    let mut m = rt.monitors.save(None, config("停用歌单")).await.unwrap();
    rt.monitors
        .ingest(
            &mut m,
            vec![song("a", "甲"), song("b", "乙"), song("c", "丙")],
            "、",
        )
        .unwrap();
    m.config.enabled = false;
    rt.monitors
        .save(Some(&m.id), m.config.clone())
        .await
        .unwrap();
    rt.monitors
        .store
        .change::<Entry>("entry", "a", |e| e.state = State::PendingConfirmation)
        .unwrap();
    rt.monitors
        .decide_batch(&rt, &m.id, batch(&["a"], "download"))
        .await
        .unwrap();
    rt.monitors
        .decide(&rt, "b", "download", None)
        .await
        .unwrap();
    assert!(
        !rt.monitors
            .store
            .get::<Monitor>("monitor", &m.id)
            .unwrap()
            .unwrap()
            .draining
    );
    drop(rt);
    let rt = runtime(&dir.0, remote.clone());
    rt.monitors.tick(&rt).await.unwrap();
    let mids: HashSet<_> = rt.tasks.list().into_iter().map(|t| t.song_mid).collect();
    assert_eq!(mids, HashSet::from(["a".to_string(), "b".to_string()]));
    assert_eq!(entry(&rt.monitors, "c").state, State::Pending);
    assert_eq!(remote.calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn batch_download_uses_selected_monitors_quality() {
    let dir = Fixture::new();
    let rt = runtime(&dir.0, FakeRemote::new(vec![]));
    let mut a = rt.monitors.save(None, config("甲")).await.unwrap();
    let mut other = config("乙");
    other.playlist_id = "98765".into();
    other.quality = "flac".into();
    let mut b = rt.monitors.save(None, other).await.unwrap();
    for m in [&mut a, &mut b] {
        rt.monitors.ingest(m, vec![song("a", "甲")], "、").unwrap();
    }
    rt.monitors
        .store
        .change::<Entry>("entry", "a", |e| e.state = State::PendingConfirmation)
        .unwrap();
    rt.monitors
        .decide_batch(&rt, &b.id, batch(&["a"], "download"))
        .await
        .unwrap();
    assert_eq!(entry(&rt.monitors, "a").quality, "flac");
}

#[tokio::test]
async fn delete_preserves_files_tasks_and_ledger_and_stops_future_dispatch() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![]);
    let rt = runtime(&dir.0, remote.clone());
    let mut m = rt.monitors.save(None, config("删除测试")).await.unwrap();
    rt.monitors
        .ingest(&mut m, vec![song("a", "甲"), song("b", "乙")], "、")
        .unwrap();
    m.config.enabled = false;
    rt.monitors
        .save(Some(&m.id), m.config.clone())
        .await
        .unwrap();
    rt.monitors
        .decide(&rt, "a", "download", None)
        .await
        .unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    let file = dir.0.join("keep.wav");
    wave(&file, None, None);
    rt.monitors
        .decide(&rt, "b", "download", None)
        .await
        .unwrap();
    let mut other = config("共享歌单");
    other.playlist_id = "98765".into();
    other.enabled = false;
    let mut shared = rt.monitors.save(None, other).await.unwrap();
    rt.monitors
        .ingest(&mut shared, vec![song("a", "甲"), song("b", "乙")], "、")
        .unwrap();
    rt.monitors
        .save(Some(&shared.id), shared.config.clone())
        .await
        .unwrap();
    let entries = rt.monitors.songs(&m.id).unwrap();
    rt.monitors.delete(&m.id).await.unwrap();
    assert!(rt.monitors.songs(&m.id).is_err());
    assert!(rt.monitors.delete(&m.id).await.is_err());
    assert_eq!(rt.monitors.songs(&shared.id).unwrap(), entries);
    assert!(file.is_file());
    assert_eq!(rt.tasks.list().len(), 1);
    assert_eq!(
        json!(rt.monitors.store.all::<Entry>("entry").unwrap()),
        entries
    );
    drop(rt);
    let rt = runtime(&dir.0, remote);
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(rt.tasks.list().len(), 1);
    assert_eq!(entry(&rt.monitors, "b").state, State::Ready);
    let mut recreated = rt.monitors.save(None, config("重新添加")).await.unwrap();
    rt.monitors
        .ingest(&mut recreated, vec![song("a", "甲")], "、")
        .unwrap();
    assert_eq!(
        entry(&rt.monitors, "a").task_id,
        rt.tasks.list().first().map(|t| t.id.clone())
    );
}

#[tokio::test]
async fn exhausted_fallback_updates_actual_quality_and_stops_monitor_retries() {
    let dir = Fixture::new();
    let rt = runtime(&dir.0, FakeRemote::new(vec![song("mid1", "虚构曲")]));
    rt.monitors.save(None, config("歌单")).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    let id = rt.tasks.list()[0].id.clone();
    rt.tasks
        .update(&id, true, |task| task.quality = "flac".into())
        .unwrap();
    rt.tasks.failed(
        &id,
        "音质候选已耗尽，最后音质 flac 获取链接失败: 模拟拒绝",
        None,
    );
    let saved = entry(&rt.monitors, "mid1");
    assert_eq!(saved.quality, "flac");
    assert_eq!(saved.next_retry, 0);
    assert!(!eligible(&saved));
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(rt.tasks.list().len(), 1);
    assert_eq!(rt.tasks.get(&id).unwrap().status, TaskStatus::Error);
}

#[test]
fn identity_is_unicode_normalized_artist_set_but_keeps_versions() {
    let a = Identity::new(
        "  ＳＯＮＧ   (Live) ",
        ["甲".into(), "乙".into(), "甲".into()],
    );
    assert_eq!(a, Identity::new("song (live)", ["乙".into(), "甲".into()]));
    assert_ne!(a, Identity::new("song", ["甲".into(), "乙".into()]));
    assert_ne!(a, Identity::new("song (remix)", ["甲".into(), "乙".into()]));
}

#[test]
fn scan_reads_tags_falls_back_and_is_incremental() {
    let dir = Fixture::new();
    let store = Store::open(&dir.0.join("library.sqlite3")).unwrap();
    let audio = dir.0.join("虚构曲 - 虚构甲、虚构乙.wav");
    wave(&audio, Some("虚构曲"), Some("虚构乙、虚构甲"));
    let roots = vec![root(&dir.0)];
    let first = library::scan(&store, roots.clone(), HashSet::new()).unwrap();
    assert_eq!((first.file_count, first.updated_count), (1, 1));
    let files = store.all::<LocalFile>("file").unwrap();
    assert!(!files[0].conflict);
    assert!(matches!(
        library::match_song(
            &Identity::new("虚构曲", ["虚构甲".into(), "虚构乙".into()]),
            &files
        ),
        Match::Found(_)
    ));
    assert_eq!(
        library::scan(&store, roots.clone(), HashSet::new())
            .unwrap()
            .updated_count,
        0
    );
    let second = dir.0.join("另一首 - 虚构甲.wav");
    wave(&second, None, None);
    let next = library::scan(&store, roots.clone(), HashSet::new()).unwrap();
    assert_eq!((next.file_count, next.updated_count), (2, 1));
    let files = store.all::<LocalFile>("file").unwrap();
    assert!(matches!(
        library::match_song(&Identity::new("另一首", ["虚构甲".into()]), &files),
        Match::Found(_)
    ));
    wave(&audio, Some("标签已修改成另一标题"), Some("虚构甲"));
    assert_eq!(
        library::scan(&store, roots.clone(), HashSet::new())
            .unwrap()
            .updated_count,
        1
    );
    std::fs::remove_file(second).unwrap();
    assert_eq!(
        library::scan(&store, roots, HashSet::new())
            .unwrap()
            .file_count,
        1
    );
}

#[test]
fn scan_failure_preserves_previous_index() {
    let dir = Fixture::new();
    let store = Store::open(&dir.0.join("library.sqlite3")).unwrap();
    wave(&dir.0.join("虚构曲 - 虚构甲.wav"), None, None);
    library::scan(&store, vec![root(&dir.0)], HashSet::new()).unwrap();
    let result = library::scan(
        &store,
        vec![root(&dir.0), root(&dir.0.join("unavailable"))],
        HashSet::new(),
    );
    assert!(result.is_err());
    assert_eq!(store.all::<LocalFile>("file").unwrap().len(), 1);
    let report: ScanReport = store.get("config", "scan").unwrap().unwrap();
    assert_eq!(report.file_count, 1);
    assert!(report.error.is_some());
}

#[test]
fn conflicts_duplicates_and_unknown_audio_require_confirmation() {
    let dir = Fixture::new();
    let store = Store::open(&dir.0.join("library.sqlite3")).unwrap();
    wave(
        &dir.0.join("文件名曲 - 虚构甲.wav"),
        Some("标签曲"),
        Some("虚构甲"),
    );
    library::scan(&store, vec![root(&dir.0)], HashSet::new()).unwrap();
    let files = store.all::<LocalFile>("file").unwrap();
    for title in ["文件名曲", "标签曲"] {
        assert!(matches!(
            library::match_song(&Identity::new(title, ["虚构甲".into()]), &files),
            Match::Confirm(_, _)
        ));
    }
    wave(
        &dir.0.join("匹配曲 - 虚构甲.wav"),
        Some("匹配曲"),
        Some("虚构甲"),
    );
    let other = dir.0.join("other");
    std::fs::create_dir(&other).unwrap();
    wave(
        &other.join("匹配曲 - 虚构甲.wav"),
        Some("匹配曲"),
        Some("虚构甲"),
    );
    library::scan(&store, vec![root(&dir.0)], HashSet::new()).unwrap();
    assert!(
        matches!(library::match_song(&Identity::new("匹配曲", ["虚构甲".into()]), &store.all::<LocalFile>("file").unwrap()), Match::Confirm(c, _) if c.len() == 2)
    );
    std::fs::write(dir.0.join("unknown.mp3"), b"not an audio file").unwrap();
    library::scan(&store, vec![root(&dir.0)], HashSet::new()).unwrap();
    assert!(matches!(
        library::match_song(
            &Identity::new("不存在", ["虚构甲".into()]),
            &store.all::<LocalFile>("file").unwrap()
        ),
        Match::Confirm(_, _)
    ));
}

#[test]
fn templates_validate_and_config_changes_reparse() {
    let dir = Fixture::new();
    assert!(library::configured_roots(dir.0.to_str().unwrap(), "{song}", "、", "[]").is_ok());
    assert!(library::configured_roots(
        dir.0.to_str().unwrap(),
        "{song} - {artist}",
        "、",
        r#"[{"path":"relative"}]"#
    )
    .is_err());
    let store = Store::open(&dir.0.join("library.sqlite3")).unwrap();
    wave(&dir.0.join("甲 - 乙.wav"), None, None);
    library::scan(&store, vec![root(&dir.0)], HashSet::new()).unwrap();
    let mut changed = root(&dir.0);
    changed.template = Some("{artist} - {song}".into());
    let report = library::scan(&store, vec![changed], HashSet::new()).unwrap();
    assert_eq!(report.updated_count, 1);
    assert_eq!(
        store.all::<LocalFile>("file").unwrap()[0].identity.title,
        "乙"
    );
}

#[tokio::test]
async fn snapshot_deduplicates_across_playlists_and_keeps_removed_decisions() {
    let dir = Fixture::new();
    let service = MonitorService::open(&dir.0).unwrap();
    let mut a = service.save(None, config("甲歌单")).await.unwrap();
    let mut other = config("乙歌单");
    other.playlist_id = "654321".into();
    let mut b = service.save(None, other).await.unwrap();
    service
        .ingest(
            &mut a,
            vec![song("mid1", "曲甲"), song("mid1", "曲甲")],
            "、",
        )
        .unwrap();
    service
        .store
        .change::<Entry>("entry", "mid1", |e| e.state = State::Ignored)
        .unwrap();
    service
        .ingest(
            &mut b,
            vec![song("mid1", "曲甲"), song("mid2", "曲乙")],
            "、",
        )
        .unwrap();
    service.ingest(&mut a, vec![], "、").unwrap();
    assert_eq!(entry(&service, "mid1").state, State::Ignored);
    assert_eq!(service.store.all::<Entry>("entry").unwrap().len(), 2);
    assert_eq!(b.members.len(), 2);
    drop(service);
    assert_eq!(
        entry(&MonitorService::open(&dir.0).unwrap(), "mid1").state,
        State::Ignored
    );
}

#[tokio::test]
async fn batches_share_tasks_and_only_new_songs_are_added() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(
        (0..45)
            .map(|i| song(&format!("mid{i}"), &format!("虚构曲{i}")))
            .collect(),
    );
    let rt = runtime(&dir.0, remote.clone());
    let a = rt.monitors.save(None, config("歌单甲")).await.unwrap();
    let mut other = config("歌单乙");
    other.playlist_id = "654321".into();
    rt.monitors.save(None, other).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(rt.tasks.list().len(), BATCH_SIZE);
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(rt.tasks.list().len(), 40);
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(rt.tasks.list().len(), 45);
    remote.songs.lock().unwrap().push(song("newmid", "新增曲"));
    rt.monitors.request_check(&a.id).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(rt.tasks.list().len(), 46);
    assert_eq!(remote.calls.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn completed_decision_survives_task_removal_and_restart() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![song("mid1", "虚构曲")]);
    let rt = runtime(&dir.0, remote.clone());
    let m = rt.monitors.save(None, config("歌单")).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    let task = rt.tasks.list()[0].clone();
    rt.tasks.completed(&task.id, "file-was-moved.mp3");
    rt.tasks.remove(&task.id).unwrap();
    assert_eq!(entry(&rt.monitors, "mid1").state, State::Downloaded);
    drop(rt);
    let rt = runtime(&dir.0, remote);
    rt.monitors.request_check(&m.id).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert!(rt.tasks.list().is_empty());
    rt.monitors
        .decide(&rt, "mid1", "reset", None)
        .await
        .unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(rt.tasks.list().len(), 1);
}

#[tokio::test]
async fn restart_resumes_only_owned_unfinished_tasks() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![song("mid1", "虚构曲")]);
    let rt = runtime(&dir.0, remote.clone());
    rt.monitors.save(None, config("歌单")).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    let original = rt.tasks.list()[0].id.clone();
    let service = TaskService::new(&rt.tasks, &rt.engine, rt.environment.as_ref());
    service
        .create_download_task(CreateTaskRequest {
            song: serde_json::from_value(song("manualmid", "手动曲")).unwrap(),
            desired_quality: "320kmp3".into(),
            duplicate_action: None,
        })
        .await
        .unwrap();
    drop(rt);
    let rt = runtime(&dir.0, remote);
    assert!(rt
        .tasks
        .list()
        .iter()
        .all(|t| t.status == TaskStatus::Interrupted));
    rt.monitors.reconcile(&rt).await.unwrap();
    assert_eq!(rt.tasks.get(&original).unwrap().status, TaskStatus::Waiting);
    assert_eq!(
        rt.tasks
            .list()
            .iter()
            .find(|t| t.song_mid == "manualmid")
            .unwrap()
            .status,
        TaskStatus::Interrupted
    );
}

#[tokio::test]
async fn removed_unfinished_task_and_dispatch_crash_require_confirmation() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![song("mid1", "虚构曲")]);
    let rt = runtime(&dir.0, remote);
    rt.monitors.save(None, config("歌单")).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    rt.tasks.remove(&rt.tasks.list()[0].id).unwrap();
    rt.monitors.reconcile(&rt).await.unwrap();
    assert_eq!(
        entry(&rt.monitors, "mid1").state,
        State::PendingConfirmation
    );
    rt.monitors
        .store
        .change::<Entry>("entry", "mid1", |e| {
            e.state = State::Dispatching;
            e.task_id = None;
        })
        .unwrap();
    rt.monitors.reconcile(&rt).await.unwrap();
    assert_eq!(
        entry(&rt.monitors, "mid1").state,
        State::PendingConfirmation
    );
}

#[tokio::test]
async fn errors_retry_same_task_with_backoff_and_stop_after_three_retries() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![song("mid1", "虚构曲")]);
    let rt = runtime(&dir.0, remote);
    rt.monitors.save(None, config("歌单")).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    let id = rt.tasks.list()[0].id.clone();
    for retry in 0..=3 {
        rt.tasks.failed(&id, "虚构下载失败", None);
        let e = entry(&rt.monitors, "mid1");
        assert_eq!(e.retries, retry);
        rt.monitors.tick(&rt).await.unwrap(); // 到时间前不会自动重试。
        assert_eq!(rt.tasks.get(&id).unwrap().status, TaskStatus::Error);
        if retry < 3 {
            assert!(e.next_retry > now());
            rt.monitors
                .store
                .change::<Entry>("entry", "mid1", |e| e.next_retry = 1)
                .unwrap();
            rt.monitors.tick(&rt).await.unwrap();
            assert_eq!(rt.tasks.list().len(), 1);
            assert_eq!(rt.tasks.get(&id).unwrap().status, TaskStatus::Waiting);
        } else {
            assert_eq!(e.next_retry, 0);
        }
    }
    let m = rt
        .monitors
        .store
        .all::<Monitor>("monitor")
        .unwrap()
        .remove(0);
    let history = rt.monitors.history(&m.id).unwrap();
    assert_eq!(history.as_array().unwrap().len(), 4);
    assert!(history
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["enqueued"] == 1));
}

#[tokio::test]
async fn credentials_and_unavailable_quality_do_not_create_task_storms() {
    let dir = Fixture::new();
    let mut unavailable = song("noquality", "无音质曲");
    unavailable["qualities"] = json!([]);
    let remote = FakeRemote::new(vec![song("noauth", "凭据测试曲"), unavailable]);
    *remote.auth_error.lock().unwrap() = Some("虚构凭据失效".into());
    let rt = runtime(&dir.0, remote);
    rt.monitors.save(None, config("歌单")).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(
        entry(&rt.monitors, "noauth").state,
        State::CredentialInvalid
    );
    assert_eq!(entry(&rt.monitors, "noquality").state, State::NoQuality);
    for _ in 0..3 {
        rt.monitors.tick(&rt).await.unwrap();
    }
    assert!(rt.tasks.list().is_empty());
    assert_eq!(entry(&rt.monitors, "noauth").retries, 0);
    let m = rt
        .monitors
        .store
        .all::<Monitor>("monitor")
        .unwrap()
        .remove(0);
    let history = rt.monitors.history(&m.id).unwrap();
    assert_eq!(history[0]["enqueued"], 0);
    assert_eq!(history[0]["failed"], 2);
    assert_eq!(history[0]["status"], "warning");
}

#[tokio::test]
async fn ambiguous_song_supports_link_ignore_download_without_overwriting_files() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![song("mid1", "虚构曲")]);
    let rt = runtime(&dir.0, remote);
    rt.monitors.save(None, config("歌单")).await.unwrap();
    let path = Path::new(rt.environment.default_download_dir()).join("虚构曲 - 虚构乙、虚构甲.wav");
    wave(&path, Some("不一致标题"), Some("虚构甲、虚构乙"));
    let original_bytes = std::fs::read(&path).unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(
        entry(&rt.monitors, "mid1").state,
        State::PendingConfirmation
    );
    assert!(rt.tasks.list().is_empty());
    rt.monitors
        .decide(&rt, "mid1", "link", Some(path.to_string_lossy().into()))
        .await
        .unwrap();
    assert_eq!(entry(&rt.monitors, "mid1").state, State::Matched);
    rt.monitors
        .decide(&rt, "mid1", "ignore", None)
        .await
        .unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert!(rt.tasks.list().is_empty());
    rt.monitors
        .decide(&rt, "mid1", "download", None)
        .await
        .unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(rt.tasks.list().len(), 1);
    assert_eq!(std::fs::read(path).unwrap(), original_bytes);
}

#[tokio::test]
async fn missing_directory_prevents_queue_and_keeps_playlist_snapshot() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![song("mid1", "虚构曲")]);
    let rt = runtime(&dir.0, remote.clone());
    rt.monitors.save(None, config("歌单")).await.unwrap();
    std::fs::remove_dir(rt.environment.default_download_dir()).unwrap();
    assert!(rt.monitors.tick(&rt).await.is_err());
    assert!(rt.tasks.list().is_empty());
    assert_eq!(remote.calls.load(Ordering::Relaxed), 0);
    let m = rt
        .monitors
        .store
        .all::<Monitor>("monitor")
        .unwrap()
        .remove(0);
    let history = rt.monitors.history(&m.id).unwrap();
    assert_eq!(history[0]["status"], "failed");
    assert_eq!(history[0]["added"], 0);
    assert_eq!(history[0]["enqueued"], 0);
    assert!(history[0]["message"]
        .as_str()
        .unwrap()
        .contains("目录不可访问"));
}

#[tokio::test]
async fn disabled_monitor_is_idle_until_explicit_single_check() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![song("mid1", "虚构曲")]);
    let rt = runtime(&dir.0, remote.clone());
    let mut input = config("停用歌单");
    input.enabled = false;
    let m = rt.monitors.save(None, input).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(remote.calls.load(Ordering::Relaxed), 0);
    rt.monitors.request_check(&m.id).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(rt.tasks.list().len(), 1);
}

#[tokio::test]
async fn invalid_quality_and_partial_snapshot_do_not_overwrite_good_state() {
    let dir = Fixture::new();
    let service = MonitorService::open(&dir.0).unwrap();
    let mut input = config("测试");
    input.quality = "ask".into();
    assert!(service.save(None, input).await.is_err());
    let mut m = service.save(None, config("正常歌单")).await.unwrap();
    service
        .ingest(&mut m, vec![song("mid1", "虚构曲")], "、")
        .unwrap();
    assert!(service
        .ingest(
            &mut m,
            vec![song("mid2", "新增曲"), json!({"mid":"invalid"})],
            "、"
        )
        .is_err());
    assert_eq!(service.store.all::<Entry>("entry").unwrap().len(), 1);
    assert_eq!(
        service
            .store
            .get::<Monitor>("monitor", &m.id)
            .unwrap()
            .unwrap()
            .members,
        vec!["mid1"]
    );
}

#[test]
fn failed_scan_does_not_mark_new_template_as_indexed() {
    let dir = Fixture::new();
    let store = Store::open(&dir.0.join("library.sqlite3")).unwrap();
    wave(&dir.0.join("甲 - 乙.wav"), None, None);
    library::scan(&store, vec![root(&dir.0)], HashSet::new()).unwrap();
    let mut changed = root(&dir.0);
    changed.template = Some("{artist} - {song}".into());
    assert!(library::scan(
        &store,
        vec![changed.clone(), root(&dir.0.join("offline"))],
        HashSet::new()
    )
    .is_err());
    let report = library::scan(&store, vec![changed], HashSet::new()).unwrap();
    assert_eq!(report.updated_count, 1);
    assert_eq!(
        store.all::<LocalFile>("file").unwrap()[0].identity.title,
        "乙"
    );
}

#[tokio::test]
async fn matched_file_can_disappear_without_redownload() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![song("mid1", "虚构曲")]);
    let rt = runtime(&dir.0, remote.clone());
    let m = rt.monitors.save(None, config("歌单")).await.unwrap();
    let file = Path::new(rt.environment.default_download_dir()).join("虚构曲 - 虚构甲、虚构乙.wav");
    wave(&file, None, None);
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(entry(&rt.monitors, "mid1").state, State::Matched);
    std::fs::remove_file(&file).unwrap();
    drop(rt);
    let rt = runtime(&dir.0, remote);
    rt.monitors.request_check(&m.id).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    assert_eq!(entry(&rt.monitors, "mid1").state, State::Matched);
    assert!(rt.tasks.list().is_empty());
}

#[tokio::test]
async fn manual_completion_while_awaiting_confirmation_is_remembered() {
    let dir = Fixture::new();
    let rt = runtime(&dir.0, FakeRemote::new(vec![]));
    let mut m = rt.monitors.save(None, config("歌单")).await.unwrap();
    rt.monitors
        .ingest(&mut m, vec![song("mid1", "虚构曲")], "、")
        .unwrap();
    rt.monitors
        .store
        .change::<Entry>("entry", "mid1", |e| e.state = State::PendingConfirmation)
        .unwrap();
    let service = TaskService::new(&rt.tasks, &rt.engine, rt.environment.as_ref());
    let result = service
        .create_download_task(CreateTaskRequest {
            song: serde_json::from_value(song("mid1", "虚构曲")).unwrap(),
            desired_quality: "320kmp3".into(),
            duplicate_action: None,
        })
        .await
        .unwrap();
    let CreateTaskResult::Created { task } = result else {
        panic!("expected task")
    };
    rt.tasks.completed(&task.id, "moved-away.mp3");
    rt.tasks.remove(&task.id).unwrap();
    assert_eq!(entry(&rt.monitors, "mid1").state, State::Downloaded);
}

#[tokio::test]
async fn explicit_pause_is_preserved_across_restart() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![song("mid1", "虚构曲")]);
    let rt = runtime(&dir.0, remote.clone());
    rt.monitors.save(None, config("歌单")).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    let id = rt.tasks.list()[0].id.clone();
    TaskService::new(&rt.tasks, &rt.engine, rt.environment.as_ref())
        .pause_task(id.clone())
        .await
        .unwrap();
    drop(rt);
    let rt = runtime(&dir.0, remote);
    rt.monitors.reconcile(&rt).await.unwrap();
    assert_eq!(rt.tasks.get(&id).unwrap().status, TaskStatus::Interrupted);
    assert_eq!(entry(&rt.monitors, "mid1").state, State::Paused);
}

#[tokio::test]
async fn recovery_does_not_attach_an_old_task_to_a_new_dispatch_intent() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![song("mid1", "虚构曲")]);
    let rt = runtime(&dir.0, remote);
    rt.monitors.save(None, config("歌单")).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    let task = rt.tasks.list()[0].clone();
    rt.tasks.completed(&task.id, "old.mp3");
    rt.monitors
        .store
        .change::<Entry>("entry", "mid1", |e| {
            e.state = State::Dispatching;
            e.task_id = None;
            e.force_download = true;
            e.dispatch_started_at = task.added_at + 1;
        })
        .unwrap();
    rt.monitors.reconcile(&rt).await.unwrap();
    assert_eq!(
        entry(&rt.monitors, "mid1").state,
        State::PendingConfirmation
    );
}

#[tokio::test]
async fn malformed_playlist_is_reported_and_waits_until_next_check() {
    let dir = Fixture::new();
    let remote = FakeRemote::new(vec![json!({"mid":"incomplete"})]);
    let rt = runtime(&dir.0, remote.clone());
    let m = rt.monitors.save(None, config("歌单")).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    rt.monitors.tick(&rt).await.unwrap();
    let stored = rt
        .monitors
        .store
        .get::<Monitor>("monitor", &m.id)
        .unwrap()
        .unwrap();
    assert_eq!(stored.last_state, "check_failed");
    assert!(stored.next_check > now());
    assert_eq!(remote.calls.load(Ordering::Relaxed), 1);
    assert!(rt.tasks.list().is_empty());
}

#[tokio::test]
async fn interval_edits_and_fresh_quality_data_preserve_decisions() {
    let dir = Fixture::new();
    let service = MonitorService::open(&dir.0).unwrap();
    let mut m = service.save(None, config("歌单")).await.unwrap();
    service
        .ingest(&mut m, vec![song("mid1", "虚构曲")], "、")
        .unwrap();
    service
        .store
        .change::<Entry>("entry", "mid1", |e| e.state = State::NoQuality)
        .unwrap();
    let mut fresh = song("mid1", "虚构曲");
    fresh["qualities"] = json!([{"quality":"flac","filename":"fiction.flac","size":100}]);
    service.ingest(&mut m, vec![fresh], "、").unwrap();
    assert_eq!(entry(&service, "mid1").state, State::NoQuality);
    assert_eq!(
        entry(&service, "mid1").song["qualities"][0]["quality"],
        "flac"
    );
    let mut input = m.config.clone();
    input.interval_minutes = 5;
    let edited = service.save(Some(&m.id), input).await.unwrap();
    assert_eq!(edited.next_check, m.last_check + 300);
}

#[tokio::test]
async fn stale_dispatch_cannot_overwrite_a_concurrent_manual_success() {
    let dir = Fixture::new();
    let rt = runtime(&dir.0, FakeRemote::new(vec![]));
    let mut m = rt.monitors.save(None, config("歌单")).await.unwrap();
    rt.monitors
        .ingest(&mut m, vec![song("mid1", "虚构曲")], "、")
        .unwrap();
    rt.monitors
        .store
        .change::<Entry>("entry", "mid1", |e| e.state = State::Ready)
        .unwrap();
    let stale = entry(&rt.monitors, "mid1");
    let result = TaskService::new(&rt.tasks, &rt.engine, rt.environment.as_ref())
        .create_download_task(CreateTaskRequest {
            song: serde_json::from_value(song("mid1", "虚构曲")).unwrap(),
            desired_quality: "320kmp3".into(),
            duplicate_action: None,
        })
        .await
        .unwrap();
    let CreateTaskResult::Created { task } = result else {
        panic!("expected task")
    };
    rt.tasks.completed(&task.id, "gone.mp3");
    rt.tasks.remove(&task.id).unwrap();
    rt.monitors.dispatch(&rt, stale, &Ok(())).await.unwrap();
    assert!(rt.tasks.list().is_empty());
    assert_eq!(entry(&rt.monitors, "mid1").state, State::Downloaded);
}
