pub struct CopyState {
    pub source: Option<std::path::PathBuf>,
    pub dest_main: Option<std::path::PathBuf>,
    pub dest_alt: Option<std::path::PathBuf>,
    // Options
    pub hash_algo: ferr_core::HashAlgo,
    pub par2_enabled: bool,
    pub par2_pct: u8,
    pub camera_mode: bool,
    pub auto_eject: bool,
    pub dry_run: bool,
    // Status
    pub is_copying: bool,
}

impl Default for CopyState {
    fn default() -> Self {
        Self {
            source: None,
            dest_main: None,
            dest_alt: None,
            hash_algo: ferr_core::HashAlgo::XxHash64,
            par2_enabled: false,
            par2_pct: 10,
            camera_mode: false,
            auto_eject: false,
            dry_run: false,
            is_copying: false,
        }
    }
}
