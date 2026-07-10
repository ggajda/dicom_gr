use super::model::Config;
use anyhow::Result;
use std::sync::OnceLock;
use std::{fs, io::ErrorKind};
use tracing::warn;

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn init_configuration() -> Result<()> {
    let config = load_configuration()?;

    CONFIG
        .set(config)
        .map_err(|_| anyhow::anyhow!("Configuration has already been initialized"))?;

    Ok(())
}

pub fn configuration() -> &'static Config {
    CONFIG
        .get()
        .expect("Configuration has not been initialized")
}

pub fn load_configuration() -> Result<Config> {
    let path = "config.json";

    let json = match fs::read_to_string(path) {
        Ok(json) => json,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            let config = Config::default();
            let json = serde_json::to_string_pretty(&config)?;
            fs::write(path, &json)?;
            warn!("Config JSON file was created with default values!");
            json
        }
        Err(err) => return Err(err.into()),
    };

    Ok(serde_json::from_str(&json)?)
}
