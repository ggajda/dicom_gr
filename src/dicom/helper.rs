use dicom_core::Tag;
use dicom_object::InMemDicomObject;
use dicom_transfer_syntax_registry::entries;
use std::io::Cursor;

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
