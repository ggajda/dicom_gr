use crate::dicom::syntax::C_STORE_RSP;
use anyhow::Result;
use dicom_core::VR;
use dicom_core::header::DataElement;
use dicom_core::value::PrimitiveValue;
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::entries;
use dicom_ul::association::AsyncServerAssociation;
use dicom_ul::{
    Pdu,
    pdu::{PDataValue, PDataValueType},
};
use std::net::SocketAddr;
use std::path::Path;
use tokio::fs;
use tokio::net::TcpStream;
use tracing::{error, info};

fn c_store_response(
    message_id: u16,
    sop_class_uid: String,
    sop_instance_uid: String,
) -> Result<Vec<u8>> {
    // Command Group Length (0000,0000) is not included in the elements without group length.
    let elements_without_group_length = [
        DataElement::new(
            tags::AFFECTED_SOP_CLASS_UID,
            VR::UI,
            PrimitiveValue::from(sop_class_uid.clone()),
        ),
        DataElement::new(
            tags::COMMAND_FIELD,
            VR::US,
            PrimitiveValue::from(C_STORE_RSP),
        ),
        DataElement::new(
            tags::MESSAGE_ID_BEING_RESPONDED_TO,
            VR::US,
            PrimitiveValue::from(message_id),
        ),
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            PrimitiveValue::from(0x0101_u16),
        ),
        DataElement::new(tags::STATUS, VR::US, PrimitiveValue::from(0x0000_u16)),
        DataElement::new(
            tags::AFFECTED_SOP_INSTANCE_UID,
            VR::UI,
            PrimitiveValue::from(sop_instance_uid.clone()),
        ),
    ];

    let elements_without_group_clone = elements_without_group_length.clone();

    let command_without_group_length =
        InMemDicomObject::command_from_element_iter(elements_without_group_length);

    let mut command_body = Vec::new();
    command_without_group_length.write_dataset_with_ts(
        &mut command_body,
        &entries::IMPLICIT_VR_LITTLE_ENDIAN.erased(),
    )?;

    // Set the Command Group Length (0000,0000) to the length of the command body.
    let command_group_length = command_body.len() as u32;

    // Command Group Length (0000,0000) included in the elements with group length.
    let mut elements_with_group_length = vec![DataElement::new(
        tags::COMMAND_GROUP_LENGTH,
        VR::UL,
        PrimitiveValue::from(command_group_length),
    )];

    elements_with_group_length.extend(elements_without_group_clone);

    let command_with_group_length =
        InMemDicomObject::command_from_element_iter(elements_with_group_length);

    let command = InMemDicomObject::command_from_element_iter(command_with_group_length);

    let mut out = Vec::new();
    command.write_dataset_with_ts(&mut out, &entries::IMPLICIT_VR_LITTLE_ENDIAN.erased())?;

    Ok(out)
}

async fn save_dicom_file(
    storage_dir: &Path,
    sop_instance_uid: &str,
    sop_class_uid: &str,
    dicom_data: &[u8],
) -> Result<()> {
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
            .write(true)
            .append(true)
            .open(&path_clone)?;

        file.write_all(&dicom_data_vec)?;
        Ok(())
    })
    .await??;

    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub async fn c_store(
    message_id: Option<u16>,
    sop_class_uid: Option<String>,
    sop_instance_uid: Option<String>,
    dicom_data: &[u8],
    addr: SocketAddr,
    presentation_context_id: u8,
    scp: &mut AsyncServerAssociation<TcpStream>,
    dicom_storage_path: &Path,
) {
    let Some(message_id) = message_id else {
        error!(
            "Failed to read the Message ID from the C-STORE-RQ from {}",
            addr
        );
        return;
    };

    let Some(sop_class_uid) = sop_class_uid else {
        error!("Missing Affected SOP Class UID in C-STORE-RQ from {}", addr);
        return;
    };

    let Some(sop_instance_uid) = sop_instance_uid else {
        error!(
            "Missing Affected SOP Instance UID in C-STORE-RQ from {}",
            addr
        );
        return;
    };

    info!(
        "Received C-STORE-RQ from {}, message_id={}, sop_instance_uid={}",
        addr, message_id, sop_instance_uid
    );

    if let Err(e) = save_dicom_file(
        dicom_storage_path,
        &sop_instance_uid,
        &sop_class_uid,
        dicom_data,
    )
    .await
    {
        error!("Failed to save the DICOM file for {}: {}", addr, e);
        return;
    }

    let response = match c_store_response(message_id, sop_class_uid, sop_instance_uid) {
        Ok(response) => response,
        Err(e) => {
            error!(
                "Failed to generate the C-STORE-RSP response for {}: {}",
                addr, e
            );
            return;
        }
    };

    let response_pdv = PDataValue {
        presentation_context_id,
        value_type: PDataValueType::Command,
        is_last: true,
        data: response,
    };

    if let Err(e) = scp
        .send(&Pdu::PData {
            data: vec![response_pdv],
        })
        .await
    {
        error!("Failed to send C-STORE-RSP to {}: {}", addr, e);
        return;
    }

    info!("Sent C-STORE-RSP Success to {}", addr);
}
