use ferr_core::CopyJob;
use iced::Subscription;

#[derive(Debug, Clone)]
pub enum CopyMsg {
    Progress(ferr_core::CopyProgress),
    Done(ferr_report::Manifest),
    Error(String),
}

pub fn copy_subscription(_job: CopyJob) -> Subscription<CopyMsg> {
    Subscription::none() // Stub pour l'instant pour valider le squelette
}
