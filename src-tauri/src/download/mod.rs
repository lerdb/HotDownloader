pub mod progress;
pub mod task;
pub mod task_service;

// 渐进迁移期间保留原模块路径，避免一次性改动所有下载调用方。
pub use hotdownloader_core::engine;
pub use hotdownloader_core::{contract, local_file_deleter, ports, task_state};

// 新增子模块
pub(crate) mod task_file;
pub(crate) mod task_lrc;
pub(crate) mod task_metadata;
