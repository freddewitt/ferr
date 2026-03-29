#[derive(Default)]
pub struct ScanState {
    pub target: Option<std::path::PathBuf>,
    pub auto_repair: bool,
    pub is_scanning: bool,
}
