mod config;
mod dicom;
mod logs;

use crate::config::{configuration, init_configuration};
use anyhow::Result;
use logs::run_logs;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize configuration
    init_configuration()?;
    // Load configuration
    let config = configuration();

    // Run logs
    run_logs(&config.logs_path)?;

    // Run DICOM server
    dicom::server::start(config).await?;

    Ok(())
}
