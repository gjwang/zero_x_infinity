use crate::config::AppConfig;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init_logging(config: &AppConfig) -> WorkerGuard {
    let file_appender = match config.rotation.as_str() {
        "hourly" => tracing_appender::rolling::hourly(&config.log_dir, &config.log_file),
        "daily" => tracing_appender::rolling::daily(&config.log_dir, &config.log_file),
        _ => tracing_appender::rolling::never(&config.log_dir, &config.log_file),
    };

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter_str = if config.enable_tracing {
        config.log_level.clone()
    } else {
        format!("{},0XINFI=off", config.log_level)
    };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter_str));

    let registry = tracing_subscriber::registry().with(filter);

    if config.use_json {
        let file_layer = fmt::layer()
            .json()
            .with_target(true) // Keep target in JSON for structured queries
            .with_writer(non_blocking)
            .with_ansi(false);
        registry.with(file_layer).init();
    } else {
        let file_layer = fmt::layer()
            .with_target(false) // Hide redundant target in text output
            .with_writer(non_blocking)
            .with_ansi(false);
        let stdout_layer = fmt::layer().with_target(false).with_ansi(true);
        registry.with(file_layer).with(stdout_layer).init();
    }

    guard
}
