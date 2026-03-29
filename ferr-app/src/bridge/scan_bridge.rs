use iced::Subscription;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub enum ScanMsg {
    Progress(Arc<ferr_verify::ScanProgress>),
    Done(Arc<ferr_verify::BitRotReport>),
    Error(String),
}

impl std::fmt::Debug for ScanMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Progress(_) => f.write_str("ScanMsg::Progress"),
            Self::Done(_) => f.write_str("ScanMsg::Done"),
            Self::Error(e) => write!(f, "ScanMsg::Error({e})"),
        }
    }
}

pub fn scan_subscription(_dest: PathBuf) -> Subscription<ScanMsg> {
    Subscription::none() // Stub
}
