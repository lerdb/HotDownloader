//! 可由 Tauri 客户端及独立 Rust 服务复用的任务业务核心。
//! 原生窗口、Android SAF、系统通知和 IPC 适配保留在各自的运行时项目中。

pub mod contract;
pub mod decryption;
pub mod download_config;
pub mod download_link;
pub mod download_path;
pub mod download_worker;
pub mod engine;
pub mod filename;
pub mod http_transfer;
pub mod json_task_repository;
pub mod kuwo_link;
pub mod kwdes;
pub mod kwlyric;
pub mod local_download_file;
pub mod local_file_deleter;
pub mod platform;
pub mod platforms;
pub mod ports;
pub mod postprocess;
pub mod qq_credentials;
pub mod qq_login;
pub mod qqmusic_link;
pub mod qrc;
pub mod settings_patch;
pub mod task_context;
pub mod task_rules;
pub mod task_service;
pub mod task_state;
