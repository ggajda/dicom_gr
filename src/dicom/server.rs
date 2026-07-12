use super::helper::{command_tag_string, command_tag_u16};
use super::syntax::{ABSTRACT_SYNTAX, C_ECHO_RQ, C_FIND_RQ, C_STORE_RQ, TRANSFER_SYNTAXES};
use super::{c_echo, c_find, c_store};
use crate::config::model::Config;
use anyhow::Result;
use dicom_dictionary_std::tags;
use dicom_ul::association::ServerAssociationOptions;
use dicom_ul::pdu::{AbortRQServiceProviderReason, AbortRQSource};
use dicom_ul::{
    Pdu,
    pdu::{PDataValue, PDataValueType},
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

pub async fn start(config: &Config) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let addr = format!("{}:{}", config.host, config.port);

    let listener = TcpListener::bind(&addr).await.map_err(|e| {
        error!("Server error: {}", e);
        e
    })?;

    let dicom_storage = Arc::new(PathBuf::from(&config.dicom_storage_path));

    info!("DICOM-GR server (v{}) is running on {}", &version, &addr);

    loop {
        let (socket, addr) = listener.accept().await?;
        info!("Connection from {}", addr);

        let dicom_storage_path = Arc::clone(&dicom_storage);

        tokio::task::spawn(async move {
            // Association configuration
            let mut association_options = ServerAssociationOptions::new(); //.accept_any();

            for &syntax in ABSTRACT_SYNTAX {
                association_options = association_options.with_abstract_syntax(syntax);
            }

            for &syntax in TRANSFER_SYNTAXES {
                association_options = association_options.with_transfer_syntax(syntax);
            }

            let association_result = association_options.establish_async(socket).await;

            // Run association
            let mut scp = match association_result {
                Ok(assoc) => {
                    info!("DICOM association established successfully with {}", addr);
                    assoc
                }
                Err(e) => {
                    warn!(
                        "Failed to establish a DICOM association with {}: {}",
                        addr, e
                    );
                    return;
                }
            };

            let mut command_buffer = Vec::new();
            let mut data_buffer = Vec::new();
            let mut query_buffer = Vec::new();

            let mut store_message_id = None;
            let mut store_sop_class_uid = None;
            let mut store_sop_instance_uid = None;
            let mut store_presentation_context_id = None;

            let mut pending_command = None;
            let mut pending_message_id = None;
            let mut pending_presentation_context_id = None;

            loop {
                match scp.receive().await {
                    Ok(Pdu::PData { data }) => {
                        for pdv in data {
                            let PDataValue {
                                presentation_context_id,
                                value_type,
                                is_last,
                                data,
                            } = pdv;

                            match value_type {
                                PDataValueType::Command => {
                                    command_buffer.extend_from_slice(&data);

                                    if !is_last {
                                        continue;
                                    }

                                    let command_field =
                                        command_tag_u16(&command_buffer, tags::COMMAND_FIELD);

                                    info!(
                                        "Received DIMSE command from {}: {}",
                                        addr,
                                        command_field.unwrap_or_default()
                                    );

                                    let message_id =
                                        command_tag_u16(&command_buffer, tags::MESSAGE_ID);

                                    match command_field {
                                        Some(C_ECHO_RQ) => {
                                            c_echo(
                                                message_id,
                                                addr,
                                                presentation_context_id,
                                                &mut scp,
                                            )
                                            .await;

                                            command_buffer.clear();
                                        }
                                        Some(C_FIND_RQ) => {
                                            info!(
                                                "Received C-FIND-RQ from {} waiting for dataset",
                                                addr
                                            );

                                            let has_dataset = command_tag_u16(
                                                &command_buffer,
                                                tags::COMMAND_DATA_SET_TYPE,
                                            );

                                            if has_dataset == Some(0x0101) {
                                                // No dataset
                                                c_find(
                                                    message_id,
                                                    &[],
                                                    addr,
                                                    presentation_context_id,
                                                    &mut scp,
                                                )
                                                .await;
                                            } else {
                                                pending_command = Some(C_FIND_RQ);
                                                pending_message_id = message_id;
                                                pending_presentation_context_id =
                                                    Some(presentation_context_id);
                                            }
                                            query_buffer.clear();
                                            command_buffer.clear();
                                        }
                                        Some(C_STORE_RQ) => {
                                            pending_command = Some(C_STORE_RQ);

                                            store_message_id = message_id;
                                            store_sop_class_uid = command_tag_string(
                                                &command_buffer,
                                                tags::AFFECTED_SOP_CLASS_UID,
                                            );
                                            store_sop_instance_uid = command_tag_string(
                                                &command_buffer,
                                                tags::AFFECTED_SOP_INSTANCE_UID,
                                            );
                                            store_presentation_context_id =
                                                Some(presentation_context_id);

                                            data_buffer.clear();
                                            command_buffer.clear();
                                        }
                                        _ => {
                                            warn!(
                                                "Received unsupported DIMSE command from {}: {}",
                                                addr,
                                                command_field.unwrap_or_default()
                                            );

                                            command_buffer.clear();
                                        }
                                    }
                                }

                                PDataValueType::Data => match pending_command {
                                    Some(C_STORE_RQ) => {
                                        data_buffer.extend_from_slice(&data);

                                        if !is_last {
                                            continue;
                                        }

                                        let Some(presentation_context_id) =
                                            store_presentation_context_id
                                        else {
                                            warn!(
                                                "Received C-STORE data from {}, but no remembered presentation_context_id was found",
                                                addr
                                            );
                                            data_buffer.clear();
                                            pending_command = None;
                                            continue;
                                        };

                                        c_store(
                                            store_message_id,
                                            store_sop_class_uid.take(),
                                            store_sop_instance_uid.take(),
                                            &data_buffer,
                                            addr,
                                            presentation_context_id,
                                            &mut scp,
                                            &dicom_storage_path,
                                        )
                                        .await;

                                        data_buffer.clear();
                                        store_message_id = None;
                                        store_presentation_context_id = None;
                                        pending_command = None;
                                    }
                                    Some(C_FIND_RQ) => {
                                        info!(
                                            "Received C-FIND query dataset from {} ({} bytes)",
                                            addr,
                                            data.len()
                                        );

                                        query_buffer.extend_from_slice(&data);

                                        if !is_last {
                                            continue;
                                        }

                                        let message_id = pending_message_id;

                                        let Some(context_id) = pending_presentation_context_id
                                        else {
                                            warn!(
                                                "Empty presentation context for C-FIND from {}",
                                                addr
                                            );

                                            query_buffer.clear();
                                            pending_command = None;
                                            continue;
                                        };

                                        let query = std::mem::take(&mut query_buffer);

                                        pending_command = None;
                                        pending_message_id = None;
                                        pending_presentation_context_id = None;

                                        c_find(message_id, &query, addr, context_id, &mut scp)
                                            .await;

                                        query_buffer.clear();

                                        // pending_command = None;
                                        // pending_message_id = None;
                                        // pending_presentation_context_id = None;
                                    }

                                    _ => warn!("Unexpected dataset from {}", addr),
                                },
                            }
                        }
                    }

                    Ok(Pdu::ReleaseRQ) => {
                        info!("Received ReleaseRQ from {}", addr);

                        if let Err(e) = scp.send(&Pdu::ReleaseRP).await {
                            error!("Failed to send ReleaseRP to {}: {}", addr, e);
                        }

                        break;
                    }

                    Ok(Pdu::AbortRQ { source }) => {
                        let source_label = match source {
                            AbortRQSource::ServiceUser => "service-user",
                            AbortRQSource::ServiceProvider(
                                AbortRQServiceProviderReason::UnrecognizedPdu,
                            ) => "service-provider",
                            AbortRQSource::ServiceProvider(
                                AbortRQServiceProviderReason::UnexpectedPdu,
                            ) => "service-provider",
                            AbortRQSource::ServiceProvider(
                                AbortRQServiceProviderReason::Reserved,
                            ) => "service-provider",
                            AbortRQSource::ServiceProvider(
                                AbortRQServiceProviderReason::UnrecognizedPduParameter,
                            ) => "service-provider",
                            AbortRQSource::ServiceProvider(
                                AbortRQServiceProviderReason::UnexpectedPduParameter,
                            ) => "service-provider",
                            AbortRQSource::ServiceProvider(
                                AbortRQServiceProviderReason::InvalidPduParameter,
                            ) => "service-provider",
                            AbortRQSource::Reserved => "reserved",
                            _ => "unknown",
                        };

                        warn!(
                            "Received an association abort request from {}: {}",
                            addr, source_label
                        );
                        break;
                    }

                    Ok(pdu) => {
                        let pdu_type = match pdu {
                            Pdu::AssociationRQ { .. } => "AssociationRQ",
                            Pdu::AssociationAC { .. } => "AssociationAC",
                            Pdu::AssociationRJ { .. } => "AssociationRJ",
                            Pdu::PData { .. } => "PData",
                            Pdu::ReleaseRQ => "ReleaseRQ",
                            Pdu::ReleaseRP => "ReleaseRP",
                            Pdu::AbortRQ { .. } => "AbortRQ",
                            _ => "unknown",
                        };
                        info!("Received another PDU type from {}: {}", addr, pdu_type);
                    }

                    Err(e) => {
                        let err_str = e.to_string();

                        if err_str.contains("Connection closed") || err_str.contains("broken pipe")
                        {
                            info!("Client {} disconnected.", addr);
                        } else {
                            error!("Error while reading PDU from {}: {}", addr, e);
                        }

                        break;
                    }
                }
            }
        });
    }
}
