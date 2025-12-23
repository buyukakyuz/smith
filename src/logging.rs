use std::path::PathBuf;

#[cfg(feature = "debug-log")]
mod inner {
    use super::*;
    use std::fs;
    use tracing_appender::non_blocking::WorkerGuard;
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    static LOG_PATH: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

    pub fn init() -> Option<(PathBuf, WorkerGuard)> {
        let log_path = PathBuf::from("smith-debug.log");

        let file = match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to open log file: {e}");
                return None;
            }
        };

        let (non_blocking, guard) = tracing_appender::non_blocking(file);

        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));

        let subscriber = tracing_subscriber::registry().with(filter).with(
            fmt::layer()
                .json()
                .with_writer(non_blocking)
                .with_target(true)
                .with_file(true)
                .with_line_number(true),
        );

        if tracing::subscriber::set_global_default(subscriber).is_err() {
            eprintln!("Failed to set tracing subscriber");
            return None;
        }

        LOG_PATH.set(log_path.clone()).ok();

        tracing::info!("Debug logging initialized");

        Some((log_path, guard))
    }

    pub fn log_file_path() -> Option<&'static PathBuf> {
        LOG_PATH.get()
    }
}

#[cfg(not(feature = "debug-log"))]
mod inner {
    use super::*;

    #[inline(always)]
    pub fn init() -> Option<(PathBuf, ())> {
        None
    }

    #[inline(always)]
    pub fn log_file_path() -> Option<&'static PathBuf> {
        None
    }
}

pub use inner::*;
