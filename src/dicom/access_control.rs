use crate::config::settings::load_configuration;
use dicom_ul::association::server::AccessControl;
use dicom_ul::pdu::{AssociationRJServiceUserReason, UserIdentity};
use tracing::info;

pub struct CustomAccessControl;

impl AccessControl for CustomAccessControl {
    fn check_access(
        &self,
        this_ae_title: &str,
        calling_ae_title: &str,
        called_ae_title: &str,
        _user_identity: Option<&UserIdentity>,
    ) -> Result<(), dicom_ul::pdu::AssociationRJServiceUserReason> {
        let config = load_configuration().unwrap_or_default();

        info!(
            "Association request: this='{}', calling='{}', called='{}'",
            this_ae_title, calling_ae_title, called_ae_title
        );

        // Calling AE Title
        //const ALLOWED: &[&str] = &["AE_Client1", "AE_Client2", "AE_Client3"];
        let clients_ae_title = config.clients_ae_title;

        if clients_ae_title.contains(&calling_ae_title.to_string())
            && this_ae_title == called_ae_title
        {
            Ok(())
        } else {
            Err(AssociationRJServiceUserReason::CallingAETitleNotRecognized)
        }
    }
}
