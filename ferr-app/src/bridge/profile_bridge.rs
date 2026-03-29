use ferr_core::CopyProfile;

pub fn get_profiles() -> Vec<CopyProfile> {
    ferr_core::list_profiles().unwrap_or_default()
}

pub fn save_profile(profile: &CopyProfile) -> anyhow::Result<()> {
    ferr_core::save_profile(profile)
}
