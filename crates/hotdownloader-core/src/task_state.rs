use std::sync::{Arc, Mutex};

use tokio::sync::Mutex as AsyncMutex;

use super::contract::{TaskRecord, TaskStatus};

/// 任务记录的持久化端口。实现可以是 Tauri Store 或普通 JSON 文件。
pub trait TaskRepository: Send + Sync {
    fn load(&self) -> Result<Vec<TaskRecord>, String>;
    fn save(&self, tasks: &[TaskRecord]) -> Result<(), String>;
}

/// 任务状态输出端口。传输层负责把完整记录推送给自己的订阅者。
pub trait TaskEventSink: Send + Sync {
    fn updated(&self, task: TaskRecord);
    fn removed(&self, task_id: &str);
}

/// Rust 持有任务记录和状态流转；前端通过快照及事件维护只读投影。
/// 所有持久化写入集中在这里，避免窗口关闭后任务状态丢失。
pub struct TaskState {
    repository: Arc<dyn TaskRepository>,
    events: Arc<dyn TaskEventSink>,
    tasks: Mutex<Vec<TaskRecord>>,
    // 串行化创建、重试和删除，防止并发任务抢占同一个下载路径。
    pub creation_lock: AsyncMutex<()>,
}

impl TaskState {
    pub fn load(
        repository: Arc<dyn TaskRepository>,
        events: Arc<dyn TaskEventSink>,
    ) -> Result<Self, String> {
        let mut tasks = repository.load()?;

        // 进程重启后下载器上下文已经不存在，不能把旧的进行中状态继续展示为运行中。
        // 保留记录并转为错误，用户可以显式重试，由重试命令重新建立下载器上下文。
        let mut interrupted = false;
        for task in &mut tasks {
            if matches!(
                task.status,
                TaskStatus::Waiting
                    | TaskStatus::Downloading
                    | TaskStatus::Processing
                    | TaskStatus::Paused
            ) {
                task.status = TaskStatus::Error;
                task.error_msg = Some("应用关闭导致中断".into());
                task.speed = None;
                task.downloaded = 0;
                interrupted = true;
            }
        }
        if interrupted {
            repository.save(&tasks)?;
        }
        Ok(Self {
            repository,
            events,
            tasks: Mutex::new(tasks),
            creation_lock: AsyncMutex::new(()),
        })
    }

    pub fn list(&self) -> Vec<TaskRecord> {
        self.tasks.lock().unwrap().clone()
    }

    pub fn get(&self, id: &str) -> Option<TaskRecord> {
        self.tasks
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.id == id)
            .cloned()
    }

    pub fn path_reserved(&self, path: &str) -> bool {
        // 磁盘文件尚未出现时，排队中的任务也已经占用了目标路径。
        self.tasks.lock().unwrap().iter().any(|task| {
            task.status != TaskStatus::Completed && task.save_path.as_deref() == Some(path)
        })
    }

    pub fn insert(&self, task: TaskRecord) -> Result<(), String> {
        let mut tasks = self.tasks.lock().unwrap();
        if tasks.iter().any(|t| t.id == task.id) {
            return Err("任务 ID 已存在".into());
        }
        tasks.push(task.clone());
        // 先写盘再广播：前端收到事件时，重新加载得到的必须是同一份状态。
        // 写盘失败则回滚内存，避免出现只能在当前窗口看到的“幽灵任务”。
        if let Err(e) = self.persist(&tasks) {
            tasks.pop();
            return Err(e);
        }
        self.events.updated(task);
        drop(tasks);
        Ok(())
    }

    pub fn update(
        &self,
        id: &str,
        persist: bool,
        change: impl FnOnce(&mut TaskRecord),
    ) -> Result<TaskRecord, String> {
        let mut tasks = self.tasks.lock().unwrap();
        let index = tasks.iter().position(|t| t.id == id).ok_or("任务不存在")?;
        let old = tasks[index].clone();
        change(&mut tasks[index]);
        if persist {
            // 稳定的状态变化必须落盘；高频进度则由调用方选择只更新内存。
            if let Err(e) = self.persist(&tasks) {
                tasks[index] = old;
                return Err(e);
            }
        }
        let task = tasks[index].clone();
        self.events.updated(task.clone());
        drop(tasks);
        Ok(task)
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut tasks = self.tasks.lock().unwrap();
        let index = tasks.iter().position(|t| t.id == id).ok_or("任务不存在")?;
        let old = tasks.remove(index);
        // 删除失败时恢复原位置，任务列表与磁盘保持一致。
        if let Err(e) = self.persist(&tasks) {
            tasks.insert(index, old);
            return Err(e);
        }
        self.events.removed(id);
        drop(tasks);
        Ok(())
    }

    fn persist(&self, tasks: &[TaskRecord]) -> Result<(), String> {
        self.repository.save(tasks)
    }

    pub fn progress(&self, id: &str, downloaded: u64, total: u64, speed: u64) {
        let mut tasks = self.tasks.lock().unwrap();
        let Some(index) = tasks.iter().position(|task| task.id == id) else {
            return;
        };
        // 迟到的进度事件不得把已结束或正在处理元数据的任务重新改回下载中。
        if matches!(
            tasks[index].status,
            TaskStatus::Completed | TaskStatus::Error | TaskStatus::Processing
        ) {
            return;
        }
        // 仓库每次保存都会重新写入整张任务表。进度事件很频繁，
        // 所以仅在首次进度触发 Waiting -> Downloading 时持久化；
        // 后续 downloaded/speed 更新内存和前端投影，不做每片段整表写盘。
        let needs_persist = tasks[index].status == TaskStatus::Waiting;
        let previous = tasks[index].clone();
        {
            let task = &mut tasks[index];
            task.downloaded = downloaded;
            task.file_size = total;
            task.speed = Some(speed);
            if task.status == TaskStatus::Waiting {
                task.status = TaskStatus::Downloading;
            }
            task.error_msg = None;
        }
        if needs_persist {
            if let Err(e) = self.persist(&tasks) {
                tasks[index] = previous;
                log::warn!("更新下载进度失败 {id}: {e}");
                return;
            }
        }
        self.events.updated(tasks[index].clone());
    }

    pub fn file_complete(&self, id: &str) {
        // 文件传输结束后还有元数据等收尾操作，不能提前标记为 Completed。
        if let Err(e) = self.update(id, true, |task| {
            task.status = TaskStatus::Processing;
            task.downloaded = task.file_size;
            task.speed = None;
        }) {
            log::warn!("更新任务处理状态失败 {id}: {e}");
        }
    }

    pub fn completed(&self, id: &str, path: &str) {
        // 只有收尾成功后才记录最终文件路径，供“打开文件位置”和删除文件使用。
        if let Err(e) = self.update(id, true, |task| {
            task.status = TaskStatus::Completed;
            task.file_path = Some(path.to_string());
            task.downloaded = task.file_size;
            task.speed = None;
            task.error_msg = None;
        }) {
            log::warn!("更新任务完成状态失败 {id}: {e}");
        }
    }

    pub fn failed(&self, id: &str, message: &str, offset: Option<u64>) {
        // offset 来自下载器时优先使用；缺省时保留现有进度，重试时再核对实际文件。
        if let Err(e) = self.update(id, true, |task| {
            task.status = TaskStatus::Error;
            task.error_msg = Some(message.to_string());
            task.speed = None;
            if let Some(offset) = offset {
                task.downloaded = offset;
            }
        }) {
            log::warn!("更新任务错误状态失败 {id}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::*;

    struct MemoryRepository {
        rows: Mutex<Vec<TaskRecord>>,
        saves: AtomicUsize,
        fail_save: AtomicBool,
    }

    impl MemoryRepository {
        fn new(rows: Vec<TaskRecord>) -> Self {
            Self {
                rows: Mutex::new(rows),
                saves: AtomicUsize::new(0),
                fail_save: AtomicBool::new(false),
            }
        }
    }

    impl TaskRepository for MemoryRepository {
        fn load(&self) -> Result<Vec<TaskRecord>, String> {
            Ok(self.rows.lock().unwrap().clone())
        }

        fn save(&self, tasks: &[TaskRecord]) -> Result<(), String> {
            if self.fail_save.load(Ordering::SeqCst) {
                return Err("模拟写盘失败".into());
            }
            *self.rows.lock().unwrap() = tasks.to_vec();
            self.saves.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordedEvents(Mutex<Vec<String>>);

    impl TaskEventSink for RecordedEvents {
        fn updated(&self, task: TaskRecord) {
            self.0.lock().unwrap().push(format!("updated:{}", task.id));
        }

        fn removed(&self, task_id: &str) {
            self.0.lock().unwrap().push(format!("removed:{task_id}"));
        }
    }

    fn task(status: TaskStatus) -> TaskRecord {
        let mut task: TaskRecord = serde_json::from_value(serde_json::json!({
            "id": "task-1", "platform": "qqmusic", "songId": 1, "songMid": "mid",
            "songTitle": "歌曲", "artist": "歌手", "album": "专辑",
            "filename": "song.mp3", "quality": "320kmp3", "status": "waiting",
            "fileSize": 100, "downloaded": 0, "retryCount": 0, "addedAt": 1
        }))
        .unwrap();
        task.status = status;
        task
    }

    #[test]
    fn restart_marks_interrupted_tasks_and_saves_without_emitting() {
        let repository = Arc::new(MemoryRepository::new(vec![task(TaskStatus::Downloading)]));
        let events = Arc::new(RecordedEvents::default());

        let state = TaskState::load(repository.clone(), events.clone()).unwrap();

        assert_eq!(state.list()[0].status, TaskStatus::Error);
        assert_eq!(repository.saves.load(Ordering::SeqCst), 1);
        assert!(events.0.lock().unwrap().is_empty());
    }

    #[test]
    fn failed_save_rolls_back_and_does_not_emit() {
        let repository = Arc::new(MemoryRepository::new(Vec::new()));
        let events = Arc::new(RecordedEvents::default());
        let state = TaskState::load(repository.clone(), events.clone()).unwrap();
        repository.fail_save.store(true, Ordering::SeqCst);

        assert!(state.insert(task(TaskStatus::Waiting)).is_err());
        assert!(state.list().is_empty());
        assert!(events.0.lock().unwrap().is_empty());
    }

    #[test]
    fn progress_persists_only_the_first_state_transition() {
        let repository = Arc::new(MemoryRepository::new(Vec::new()));
        let events = Arc::new(RecordedEvents::default());
        let state = TaskState::load(repository.clone(), events.clone()).unwrap();
        state.insert(task(TaskStatus::Waiting)).unwrap();

        state.progress("task-1", 10, 100, 5);
        state.progress("task-1", 20, 100, 5);

        assert_eq!(state.list()[0].downloaded, 20);
        assert_eq!(repository.saves.load(Ordering::SeqCst), 2);
        assert_eq!(repository.rows.lock().unwrap()[0].downloaded, 10);
        assert_eq!(events.0.lock().unwrap().len(), 3);
    }
}
