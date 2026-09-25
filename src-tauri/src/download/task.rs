// 下载循环和任务上下文已经迁入共享核心；保留旧模块路径兼容现有命令与适配器。
pub use hotdownloader_core::download_worker::download_task;
pub use hotdownloader_core::task_context::TaskContext;
