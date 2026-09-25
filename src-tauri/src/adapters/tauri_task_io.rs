use tauri::{AppHandle, Emitter};

use crate::download::contract::TaskRecord;
use crate::download::task_state::{TaskEventSink, TaskRepository};
use crate::storage::store_wrapper;

const TASK_UPDATED: &str = "task-updated";
const TASK_REMOVED: &str = "task-removed";

/// 保持现有 Tauri Store 数据格式和事件名称，供桌面及 Android 客户端使用。
pub struct TauriTaskIo {
    app: AppHandle,
}

impl TauriTaskIo {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl TaskRepository for TauriTaskIo {
    fn load(&self) -> Result<Vec<TaskRecord>, String> {
        let json = store_wrapper::load_string(&self.app, "tasks").map_err(|e| e.to_string())?;
        if json.is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(&json).map_err(|e| format!("解析任务记录失败: {e}"))
    }

    fn save(&self, tasks: &[TaskRecord]) -> Result<(), String> {
        let json = serde_json::to_string(tasks).map_err(|e| e.to_string())?;
        store_wrapper::save_string(&self.app, "tasks", &json).map_err(|e| e.to_string())
    }
}

impl TaskEventSink for TauriTaskIo {
    fn updated(&self, task: TaskRecord) {
        if let Err(error) = self.app.emit(TASK_UPDATED, task) {
            log::warn!("发送任务更新事件失败: {error}");
        }
    }

    fn removed(&self, task_id: &str) {
        if let Err(error) = self.app.emit(TASK_REMOVED, task_id.to_string()) {
            log::warn!("发送任务删除事件失败: {error}");
        }
    }
}
