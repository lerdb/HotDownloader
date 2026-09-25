use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::contract::TaskRecord;
use super::task_state::TaskRepository;

/// 普通文件系统实现，供独立 Rust 进程保存任务快照。
/// 文件写入同目录临时文件后替换目标，Linux 上 rename 保证读者只看到完整 JSON。
pub struct JsonTaskRepository {
    path: PathBuf,
}

impl JsonTaskRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl TaskRepository for JsonTaskRepository {
    fn load(&self) -> Result<Vec<TaskRecord>, String> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("读取任务文件失败: {error}")),
        };
        serde_json::from_slice(&bytes).map_err(|error| format!("解析任务文件失败: {error}"))
    }

    fn save(&self, tasks: &[TaskRecord]) -> Result<(), String> {
        // 先序列化并创建同目录临时文件，避免写入中断时截断现有记录。
        let json = serde_json::to_vec(tasks).map_err(|error| error.to_string())?;
        let parent = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|error| format!("创建任务目录失败: {error}"))?;

        let temp = parent.join(format!(".tasks-{:x}.tmp", rand::random::<u64>()));
        let result = (|| -> Result<(), String> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|error| format!("创建临时任务文件失败: {error}"))?;
            file.write_all(&json)
                .and_then(|_| file.sync_all())
                .map_err(|error| format!("写入任务文件失败: {error}"))?;

            // Docker 目标为 Linux，rename 可原子替换旧文件。Windows 标准库不支持
            // 覆盖现有目标，仅在这个未用于 Tauri 的文件仓库实现中回退为先删除旧文件。
            #[cfg(windows)]
            if self.path.exists() {
                fs::remove_file(&self.path)
                    .map_err(|error| format!("替换旧任务文件失败: {error}"))?;
            }
            fs::rename(&temp, &self.path).map_err(|error| format!("提交任务文件失败: {error}"))
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_reloads_task_snapshots() {
        let directory =
            std::env::temp_dir().join(format!("hotdownloader-core-{:x}", rand::random::<u64>()));
        let path = directory.join("tasks.json");
        let repository = JsonTaskRepository::new(&path);
        assert!(repository.load().unwrap().is_empty());

        let task: TaskRecord = serde_json::from_value(serde_json::json!({
            "id": "task-1", "platform": "qqmusic", "songId": 1, "songMid": "mid",
            "songTitle": "歌曲", "artist": "歌手", "album": "专辑",
            "filename": "song.mp3", "quality": "320kmp3", "status": "completed",
            "fileSize": 100, "downloaded": 100, "retryCount": 0, "addedAt": 1
        }))
        .unwrap();
        repository.save(&[task]).unwrap();

        let loaded = repository.load().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "task-1");
        repository.save(&loaded).unwrap();
        assert_eq!(repository.load().unwrap().len(), 1);

        fs::remove_dir_all(directory).unwrap();
    }
}
