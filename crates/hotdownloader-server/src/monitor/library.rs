use super::{
    now,
    store::{self, Result, Store},
};
use lofty::{
    file::TaggedFileExt,
    tag::{Accessor, ItemKey},
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
};
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScanRoot {
    pub path: String,
    /// 挂载盘内预先创建的标记文件，相对于扫描根目录。
    #[serde(default)]
    pub mount_marker: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default = "separator")]
    pub artist_separator: String,
}
fn separator() -> String {
    "、".into()
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Identity {
    pub title: String,
    pub artists: BTreeSet<String>,
}

pub fn normalize(value: &str) -> String {
    value
        .nfkc()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

impl Identity {
    pub fn new(title: &str, artists: impl IntoIterator<Item = String>) -> Self {
        Self {
            title: normalize(title),
            artists: artists
                .into_iter()
                .map(|a| normalize(&a))
                .filter(|a| !a.is_empty())
                .collect(),
        }
    }
    fn complete(&self) -> bool {
        !self.title.is_empty() && !self.artists.is_empty()
    }
    fn overlaps(&self, other: &Self) -> bool {
        !self.title.is_empty()
            && self.title == other.title
            && (self.artists.is_empty() || self.artists == other.artists)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalFile {
    pub path: String,
    pub root: String,
    pub size: u64,
    pub modified: String,
    pub metadata: Identity,
    pub filename: Identity,
    pub identity: Identity,
    pub conflict: bool,
    pub warning: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub roots: Vec<ScanRoot>,
    #[serde(default)]
    pub indexed_roots: Vec<ScanRoot>,
    pub file_count: usize,
    pub updated_count: usize,
    pub last_scan: u64,
    pub last_success: u64,
    pub error: Option<String>,
    pub unresolved_count: usize,
    #[serde(default)]
    pub directories: Vec<DirectoryStatus>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryStatus {
    pub path: String,
    pub file_count: usize,
    pub observed_count: Option<usize>,
    pub warning_count: usize,
    pub state: String,
    pub error: Option<String>,
    pub checked_at: u64,
    pub indexed_at: u64,
}

pub fn roots(download: &str, template: &str, artist_separator: &str) -> Result<Vec<ScanRoot>> {
    let extra = std::env::var("HOTDOWNLOADER_SCAN_DIRS").unwrap_or_else(|_| "[]".into());
    let mut roots = configured_roots(download, template, artist_separator, &extra)?;
    if let Some(marker) = std::env::var("HOTDOWNLOADER_DOWNLOAD_MOUNT_MARKER")
        .ok()
        .filter(|value| !value.is_empty())
    {
        validate_marker(&marker)?;
        roots[0].mount_marker = Some(marker);
    }
    Ok(roots)
}

fn validate_marker(marker: &str) -> Result<()> {
    if marker.is_empty()
        || Path::new(marker)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("挂载标记必须是扫描目录内的相对文件路径，不能包含 ..".into());
    }
    Ok(())
}

fn check_marker(base: &Path, marker: &str) -> Result<()> {
    validate_marker(marker)?;
    let path = base
        .join(marker)
        .canonicalize()
        .map_err(|e| format!("挂载标记不可访问 {marker}: {e}"))?;
    if !path.starts_with(base) {
        return Err("挂载标记必须位于扫描目录内".into());
    }
    let mut file =
        std::fs::File::open(&path).map_err(|e| format!("挂载标记不可读取 {marker}: {e}"))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("挂载标记必须是普通文件".into());
    }
    let count = file
        .read(&mut [0u8; 1])
        .map_err(|e| format!("挂载标记不可读取 {marker}: {e}"))?;
    if count == 0 && file.metadata().map_err(|e| e.to_string())?.len() > 0 {
        return Err("挂载标记内容不可读取".into());
    }
    Ok(())
}

/// 缓存命中时只探测根目录和挂载标记，不递归遍历整库。
pub fn check_cached_roots(report: &ScanReport) -> Result<()> {
    for root in &report.roots {
        let base = Path::new(&root.path)
            .canonicalize()
            .map_err(|e| format!("目录不可访问 {}: {e}", root.path))?;
        if let Some(marker) = &root.mount_marker {
            check_marker(&base, marker)?;
        }
        let mut entries =
            std::fs::read_dir(&base).map_err(|e| format!("目录不可访问 {}: {e}", root.path))?;
        match entries.next() {
            Some(Err(error)) => return Err(format!("目录不可访问 {}: {error}", root.path)),
            None if root.mount_marker.is_none()
                && report
                    .directories
                    .iter()
                    .any(|d| d.path == root.path && d.file_count > 0) =>
            {
                return Err(format!("目录 {} 已变为空目录，疑似挂载异常", root.path));
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn configured_roots(
    download: &str,
    template: &str,
    artist_separator: &str,
    extra: &str,
) -> Result<Vec<ScanRoot>> {
    let mut roots = vec![ScanRoot {
        path: download.into(),
        mount_marker: None,
        template: template_regex(template).ok().map(|_| template.into()),
        artist_separator: artist_separator.into(),
    }];
    roots.extend(
        serde_json::from_str::<Vec<ScanRoot>>(extra)
            .map_err(|e| format!("HOTDOWNLOADER_SCAN_DIRS 必须是目录对象的 JSON 数组: {e}"))?,
    );
    let mut seen = HashSet::new();
    for root in &roots {
        if let Some(marker) = &root.mount_marker {
            validate_marker(marker)?;
        }
        if !Path::new(&root.path).is_absolute() {
            return Err("扫描目录必须是容器内绝对路径".into());
        }
        if !seen.insert(PathBuf::from(&root.path)) {
            return Err("扫描目录重复".into());
        }
        if root.artist_separator.is_empty() {
            return Err("歌手分隔符不能为空".into());
        }
        if let Some(template) = &root.template {
            template_regex(template)?;
        }
    }
    Ok(roots)
}

fn template_regex(template: &str) -> Result<Regex> {
    let token = Regex::new(r"\{([^{}]+)\}").unwrap();
    let mut pattern = String::from("^");
    let mut end = 0;
    let mut names = HashSet::new();
    for capture in token.captures_iter(template) {
        let part = capture.get(0).unwrap();
        let name = &capture[1];
        if !["song", "artist", "album", "quality"].contains(&name)
            || !names.insert(name.to_string())
        {
            return Err("模板只支持不重复的 {song}、{artist}、{album}、{quality}".into());
        }
        if end > 0 && end == part.start() {
            return Err("模板变量间必须有分隔文字".into());
        }
        pattern.push_str(&regex::escape(&template[end..part.start()]));
        pattern.push_str(&format!("(?P<{name}>.+?)"));
        end = part.end();
    }
    if !names.contains("song") || !names.contains("artist") {
        return Err("扫描模板必须包含 {song} 和 {artist}".into());
    }
    pattern.push_str(&regex::escape(&template[end..]));
    pattern.push('$');
    Regex::new(&pattern).map_err(|e| e.to_string())
}

fn from_filename(path: &Path, root: &ScanRoot) -> Identity {
    let Some(template) = root.template.as_deref() else {
        return Identity::default();
    };
    let Ok(regex) = template_regex(template) else {
        return Identity::default();
    };
    let Some(stem) = path.file_stem().and_then(|p| p.to_str()) else {
        return Identity::default();
    };
    let Some(parts) = regex.captures(stem) else {
        return Identity::default();
    };
    Identity::new(
        &parts["song"],
        parts["artist"]
            .split(&root.artist_separator)
            .map(str::to_string),
    )
}

fn read_file(path: &Path, root: &ScanRoot, size: u64, modified: String) -> LocalFile {
    let filename = from_filename(path, root);
    let mut metadata = Identity::default();
    let mut warning = None;
    match lofty::read_from_path(path) {
        Ok(audio) => {
            if let Some(tag) = audio.primary_tag().or_else(|| audio.first_tag()) {
                let artists = tag
                    .get_strings(&ItemKey::TrackArtist)
                    .flat_map(|s| s.split(&root.artist_separator))
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                metadata = Identity::new(tag.title().as_deref().unwrap_or(""), artists);
            }
        }
        Err(e) => warning = Some(format!("无法读取音频标签: {e}")),
    }
    let conflict = (!metadata.title.is_empty()
        && !filename.title.is_empty()
        && metadata.title != filename.title)
        || (!metadata.artists.is_empty()
            && !filename.artists.is_empty()
            && metadata.artists != filename.artists);
    let identity = Identity {
        title: if metadata.title.is_empty() {
            filename.title.clone()
        } else {
            metadata.title.clone()
        },
        artists: if metadata.artists.is_empty() {
            filename.artists.clone()
        } else {
            metadata.artists.clone()
        },
    };
    LocalFile {
        path: path.to_string_lossy().into(),
        root: root.path.clone(),
        size,
        modified,
        metadata,
        filename,
        identity,
        conflict,
        warning,
    }
}

fn walk(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for item in
        std::fs::read_dir(path).map_err(|e| format!("目录不可访问 {}: {e}", path.display()))?
    {
        let item = item.map_err(|e| e.to_string())?;
        let kind = item.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            walk(&item.path(), files)?;
        } else if kind.is_file() {
            let path = item.path();
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if [
                "mp3", "flac", "m4a", "mp4", "aac", "ogg", "opus", "ape", "wav", "aiff", "aif",
                "wv",
            ]
            .contains(&ext.as_str())
            {
                files.push(path);
            }
        }
    }
    Ok(())
}

/// 整轮遍历成功才提交删除和更新；断挂载或子目录权限错误不会清空索引。
pub fn scan(store: &Store, roots: Vec<ScanRoot>, excluded: HashSet<String>) -> Result<ScanReport> {
    let mut report = store
        .get::<ScanReport>("config", "scan")?
        .unwrap_or_default();
    let old_roots = report.indexed_roots.clone();
    report.roots = roots.clone();
    report.last_scan = now();
    let old: HashMap<String, LocalFile> = store
        .all::<LocalFile>("file")?
        .into_iter()
        .map(|f| (f.path.clone(), f))
        .collect();
    let mut directories = Vec::new();
    let mut scan_errors = Vec::new();
    let mut collect = || -> Result<(Vec<LocalFile>, usize)> {
        let mut files = Vec::new();
        let mut updated = 0;
        let mut seen = HashSet::new();
        // 更具体的根先处理，确保嵌套目录使用自己的模板。
        let mut ordered = roots.clone();
        ordered.sort_by_key(|r| std::cmp::Reverse(r.path.len()));
        for root in &ordered {
            let previous = report.directories.iter().find(|d| d.path == root.path);
            let mut directory = DirectoryStatus {
                path: root.path.clone(),
                file_count: old.values().filter(|f| f.root == root.path).count(),
                indexed_at: previous.map(|d| d.indexed_at).unwrap_or_else(|| {
                    if old_roots.iter().any(|r| r.path == root.path) {
                        report.last_success
                    } else {
                        0
                    }
                }),
                checked_at: report.last_scan,
                ..Default::default()
            };
            let start = files.len();
            let result = (|| -> Result<()> {
                let base = Path::new(&root.path)
                    .canonicalize()
                    .map_err(|e| format!("目录不可访问 {}: {e}", root.path))?;
                if let Some(marker) = &root.mount_marker {
                    check_marker(&base, marker)?;
                }
                let mut paths = Vec::new();
                walk(&base, &mut paths)?;
                for path in paths {
                    let key = path.to_string_lossy().to_string();
                    if excluded.contains(&key) || !seen.insert(key.clone()) {
                        continue;
                    }
                    let info =
                        std::fs::metadata(&path).map_err(|e| format!("文件不可访问 {key}: {e}"))?;
                    let modified = info
                        .modified()
                        .map_err(|e| e.to_string())?
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|e| e.to_string())?
                        .as_nanos()
                        .to_string();
                    if let Some(previous) = old.get(&key).filter(|f| {
                        f.size == info.len()
                            && f.modified == modified
                            && f.root == root.path
                            && old_roots.contains(root)
                    }) {
                        files.push(previous.clone());
                    } else {
                        files.push(read_file(&path, root, info.len(), modified));
                        updated += 1;
                    }
                }
                directory.observed_count = Some(files.len() - start);
                if let Some(marker) = &root.mount_marker {
                    // 遍历期间也可能发生卸载，提交前再次检查。
                    check_marker(&base, marker)?;
                } else if directory.file_count > 0
                    && (files.len() - start) <= directory.file_count / 5
                {
                    return Err(format!("目录 {} 音频数量从 {} 骤减至 {}，疑似挂载异常；保留索引并暂停本轮补齐。若为主动清理，请配置盘内挂载标记后重新扫描", root.path, directory.file_count, files.len() - start));
                }
                Ok(())
            })();
            match result {
                Ok(()) => {
                    directory.observed_count = Some(files.len() - start);
                    directory.warning_count = files[start..]
                        .iter()
                        .filter(|f| f.warning.is_some() || f.conflict || !f.identity.complete())
                        .count();
                    directory.state = if directory.warning_count > 0 {
                        "warning"
                    } else {
                        "healthy"
                    }
                    .into();
                }
                Err(error) => {
                    directory.state = "unavailable".into();
                    directory.error = Some(error.clone());
                    scan_errors.push(error);
                }
            }
            directories.push(directory);
        }
        if scan_errors.is_empty() {
            Ok((files, updated))
        } else {
            Err(scan_errors.join("；"))
        }
    };
    let result = collect();
    report.directories = directories;
    match result {
        Ok((files, updated)) => {
            for directory in &mut report.directories {
                directory.file_count = directory.observed_count.unwrap_or(0);
                directory.indexed_at = report.last_scan;
            }
            report.indexed_roots = roots;
            report.last_success = report.last_scan;
            report.file_count = files.len();
            report.updated_count = updated;
            report.unresolved_count = files
                .iter()
                .filter(|f| !f.identity.complete() || f.warning.is_some() || f.conflict)
                .count();
            report.error = None;
            let mut db = store.0.lock().unwrap();
            let tx = db.transaction().map_err(|e| e.to_string())?;
            let current: HashSet<_> = files.iter().map(|f| f.path.as_str()).collect();
            for key in old.keys().filter(|key| !current.contains(key.as_str())) {
                tx.execute("DELETE FROM records WHERE kind='file' AND key=?", [key])
                    .map_err(|e| e.to_string())?;
            }
            for file in &files {
                // 标签只增量读取，数据库同样只更新实际改变的条目。
                if old.get(&file.path) != Some(file) {
                    store::write(&tx, "file", &file.path, file)?;
                }
            }
            store::write(&tx, "config", "scan", &report)?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(report)
        }
        Err(error) => {
            report.error = Some(error.clone());
            store.put("config", "scan", &report)?;
            Err(error)
        }
    }
}

pub enum Match {
    Found(String),
    Missing,
    Confirm(Vec<LocalFile>, String),
}

/// 关联前重新打开并核对索引快照，不能仅凭可读取的目录或过期标签认定文件存在。
pub fn verify_file(file: &LocalFile) -> Result<()> {
    let check = || -> std::io::Result<bool> {
        let mut opened = std::fs::File::open(&file.path)?;
        let count = opened.read(&mut [0u8; 1])?;
        let info = opened.metadata()?;
        let modified = info
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_nanos()
            .to_string();
        Ok(info.is_file()
            && info.len() == file.size
            && modified == file.modified
            && (file.size == 0 || count == 1))
    };
    match check() {
        Ok(true) => Ok(()),
        _ => Err("候选文件已不可读取或发生变化，请重新扫描后确认".into()),
    }
}

pub fn match_song(identity: &Identity, files: &[LocalFile]) -> Match {
    if !identity.complete() {
        return Match::Confirm(vec![], "在线歌曲缺少标题或歌手".into());
    }
    let candidates: Vec<_> = files
        .iter()
        .filter(|f| {
            f.identity.overlaps(identity)
                || f.metadata.overlaps(identity)
                || f.filename.overlaps(identity)
        })
        .cloned()
        .collect();
    if candidates.len() == 1
        && candidates[0].identity == *identity
        && !candidates[0].conflict
        && candidates[0].warning.is_none()
    {
        if let Err(error) = verify_file(&candidates[0]) {
            return Match::Confirm(candidates, error);
        }
        return Match::Found(candidates[0].path.clone());
    }
    if !candidates.is_empty() {
        return Match::Confirm(
            candidates,
            "元数据与文件名冲突、标签不完整或存在多个候选文件".into(),
        );
    }
    if files
        .iter()
        .any(|f| !f.identity.complete() || f.warning.is_some())
    {
        return Match::Confirm(
            vec![],
            "音乐库存在无法识别的音频文件，请确认是否需要下载".into(),
        );
    }
    Match::Missing
}
