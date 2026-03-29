use iced::Subscription;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub enum VerifyMsg {
    Progress(String), // Simplifié
    Done(Arc<ferr_verify::VerifyReport>),
    Error(String),
}

impl std::fmt::Debug for VerifyMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Progress(p) => write!(f, "VerifyMsg::Progress({p})"),
            Self::Done(_) => f.write_str("VerifyMsg::Done"),
            Self::Error(e) => write!(f, "VerifyMsg::Error({e})"),
        }
    }
}

pub fn verify_subscription(_src: PathBuf, _dest: PathBuf) -> Subscription<VerifyMsg> {
    Subscription::none() // Stub
}
