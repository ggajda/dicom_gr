use crate::config::configuration;
use anyhow::Result;
use dicom_core::Tag;
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::entries;
use std::io::Cursor;
use std::path::Path;
use tokio::fs;

fn parse_command(command: &[u8]) -> Option<InMemDicomObject> {
    let ts = entries::IMPLICIT_VR_LITTLE_ENDIAN.erased();
    InMemDicomObject::read_dataset_with_ts(Cursor::new(command), &ts).ok()
}

pub fn command_tag_u16(command: &[u8], tag: Tag) -> Option<u16> {
    parse_command(command)?
        .element(tag)
        .ok()?
        .value()
        .to_int::<u16>()
        .ok()
}

pub fn command_tag_string(command: &[u8], tag: Tag) -> Option<String> {
    parse_command(command)?
        .element(tag)
        .ok()?
        .value()
        .to_str()
        .ok()
        .map(|s| s.to_string())
}

pub async fn save_dicom_file(
    sop_instance_uid: String,
    sop_class_uid: String,
    dicom_data: Vec<u8>,
) -> Result<()> {
    let storage_path = &configuration().dicom_storage_path;
    let storage_dir = Path::new(&storage_path);

    fs::create_dir_all(storage_dir).await?;

    let safe_name = sop_instance_uid
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>();

    let path = storage_dir.join(format!("{safe_name}.dcm"));

    // Clone the values into owned types so they can be safely passed to the spawned thread.
    let sop_class_uid_owned = sop_class_uid.to_string();
    let sop_instance_uid_owned = sop_instance_uid.to_string();
    let dicom_data_vec = dicom_data.to_vec();
    let path_clone = path.clone();

    tokio::task::spawn_blocking(move || -> Result<()> {
        use dicom_object::FileDicomObject;
        use std::fs::OpenOptions;
        use std::io::Write;

        // 1. Build the file meta header inside this thread.
        let meta = dicom_object::meta::FileMetaTableBuilder::new()
            .media_storage_sop_class_uid(sop_class_uid_owned)
            .media_storage_sop_instance_uid(sop_instance_uid_owned)
            .transfer_syntax(entries::EXPLICIT_VR_LITTLE_ENDIAN.uid())
            .build()?;

        // 2. Create an empty file object; now `file_object` is definitely in scope.
        let file_object = FileDicomObject::new_empty_with_meta(meta);

        // Step A: Write the correct official DICOM header (128B + DICM + Group 0002).
        file_object.write_to_file(&path_clone)?;

        // Step B: Open the file in append mode and add the raw network image bytes.
        let mut file = OpenOptions::new()
            //.write(true)
            .append(true)
            .open(&path_clone)?;

        file.write_all(&dicom_data_vec)?;
        Ok(())
    })
    .await??;

    Ok(())
}
