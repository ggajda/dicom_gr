use crate::dicom::syntax::C_FIND_RSP;
use crate::dicom::wl::source::{DbSource, get_worklist};
use crate::dicom::wl::worklist::WorklistItem;
use anyhow::Result;
use dicom_core::{Tag, VR, header::DataElement, value::PrimitiveValue};
use dicom_dictionary_std::tags;
use dicom_dictionary_std::uids;
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::entries;

use dicom_core::DicomValue;
use dicom_ul::association::AsyncServerAssociation;
use dicom_ul::{
    Pdu,
    pdu::{PDataValue, PDataValueType},
};
use std::io::Cursor;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{error, info};

#[derive(Default)]
struct WorklistFilter {
    patient_name: Option<String>,
    patient_id: Option<String>,
    accession_number: Option<String>,
    modality: Option<String>,
    station_ae_title: Option<String>,
    scheduled_date: Option<String>,
}

impl WorklistFilter {
    fn matches(&self, item: &WorklistItem) -> bool {
        if let Some(ref value) = self.patient_name
            && !matches_dicom_string(&item.patient_name, value)
        {
            return false;
        }

        if let Some(ref value) = self.patient_id
            && !matches_dicom_string(&item.patient_id, value)
        {
            return false;
        }

        if let Some(ref value) = self.accession_number
            && !matches_dicom_string(&item.accession_number, value)
        {
            return false;
        }

        if let Some(ref value) = self.modality
            && !matches_dicom_string(&item.modality, value)
        {
            return false;
        }
        if let Some(ref value) = self.station_ae_title
            && !matches_dicom_string(&item.station_ae_title, value)
        {
            return false;
        }

        if let Some(ref value) = self.scheduled_date
            && !matches_dicom_string(&item.scheduled_date, value)
        {
            return false;
        }

        true
    }
}

fn matches_dicom_string(source: &str, filter: &str) -> bool {
    let source = source.to_lowercase();
    let filter = filter.replace("*", "").to_lowercase();

    source.contains(&filter)
}

fn c_find_response(message_id: u16, status: u16, has_dataset: bool) -> Result<Vec<u8>> {
    let dataset_type = if has_dataset { 0x0000_u16 } else { 0x0101_u16 };

    let elements = [
        DataElement::new(
            tags::AFFECTED_SOP_CLASS_UID,
            VR::UI,
            uids::MODALITY_WORKLIST_INFORMATION_MODEL_FIND,
        ),
        DataElement::new(
            tags::COMMAND_FIELD,
            VR::US,
            PrimitiveValue::from(C_FIND_RSP),
        ),
        DataElement::new(
            tags::MESSAGE_ID_BEING_RESPONDED_TO,
            VR::US,
            PrimitiveValue::from(message_id),
        ),
        DataElement::new(
            tags::COMMAND_DATA_SET_TYPE,
            VR::US,
            PrimitiveValue::from(dataset_type),
        ),
        DataElement::new(tags::STATUS, VR::US, PrimitiveValue::from(status)),
    ];

    let command = InMemDicomObject::command_from_element_iter(elements);

    let mut out = Vec::new();

    command.write_dataset_with_ts(&mut out, &entries::IMPLICIT_VR_LITTLE_ENDIAN.erased())?;

    Ok(out)
}

fn parse_filter(query: &[u8]) -> WorklistFilter {
    let mut filter = WorklistFilter::default();

    let obj = match InMemDicomObject::read_dataset_with_ts(
        Cursor::new(query),
        &entries::IMPLICIT_VR_LITTLE_ENDIAN.erased(),
    ) {
        Ok(obj) => obj,

        Err(e) => {
            error!("Cannot parse C-FIND dataset: {}", e);

            return filter;
        }
    };

    filter.patient_name = read_filter(&obj, tags::PATIENT_NAME);
    filter.patient_id = read_filter(&obj, tags::PATIENT_ID);
    filter.accession_number = read_filter(&obj, tags::ACCESSION_NUMBER);
    filter.modality =
        read_filter(&obj, tags::MODALITY).or_else(|| read_sub_filter(&obj, tags::MODALITY));
    filter.station_ae_title = read_filter(&obj, tags::SCHEDULED_STATION_AE_TITLE)
        .or_else(|| read_sub_filter(&obj, tags::SCHEDULED_STATION_AE_TITLE));
    filter.scheduled_date = read_filter(&obj, tags::SCHEDULED_PROCEDURE_STEP_START_DATE)
        .or_else(|| read_sub_filter(&obj, tags::SCHEDULED_PROCEDURE_STEP_START_DATE));

    filter
}

fn read_sub_filter(obj: &InMemDicomObject, tag: Tag) -> Option<String> {
    let element = obj.element(tags::SCHEDULED_PROCEDURE_STEP_SEQUENCE).ok()?;

    match element.value() {
        DicomValue::Sequence(seq) => {
            let item = &seq.items()[0];
            item.element(tag).ok()?.to_str().ok().map(|s| s.to_string())
        }
        _ => None,
    }
}

fn read_filter(obj: &InMemDicomObject, tag: Tag) -> Option<String> {
    obj.element(tag)
        .ok()
        .and_then(|e| e.to_str().ok())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

fn worklist_dataset(item: &WorklistItem) -> Result<Vec<u8>> {
    let obj = InMemDicomObject::from_element_iter([
        // Allow UTF-8 chars
        DataElement::new(tags::SPECIFIC_CHARACTER_SET, VR::CS, "ISO_IR 192"),
        DataElement::new(
            tags::PATIENT_NAME,
            VR::PN,
            PrimitiveValue::from(item.patient_name.clone()),
        ),
        DataElement::new(
            tags::PATIENT_ID,
            VR::LO,
            PrimitiveValue::from(item.patient_id.clone()),
        ),
        DataElement::new(
            tags::ACCESSION_NUMBER,
            VR::SH,
            PrimitiveValue::from(item.accession_number.clone()),
        ),
        DataElement::new(
            tags::MODALITY,
            VR::CS,
            PrimitiveValue::from(item.modality.clone()),
        ),
        DataElement::new(
            tags::SCHEDULED_PROCEDURE_STEP_START_DATE,
            VR::DA,
            PrimitiveValue::from(item.scheduled_date.clone()),
        ),
        DataElement::new(
            tags::SCHEDULED_PROCEDURE_STEP_START_TIME,
            VR::TM,
            PrimitiveValue::from(item.scheduled_time.clone()),
        ),
        DataElement::new(
            tags::SCHEDULED_STATION_AE_TITLE,
            VR::AE,
            PrimitiveValue::from(item.station_ae_title.clone()),
        ),
    ]);

    let mut out = Vec::new();

    obj.write_dataset_with_ts(&mut out, &entries::IMPLICIT_VR_LITTLE_ENDIAN.erased())?;

    Ok(out)
}

pub async fn c_find(
    message_id: Option<u16>,
    query: &[u8],
    addr: SocketAddr,
    presentation_context_id: u8,
    scp: &mut AsyncServerAssociation<TcpStream>,
) {
    let Some(message_id) = message_id else {
        error!("Empty Message ID from {}", addr);
        return;
    };

    //info!("*** QUERY: {:?}", query);
    let filter = parse_filter(query);

    info!("C-FIND query dump:");
    info!("Query dataset size: {} bytes", query.len());
    info!(
        "C-FIND from {}, patient_name={}, modality={}",
        addr,
        filter.patient_name.as_deref().unwrap_or_default(),
        filter.modality.as_deref().unwrap_or_default()
    );

    // Getting worklist data / DbSource::Json => from json file / DbSource::Default => default data from Worklist struct
    let worklist = match get_worklist(DbSource::Json) {
        Ok(list) => list,
        Err(e) => {
            error!("Error while loading worklist: {}", e);
            return; // lub odpowiedni status błędu DICOM, jeśli wymagany
        }
    };

    for item in worklist.into_iter().filter(|x| filter.matches(x)) {
        let command = match c_find_response(message_id, 0xFF00, true) {
            Ok(v) => v,
            Err(e) => {
                error!("Command worklist error: {}", e);
                return;
            }
        };

        let dataset = match worklist_dataset(&item) {
            Ok(v) => v,
            Err(e) => {
                error!("Dataset worklist error: {}", e);
                return;
            }
        };

        let pdv = vec![
            PDataValue {
                presentation_context_id,
                value_type: PDataValueType::Command,
                is_last: true,
                data: command,
            },
            PDataValue {
                presentation_context_id,
                value_type: PDataValueType::Data,
                is_last: true,
                data: dataset,
            },
        ];

        if let Err(e) = scp.send(&Pdu::PData { data: pdv }).await {
            error!("Error sending C-FIND Pending: {}", e);
            return;
        }

        info!("Sent {} to {}", item.patient_name, addr);
    }

    let command = match c_find_response(message_id, 0x0000, false) {
        Ok(v) => v,
        Err(e) => {
            error!("C-FIND command error: {}", e);
            return;
        }
    };

    if let Err(e) = scp
        .send(&Pdu::PData {
            data: vec![PDataValue {
                presentation_context_id,
                value_type: PDataValueType::Command,
                is_last: true,
                data: command,
            }],
        })
        .await
    {
        error!("Error sending C-FIND Success: {}", e);
    }

    info!("C-FIND is finished for {}", addr);
}
