use crate::config::configuration;
use crate::dicom::wl::worklist::WorklistItem;
use anyhow::Result;
use chrono::Local;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::Path;

// Helper function that populates default data when the file is missing
fn create_default_worklist(path: &Path) -> Result<Vec<WorklistItem>> {
    let current_date = Local::now().format("%Y%m%d").to_string();

    let default_json_data = vec![
        WorklistItem {
            patient_name: "First^Patient".into(),
            patient_id: "12345".into(),
            accession_number: "ACC001".into(),
            modality: "CT".into(),
            scheduled_date: current_date.clone(),
            scheduled_time: "120000".into(),
            station_ae_title: "CT_ROOM".into(),
        },
        WorklistItem {
            patient_name: "Second^Patient".into(),
            patient_id: "54321".into(),
            accession_number: "ACC002".into(),
            modality: "MR".into(),
            scheduled_date: current_date.clone(),
            scheduled_time: "140000".into(),
            station_ae_title: "MR_ROOM".into(),
        },
    ];

    // Create a new file
    let mut file = File::create(path)?;

    // Serialize the structure into a pretty-printed JSON string
    let json_string = serde_json::to_string_pretty(&default_json_data)?;
    file.write_all(json_string.as_bytes())?;

    Ok(default_json_data)
}

pub fn get_json_worklist() -> Result<Vec<WorklistItem>> {
    let json_worklist_path = Path::new(&configuration().json_worklist_path);

    // If the file DOES NOT exist, create it with default data and return it immediately
    if !json_worklist_path.exists() {
        return create_default_worklist(json_worklist_path);
    }

    // If the file exists, read and parse it normally
    let file = File::open(json_worklist_path)?;
    let reader = BufReader::new(file);
    let worklist = serde_json::from_reader(reader)?;

    Ok(worklist)
}
