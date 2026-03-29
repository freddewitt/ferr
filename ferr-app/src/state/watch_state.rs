#[derive(Default)]
pub struct WatchState {
    pub is_active: bool,
    pub selected_profile: Option<String>,
    pub auto_eject: bool,
    pub delay_secs: u64,
}
