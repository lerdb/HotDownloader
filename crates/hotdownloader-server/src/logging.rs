//! 独立服务的标准错误输出日志。Docker 收集 stdout/stderr，按任务 ID 即可检索。

use std::time::{SystemTime, UNIX_EPOCH};

struct ServerLogger;

static LOGGER: ServerLogger = ServerLogger;

impl log::Log for ServerLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default();
        // JSON 行避免异常文本中的换行拆散一条日志。日志不记录 HTTP 授权头或凭据文件。
        eprintln!(
            "{}",
            serde_json::json!({
                "time": timestamp,
                "level": record.level().to_string(),
                "target": record.target(),
                "message": record.args().to_string(),
            })
        );
    }

    fn flush(&self) {}
}

pub fn initialize() {
    let level = match std::env::var("HOTDOWNLOADER_LOG_LEVEL")
        .unwrap_or_else(|_| "info".into())
        .to_ascii_lowercase()
        .as_str()
    {
        "off" => log::LevelFilter::Off,
        "error" => log::LevelFilter::Error,
        "warn" => log::LevelFilter::Warn,
        "debug" => log::LevelFilter::Debug,
        "trace" => log::LevelFilter::Trace,
        _ => log::LevelFilter::Info,
    };
    log::set_logger(&LOGGER).expect("服务日志只应初始化一次");
    log::set_max_level(level);
}
