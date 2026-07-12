use crate::dicom::wl::data::get_json_worklist;
use crate::dicom::wl::worklist::WorklistItem;
use anyhow::Result;

#[allow(dead_code)]
pub enum DbSource {
    Json,
    Default,
}

pub fn get_worklist(source: DbSource) -> Result<Vec<WorklistItem>> {
    let data = match source {
        DbSource::Json => get_json_worklist()?,
        DbSource::Default => vec![WorklistItem::default()],
    };

    Ok(data)
}
