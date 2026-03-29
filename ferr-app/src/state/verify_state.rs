#[derive(Default)]
pub struct VerifyState {
    pub source: Option<std::path::PathBuf>,
    pub dest: Option<std::path::PathBuf>,
    pub is_verifying: bool,
}
