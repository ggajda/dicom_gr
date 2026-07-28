use anyhow::Result;
use std::path::Path;
use tracing_appender::rolling;
use tracing_subscriber::{
    fmt::{self, time::OffsetTime},
    layer::SubscriberExt,
    registry,
    util::SubscriberInitExt,
};

pub fn run_logs(logs_path_name: &str) -> Result<()> {
    let logs_path = Path::new(logs_path_name);
    let log_file = rolling::daily(logs_path, "app.log");

    let local_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);

    let format = time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

    let timer = OffsetTime::new(local_offset, format);

    registry()
        // stdout
        .with(
            fmt::layer()
                .with_target(false)
                .with_timer(timer.clone())
                .with_writer(std::io::stdout),
        )
        // file
        .with(
            fmt::layer()
                .with_target(false)
                .with_ansi(false) // no colors in the log file
                .with_timer(timer)
                .with_writer(log_file),
        )
        .init();

    Ok(())
}
