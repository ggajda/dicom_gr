use anyhow::Result;
use std::path::Path;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::SubscriberExt, registry, util::SubscriberInitExt};

pub fn run_logs(logs_path_name: &str) -> Result<()> {
    let logs_path = Path::new(logs_path_name);
    let log_file = rolling::daily(logs_path, "app.log");

    registry()
        // stdout
        .with(fmt::layer().with_target(false).with_writer(std::io::stdout))
        // file
        .with(
            fmt::layer()
                .with_target(false)
                .with_ansi(false) // no colors in the log file
                .with_writer(log_file),
        )
        .init();

    Ok(())
}
