use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::domain::config::LogLevel;

impl LogLevel {
    pub fn as_filter_str(&self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// Initialize tracing with the given log level, writing to stderr and a daily-rotating log file.
/// Returns a WorkerGuard that must be held for the lifetime of the application to ensure
/// buffered logs are flushed.
pub fn init_logging(log_level: LogLevel, log_dir: &Path) -> WorkerGuard {
    let filter = EnvFilter::new(log_level.as_filter_str());

    let file_appender = tracing_appender::rolling::daily(log_dir, "cupola");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    guard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_as_filter_str() {
        assert_eq!(LogLevel::Trace.as_filter_str(), "trace");
        assert_eq!(LogLevel::Debug.as_filter_str(), "debug");
        assert_eq!(LogLevel::Info.as_filter_str(), "info");
        assert_eq!(LogLevel::Warn.as_filter_str(), "warn");
        assert_eq!(LogLevel::Error.as_filter_str(), "error");
    }
}
