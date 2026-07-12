use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)] // Added Serialize to allow saving data back to disk
pub struct WorklistItem {
    pub patient_name: String,
    pub patient_id: String,
    pub accession_number: String,
    pub modality: String,
    pub scheduled_date: String,
    pub scheduled_time: String,
    pub station_ae_title: String,
}

impl Default for WorklistItem {
    fn default() -> Self {
        let current_date = Local::now().format("%Y%m%d").to_string();

        Self {
            patient_name: "First^Patient".into(),
            patient_id: "12345".into(),
            accession_number: "ACC001".into(),
            modality: "CT".into(),
            scheduled_date: current_date.clone(),
            scheduled_time: "120000".into(),
            station_ae_title: "CT_ROOM".into(),
        }
    }
}

// #[derive(Clone, Debug, Deserialize, Serialize)] // Added Serialize to allow saving data back to disk
// pub struct Worklist {
//     pub worklist_items: Vec<WorklistItem>,
// }

// impl Default for Worklist {
//     fn default() -> Self {
//         Self {
//             worklist_items: vec![WorklistItem::default()],
//         }
//     }
// }
