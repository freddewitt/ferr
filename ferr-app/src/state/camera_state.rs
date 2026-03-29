#[derive(Default)]
pub struct CameraState {
    pub format_detection_enabled: bool,
    pub integrity_check: bool,
    pub rename_enabled: bool,
    pub rename_template: String,
    pub metadata_read_enabled: bool,
}
