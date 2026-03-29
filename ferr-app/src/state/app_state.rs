#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Copy,
    Watch,
    Verify,
    History,
    Profiles,
    Scan,
    Camera,
}

pub struct AppState {
    pub current_tab: Tab,
    pub copy: super::copy_state::CopyState,
    pub watch: super::watch_state::WatchState,
    pub verify: super::verify_state::VerifyState,
    pub history: super::history_state::HistoryState,
    pub profiles: super::profile_state::ProfileState,
    pub scan: super::scan_state::ScanState,
    pub camera: super::camera_state::CameraState,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            current_tab: Tab::Copy,
            copy: super::copy_state::CopyState::default(),
            watch: super::watch_state::WatchState::default(),
            verify: super::verify_state::VerifyState::default(),
            history: super::history_state::HistoryState::default(),
            profiles: super::profile_state::ProfileState::default(),
            scan: super::scan_state::ScanState::default(),
            camera: super::camera_state::CameraState::default(),
        }
    }
}
