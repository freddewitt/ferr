use iced::Subscription;
use ferr_core::{WatchConfig, WatchEvent};
use std::sync::Arc;

#[derive(Clone)]
pub enum WatchMsg {
    Event(Arc<WatchEvent>),
    Error(String),
}

impl std::fmt::Debug for WatchMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Event(_) => f.write_str("WatchMsg::Event"),
            Self::Error(e) => write!(f, "WatchMsg::Error({e})"),
        }
    }
}

pub fn watch_subscription(_config: WatchConfig) -> Subscription<WatchMsg> {
    Subscription::none() // Stub
}
