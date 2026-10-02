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
    path::{Path, PathBuf},
};
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScanRoot {
    pub path: String,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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
}

pub fn roots(download: &str, template: &str, artist_separator: &str) -> Result<Vec<ScanRoot>> {
    let extra = std::env::var("HOTDOWNLOADER_SCAN_DIRS").unwrap_or_else(|_| "[]".into());
    configured_roots(download, template, artist_separator, &extra)
}

pub fn configured_roots(
    download: &str,
    template: &str,
    artist_separator: &str,
    extra: &str,
) -> Result<Vec<ScanRoot>> {
    let mut roots = vec![ScanRoot {
        path: download.into(),
        template: template_regex(template).ok().map(|_| template.into()),
        artist_separator: artist_separator.into(),
    }];
    roots.extend(
        serde_json::from_str::<Vec<ScanRoot>>(extra)
            .map_err(|e| format!("HOTDOWNLOADER_SCAN_DIRS 必须是目录对象的 JSON 数组: {e}"))?,
    );
    let mut seen = HashSet::new();
    for root in &roots {
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
    let collect = || -> Result<(Vec<LocalFile>, usize)> {
        let mut files = Vec::new();
        let mut updated = 0;
        let mut seen = HashSet::new();
        // 更具体的根先处理，确保嵌套目录使用自己的模板。
        let mut ordered = roots.clone();
        ordered.sort_by_key(|r| std::cmp::Reverse(r.path.len()));
        for root in &ordered {
            let base = Path::new(&root.path)
                .canonicalize()
                .map_err(|e| format!("目录不可访问 {}: {e}", root.path))?;
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
        }
        Ok((files, updated))
    };
    match collect() {
        Ok((files, updated)) => {
            report.indexed_roots = roots;
            report.last_success = report.last_scan;
            report.file_count = files.len();
            report.updated_count = updated;
            report.unresolved_count = files
                .iter()
                .filter(|f| !f.identity.complete() || f.warning.is_some())
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
                if old
                    .get(&file.path)
                    .map(|p| serde_json::to_string(p).unwrap())
                    != Some(serde_json::to_string(file).unwrap())
                {
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
