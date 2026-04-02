// volume.rs — detect mounted volumes and disk usage

use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct VolumeInfo {
    pub name: String,
    pub mount: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub removable: bool,
}

/// List mounted volumes with disk usage.
///
/// On macOS, reads /Volumes/ and queries each with statvfs.
/// On Linux, reads /proc/mounts. On Windows, uses GetDriveType.
pub fn list_volumes() -> Vec<VolumeInfo> {
    #[cfg(target_os = "macos")]
    return list_macos();

    #[cfg(target_os = "linux")]
    return list_linux();

    #[cfg(target_os = "windows")]
    return list_windows();

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    return vec![];
}

#[cfg(target_os = "macos")]
fn list_macos() -> Vec<VolumeInfo> {
    let volumes_dir = Path::new("/Volumes");
    let Ok(entries) = std::fs::read_dir(volumes_dir) else {
        return vec![];
    };

    let mut vols = Vec::new();
    for entry in entries.flatten() {
        let mount = entry.path();
        let name = mount.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if let Some(stats) = statvfs(&mount) {
            vols.push(VolumeInfo {
                removable: name != "Macintosh HD" && !name.is_empty(),
                name,
                mount: mount.to_string_lossy().to_string(),
                total: stats.0,
                used: stats.0.saturating_sub(stats.1),
                free: stats.1,
            });
        }
    }
    vols
}

#[cfg(target_os = "linux")]
fn list_linux() -> Vec<VolumeInfo> {
    let Ok(content) = std::fs::read_to_string("/proc/mounts") else {
        return vec![];
    };

    let mut vols = Vec::new();
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 { continue; }
        let mount = Path::new(parts[1]);
        if !mount.starts_with("/media") && !mount.starts_with("/mnt") && parts[1] != "/" {
            continue;
        }
        let name = mount.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| parts[1].to_string());

        if let Some(stats) = statvfs(mount) {
            vols.push(VolumeInfo {
                removable: mount.starts_with("/media"),
                name,
                mount: parts[1].to_string(),
                total: stats.0,
                used: stats.0.saturating_sub(stats.1),
                free: stats.1,
            });
        }
    }
    vols
}

#[cfg(target_os = "windows")]
fn list_windows() -> Vec<VolumeInfo> {
    // Enumerate drive letters A-Z
    let mut vols = Vec::new();
    for c in b'A'..=b'Z' {
        let mount = format!("{}:\\", c as char);
        let path = Path::new(&mount);
        if path.exists() {
            if let Some(stats) = statvfs(path) {
                vols.push(VolumeInfo {
                    removable: true,
                    name: mount.trim_end_matches('\\').to_string(),
                    mount: mount.clone(),
                    total: stats.0,
                    used: stats.0.saturating_sub(stats.1),
                    free: stats.1,
                });
            }
        }
    }
    vols
}

/// Returns (total_bytes, free_bytes) for the given path via statvfs/statfs.
fn statvfs(path: &Path) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;

        let cpath = CString::new(path.to_string_lossy().as_bytes()).ok()?;
        let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();
        let ret = unsafe { libc::statvfs(cpath.as_ptr(), stat.as_mut_ptr()) };
        if ret != 0 { return None; }
        let stat = unsafe { stat.assume_init() };
        let total = (stat.f_blocks as u64) * (stat.f_frsize as u64);
        let free  = (stat.f_bavail as u64) * (stat.f_frsize as u64);
        Some((total, free))
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        let mut free_caller = 0u64;
        let mut total = 0u64;
        let mut total_free = 0u64;
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free_caller,
                &mut total,
                &mut total_free,
            )
        };
        if ok == 0 { return None; }
        Some((total, free_caller))
    }

    #[cfg(not(any(unix, windows)))]
    None
}
