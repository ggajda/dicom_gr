use crate::dicom::syntax::C_ECHO_RSP;
use anyhow::Result;
use dicom_core::VR;
use dicom_core::header::DataElement;
use dicom_core::value::PrimitiveValue;
use dicom_dictionary_std::tags;
use dicom_dictionary_std::uids;
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::entries;
use dicom_ul::association::AsyncServerAssociation;
use dicom_ul::{
    Pdu,
    pdu::{PDataValue, PDataValueType},
};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{error, info};

fn c_echo_response(message_id: u16) -> Result<Vec<u8>> {
    // Command Group Length (0000,0000) is not included in the elements without group length.
    let elements_without_group_length = [
        DataElement::new(tags::AFFECTED_SOP_CLASS_UID, VR::UI, uids::VERIFICATION),
        DataElement::new(
            tags::COMMAND_FIELD,
            VR::US,
            PrimitiveValue::from(C_ECHO_RSP),
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

pub async fn c_echo(
    message_id: Option<u16>,
    addr: SocketAddr,
    presentation_context_id: u8,
    scp: &mut AsyncServerAssociation<TcpStream>,
) {
    let Some(message_id) = message_id else {
        error!(
            "Failed to read the Message ID from the C-ECHO-RQ from {}",
            addr
        );
        return;
    };

    info!(
        "Received C-ECHO-RQ from {}, message_id={}",
        addr, message_id
    );

    let response = match c_echo_response(message_id) {
        Ok(response) => response,
        Err(e) => {
            error!(
                "Failed to generate the C-ECHO-RSP response for {}: {}",
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
        error!("Failed to send C-ECHO-RSP to {}: {}", addr, e);
        return;
    }

    info!("Sent C-ECHO-RSP Success to {}", addr);
}
