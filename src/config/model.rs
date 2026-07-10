use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub logs_path: String,
    pub dicom_storage_path: String,
    pub json_worklist_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 11112,
            logs_path: "logs".to_string(),
            dicom_storage_path: "dicom_storage".to_string(),
            json_worklist_path: "worklist.json".to_string(),
        }
    }
}
