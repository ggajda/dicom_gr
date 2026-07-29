use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub server_ae_title: String,
    pub clients_ae_title: Vec<String>,
    pub dicom_storage_path: String,
    pub json_worklist_path: String,
    pub logs_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 11112,
            server_ae_title: "AE_Server".to_string(),
            clients_ae_title: vec!["AE_Client1".to_string(), "AE_Client2".to_string()],
            dicom_storage_path: "dicom_storage".to_string(),
            json_worklist_path: "worklist.json".to_string(),
            logs_path: "logs".to_string(),
        }
    }
}
