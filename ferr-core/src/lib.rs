use std::path::{Path, PathBuf};
use std::time::Instant;

pub use ferr_camera::{CameraFormat, Clip};
pub use ferr_hash::HashAlgo;

// ---------------------------------------------------------------------------
// Énumérations et structures publiques
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyPhase {
    Copying,
    Verifying,
    GeneratingPar2,
    Done,
}

#[derive(Debug, Clone)]
pub struct CopyProgress {
    pub current_file: PathBuf,
    pub file_bytes_done: u64,
    pub file_bytes_total: u64,
    pub total_files_done: usize,
    pub total_files: usize,
    pub speed_bytes_sec: u64,
    pub errors: usize,
    pub phase: CopyPhase,
    pub dedup_skipped: usize,
}

pub struct CopyJob {
    pub source: PathBuf,
    pub destinations: Vec<PathBuf>,
    pub hash_algo: HashAlgo,
    pub resume: bool,
    pub par2_redundancy: Option<u8>,
    pub preserve_metadata: bool,
    pub camera_mode: bool,
    pub rename_template: Option<String>,
    pub auto_eject: bool,
    pub dedup: bool,
    pub generate_pdf: bool,
    pub send_notify: bool,
    pub record_session: bool,
}

impl Default for CopyJob {
    fn default() -> Self {
        Self {
            source: PathBuf::new(),
            destinations: Vec::new(),
            hash_algo: HashAlgo::XxHash64,
            resume: false,
            par2_redundancy: None,
            preserve_metadata: true,
            camera_mode: false,
            rename_template: None,
            auto_eject: false,
            dedup: false,
            generate_pdf: true,
            send_notify: true,
            record_session: true,
        }
    }
}

// ---------------------------------------------------------------------------
// SpaceCheck (feat 2)
// ---------------------------------------------------------------------------

pub struct SpaceCheck {
    pub destination: PathBuf,
    pub available_bytes: u64,
    pub required_bytes: u64,
    pub ok: bool,
    pub delta_bytes: i64,
}

pub fn check_space(
    source: &Path,
    destinations: &[PathBuf],
    par2_redundancy: Option<u8>,
) -> anyhow::Result<Vec<SpaceCheck>> {
    let source_size = dir_size(source)?;
    let par2_extra = par2_redundancy
        .map(|pct| source_size * pct as u64 / 100)
        .unwrap_or(0);
    let required = source_size + par2_extra;

    let mut checks = Vec::new();
    for dest in destinations {
        let avail_path = find_existing_ancestor(dest);
        let available = available_space(&avail_path)?;
        let ok = available >= required;
        checks.push(SpaceCheck {
            destination: dest.clone(),
            available_bytes: available,
            required_bytes: required,
            ok,
            delta_bytes: available as i64 - required as i64,
        });
    }
    Ok(checks)
}

// ---------------------------------------------------------------------------
// Profils (feat 7)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CopyProfile {
    pub name: String,
    pub created_at: String,
    pub destinations: Vec<PathBuf>,
    pub hash_algo: String,
    pub par2_redundancy: Option<u8>,
    pub camera_mode: bool,
    pub auto_eject: bool,
}

fn profiles_dir() -> PathBuf {
    if let Ok(d) = std::env::var("FERR_CONFIG_DIR") {
        return PathBuf::from(d).join("profiles");
    }
    #[cfg(unix)]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home)
            .join(".config")
            .join("ferr")
            .join("profiles")
    }
    #[cfg(not(unix))]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(appdata).join("ferr").join("profiles")
    }
}

pub fn save_profile(profile: &CopyProfile) -> anyhow::Result<()> {
    let dir = profiles_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", profile.name));
    let json = serde_json::to_string_pretty(profile)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load_profile(name: &str) -> anyhow::Result<CopyProfile> {
    let path = profiles_dir().join(format!("{name}.json"));
    let data = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

pub fn list_profiles() -> anyhow::Result<Vec<CopyProfile>> {
    let dir = profiles_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut profiles = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let data = std::fs::read_to_string(&path)?;
            if let Ok(p) = serde_json::from_str::<CopyProfile>(&data) {
                profiles.push(p);
            }
        }
    }
    Ok(profiles)
}

pub fn delete_profile(name: &str) -> anyhow::Result<()> {
    let path = profiles_dir().join(format!("{name}.json"));
    std::fs::remove_file(path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Éjection automatique (feat 8)
// ---------------------------------------------------------------------------

pub fn eject_volume(mount_point: &Path) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("diskutil")
            .args(["eject", &mount_point.to_string_lossy()])
            .status()?;
        if !status.success() {
            anyhow::bail!("diskutil eject a échoué sur {}", mount_point.display());
        }
        return Ok(());
    }

    #[cfg(windows)]
    {
        // Sur Windows : utiliser DeviceIoControl via PowerShell comme fallback simple
        let drive = mount_point
            .to_string_lossy()
            .trim_end_matches('\\')
            .to_string();
        let script = format!(
            "(New-Object -comObject Shell.Application).Namespace(17).ParseName(\"{drive}\").InvokeVerb(\"Eject\")"
        );
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .status()?;
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    {
        anyhow::bail!("Éjection non supportée sur cette plateforme");
    }
}

// ---------------------------------------------------------------------------
// Dry-run (feat 12)
// ---------------------------------------------------------------------------

pub struct DryRunReport {
    pub files: Vec<PathBuf>,
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub par2_size_bytes: u64,
    pub space_checks: Vec<SpaceCheck>,
    pub estimated_secs: u64,
    pub clips: Option<Vec<Clip>>,
}

const DRY_RUN_SPEED_BPS: u64 = 300 * 1_000_000; // 300 Mo/s

pub fn dry_run(job: &CopyJob) -> anyhow::Result<DryRunReport> {
    let files = collect_files(&job.source)?;
    let total_size_bytes: u64 = files
        .iter()
        .map(|f| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0))
        .sum();

    let par2_size_bytes = job
        .par2_redundancy
        .map(|pct| total_size_bytes * pct as u64 / 100)
        .unwrap_or(0);

    let space_checks = check_space(&job.source, &job.destinations, job.par2_redundancy)?;

    let estimated_secs = (total_size_bytes + par2_size_bytes)
        .checked_div(DRY_RUN_SPEED_BPS)
        .unwrap_or(0);

    let clips = if job.camera_mode {
        Some(ferr_camera::scan_clips(&job.source, None)?)
    } else {
        None
    };

    Ok(DryRunReport {
        files: files.clone(),
        total_files: files.len(),
        total_size_bytes,
        par2_size_bytes,
        space_checks,
        estimated_secs,
        clips,
    })
}

// ---------------------------------------------------------------------------
// Watch mode (feat 6)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct CopyJobTemplate {
    pub destinations: Vec<PathBuf>,
    pub hash_algo_str: String,
    pub resume: bool,
    pub par2_redundancy: Option<u8>,
    pub camera_mode: bool,
    pub preserve_metadata: bool,
    pub rename_template: Option<String>,
    pub auto_eject: bool,
}

pub struct WatchConfig {
    pub mount_point: PathBuf,
    pub copy_job: CopyJobTemplate,
    pub delay_secs: u64,
    pub auto_eject: bool,
}

pub enum WatchEvent {
    Waiting,
    VolumeDetected {
        name: String,
        path: PathBuf,
        size: u64,
    },
    CopyStarting {
        volume: String,
    },
    CopyProgress(CopyProgress),
    CopyDone {
        volume: String,
        manifest: ferr_report::Manifest,
    },
    Ejected {
        volume: String,
    },
    Error {
        volume: String,
        error: String,
    },
}

pub fn run_watch(
    config: WatchConfig,
    on_event: impl Fn(WatchEvent) + Send + Sync,
) -> anyhow::Result<()> {
    use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.send(res);
        },
        Config::default(),
    )?;

    watcher.watch(&config.mount_point, RecursiveMode::NonRecursive)?;
    on_event(WatchEvent::Waiting);

    for result in &rx {
        let event = match result {
            Ok(e) => e,
            Err(e) => {
                eprintln!("ferr watch : erreur de surveillance : {e}");
                continue;
            }
        };

        if matches!(event.kind, EventKind::Create(_)) {
            for path in event.paths {
                if !path.is_dir() {
                    continue;
                }
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string_lossy().into_owned());

                let size = dir_size(&path).unwrap_or(0);
                on_event(WatchEvent::VolumeDetected {
                    name: name.clone(),
                    path: path.clone(),
                    size,
                });

                // Attendre delay_secs
                std::thread::sleep(std::time::Duration::from_secs(config.delay_secs));
                on_event(WatchEvent::CopyStarting {
                    volume: name.clone(),
                });

                let hash_algo = match config.copy_job.hash_algo_str.as_str() {
                    "sha256" => HashAlgo::Sha256,
                    _ => HashAlgo::XxHash64,
                };

                let job = CopyJob {
                    source: path.clone(),
                    destinations: config.copy_job.destinations.clone(),
                    hash_algo,
                    resume: config.copy_job.resume,
                    par2_redundancy: config.copy_job.par2_redundancy,
                    preserve_metadata: config.copy_job.preserve_metadata,
                    camera_mode: config.copy_job.camera_mode,
                    rename_template: config.copy_job.rename_template.clone(),
                    auto_eject: false,
                    ..Default::default()
                };

                match run_copy(job, |p| on_event(WatchEvent::CopyProgress(p))) {
                    Ok(manifest) => {
                        on_event(WatchEvent::CopyDone {
                            volume: name.clone(),
                            manifest,
                        });
                        if config.auto_eject || config.copy_job.auto_eject {
                            match eject_volume(&path) {
                                Ok(()) => on_event(WatchEvent::Ejected { volume: name }),
                                Err(e) => on_event(WatchEvent::Error {
                                    volume: name,
                                    error: format!("Éjection : {e}"),
                                }),
                            }
                        }
                    }
                    Err(e) => {
                        on_event(WatchEvent::Error {
                            volume: name,
                            error: e.to_string(),
                        });
                    }
                }

                on_event(WatchEvent::Waiting);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// run_copy
// ---------------------------------------------------------------------------

pub fn run_copy(
    job: CopyJob,
    on_progress: impl Fn(CopyProgress) + Send,
) -> anyhow::Result<ferr_report::Manifest> {
    let start = Instant::now();

    let hasher: Box<dyn ferr_hash::Hasher> = match job.hash_algo {
        HashAlgo::XxHash64 => Box::new(ferr_hash::XxHasher),
        HashAlgo::Sha256 => Box::new(ferr_hash::Sha256Hasher),
    };

    let destinations: Vec<Box<dyn ferr_transfer::Destination>> = job
        .destinations
        .iter()
        .map(|p| -> Box<dyn ferr_transfer::Destination> {
            Box::new(ferr_transfer::LocalDest { root: p.clone() })
        })
        .collect();

    let resume_manifest: Option<ferr_report::Manifest> = if job.resume {
        job.destinations
            .first()
            .map(|d| d.join("ferr-manifest.json"))
            .and_then(|p| ferr_report::load_manifest(&p).ok())
    } else {
        None
    };

    // Vérification espace disque
    let space_checks =
        check_space(&job.source, &job.destinations, job.par2_redundancy).unwrap_or_default();
    for check in &space_checks {
        if !check.ok {
            anyhow::bail!(
                "Espace insuffisant sur {} : manque {} octets",
                check.destination.display(),
                check.delta_bytes.unsigned_abs()
            );
        }
    }

    let src_files = collect_files(&job.source)?;
    let total_files = src_files.len();
    let mut file_entries: Vec<ferr_report::FileEntry> = Vec::new();
    let mut total_size_bytes = 0u64;
    let mut errors = 0usize;
    let mut dedup_skipped = 0usize;
    let global_start = Instant::now();

    for (idx, src_file) in src_files.iter().enumerate() {
        let rel = src_file.strip_prefix(&job.source)?;
        let file_size = std::fs::metadata(src_file).map(|m| m.len()).unwrap_or(0);
        let modified_at = std::fs::metadata(src_file)
            .and_then(|m| m.modified())
            .map(|t| {
                let secs = t
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                format_unix_time(secs)
            })
            .unwrap_or_else(|_| "unknown".to_string());

        // Appliquer le renommage si template fourni
        let dest_rel = if let Some(tmpl) = &job.rename_template {
            // On a besoin du clip pour le renommage — scan simplifié
            let pseudo_clip = ferr_camera::Clip {
                id: rel.to_string_lossy().into_owned(),
                format: ferr_camera::CameraFormat::Unknown,
                primary_file: src_file.clone(),
                sidecar_files: Vec::new(),
                total_size: file_size,
                metadata: ferr_camera::ClipMetadata::default(),
            };
            match ferr_camera::apply_rename_template(&pseudo_clip, tmpl) {
                Ok(name) => {
                    let mut renamed = rel.to_path_buf();
                    renamed.set_file_name(name);
                    renamed
                }
                Err(_) => rel.to_path_buf(),
            }
        } else {
            rel.to_path_buf()
        };

        // Déduplication
        if job.dedup {
            let temp_hash = hasher.hash_file(src_file);
            if let Ok(h) = temp_hash {
                if let Ok(records) = ferr_session::find_file_by_hash(&h.hex) {
                    if !records.is_empty() {
                        dedup_skipped += 1;
                        on_progress(CopyProgress {
                            current_file: rel.to_path_buf(),
                            file_bytes_done: file_size,
                            file_bytes_total: file_size,
                            total_files_done: idx,
                            total_files,
                            speed_bytes_sec: 0,
                            errors,
                            phase: CopyPhase::Copying,
                            dedup_skipped,
                        });
                        file_entries.push(ferr_report::FileEntry {
                            path: dest_rel.to_string_lossy().replace('\\', "/"),
                            size: file_size,
                            hash_algo: job.hash_algo.to_string(),
                            hash: h.hex,
                            modified_at,
                            status: ferr_report::FileStatus::Skipped,
                            par2_generated: false,
                        });
                        continue;
                    }
                }
            }
        }

        on_progress(CopyProgress {
            current_file: rel.to_path_buf(),
            file_bytes_done: 0,
            file_bytes_total: file_size,
            total_files_done: idx,
            total_files,
            speed_bytes_sec: speed_bytes_sec(
                total_size_bytes,
                global_start.elapsed().as_secs_f64(),
            ),
            errors,
            phase: CopyPhase::Copying,
            dedup_skipped,
        });

        let result = ferr_transfer::copy_file(
            src_file,
            &dest_rel,
            &destinations,
            hasher.as_ref(),
            resume_manifest.as_ref(),
            |bytes_done| {
                on_progress(CopyProgress {
                    current_file: rel.to_path_buf(),
                    file_bytes_done: bytes_done,
                    file_bytes_total: file_size,
                    total_files_done: idx,
                    total_files,
                    speed_bytes_sec: speed_bytes_sec(
                        total_size_bytes + bytes_done,
                        global_start.elapsed().as_secs_f64(),
                    ),
                    errors,
                    phase: CopyPhase::Copying,
                    dedup_skipped,
                });
            },
            job.preserve_metadata,
        );

        match result {
            Ok(transfer) => {
                total_size_bytes += transfer.src_hash.bytes_read;
                let all_ok = transfer.dest_results.iter().all(|r| r.success);
                let status = if transfer.skipped {
                    ferr_report::FileStatus::Skipped
                } else if all_ok {
                    ferr_report::FileStatus::Ok
                } else {
                    errors += 1;
                    ferr_report::FileStatus::Corrupted
                };
                file_entries.push(ferr_report::FileEntry {
                    path: dest_rel.to_string_lossy().replace('\\', "/"),
                    size: transfer.src_hash.bytes_read,
                    hash_algo: transfer.src_hash.algo.to_string(),
                    hash: transfer.src_hash.hex,
                    modified_at,
                    status,
                    par2_generated: false,
                });
            }
            Err(e) => {
                errors += 1;
                eprintln!("Erreur sur {} : {e}", rel.display());
                file_entries.push(ferr_report::FileEntry {
                    path: dest_rel.to_string_lossy().replace('\\', "/"),
                    size: file_size,
                    hash_algo: job.hash_algo.to_string(),
                    hash: String::new(),
                    modified_at,
                    status: ferr_report::FileStatus::Corrupted,
                    par2_generated: false,
                });
            }
        }
    }

    // --- Phase PAR2 ---
    if let Some(pct) = job.par2_redundancy {
        on_progress(CopyProgress {
            current_file: PathBuf::from("(par2)"),
            file_bytes_done: 0,
            file_bytes_total: 0,
            total_files_done: total_files,
            total_files,
            speed_bytes_sec: 0,
            errors,
            phase: CopyPhase::GeneratingPar2,
            dedup_skipped,
        });
        for dest_path in &job.destinations {
            let par2_out = dest_path.join("_par2");
            match ferr_par2::generate(dest_path, &par2_out, pct, |_| {}) {
                Ok(_) => {
                    for e in &mut file_entries {
                        e.par2_generated = true;
                    }
                }
                Err(e) => eprintln!("PAR2 non disponible : {e}"),
            }
        }
    }

    // --- Finalisation ---
    let duration_secs = start.elapsed().as_secs_f64();
    let status = if errors == 0 {
        ferr_report::JobStatus::Ok
    } else if errors < total_files {
        ferr_report::JobStatus::Partial
    } else {
        ferr_report::JobStatus::Failed
    };

    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "unknown".to_string());

    let manifest = ferr_report::Manifest {
        ferr_version: env!("CARGO_PKG_VERSION").to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        hostname,
        source_path: job.source.to_string_lossy().into_owned(),
        total_files,
        total_size_bytes,
        duration_secs,
        status,
        files: file_entries,
    };

    // Sauvegarder le manifest dans chaque destination
    for dest_path in &job.destinations {
        let mp = dest_path.join("ferr-manifest.json");
        if let Err(e) = ferr_report::save_manifest(&manifest, &mp) {
            eprintln!("Manifest non sauvegardé dans {} : {e}", dest_path.display());
        }
    }

    // Générer le PDF
    if job.generate_pdf {
        let pdf_name = format!(
            "ferr_report_{}.pdf",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );
        for dest_path in &job.destinations {
            let pdf_path = dest_path.join(&pdf_name);
            if let Err(e) = ferr_pdf::generate_report(&manifest, &pdf_path) {
                eprintln!("PDF non généré : {e}");
            }
        }
    }

    // Enregistrer la session
    if job.record_session {
        if let Err(e) = ferr_session::record_session(&manifest) {
            eprintln!("Session non enregistrée : {e}");
        }
    }

    // Notification
    if job.send_notify {
        let title = "ferr — Copie terminée";
        let msg = format!(
            "{} fichiers · {} · {:.1}s",
            total_files,
            human_size(total_size_bytes),
            duration_secs
        );
        let _ = ferr_notify::notify_done(title, &msg, errors == 0);
    }

    // Éjection automatique
    if job.auto_eject && errors == 0 {
        if let Err(e) = eject_volume(&job.source) {
            eprintln!("Éjection échouée : {e}");
        }
    }

    on_progress(CopyProgress {
        current_file: PathBuf::from("(terminé)"),
        file_bytes_done: total_size_bytes,
        file_bytes_total: total_size_bytes,
        total_files_done: total_files,
        total_files,
        speed_bytes_sec: speed_bytes_sec(total_size_bytes, duration_secs),
        errors,
        phase: CopyPhase::Done,
        dedup_skipped,
    });

    Ok(manifest)
}

// ---------------------------------------------------------------------------
// Utilitaires internes
// ---------------------------------------------------------------------------

fn collect_files(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_recursive(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Some(name) = path.file_name() {
            let n = name.to_string_lossy();
            if n == "ferr-manifest.json" || n == "_par2" || n.ends_with(".pdf") {
                continue;
            }
        }
        if path.is_dir() {
            collect_recursive(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn dir_size(dir: &Path) -> anyhow::Result<u64> {
    let mut total = 0u64;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            total += dir_size(&path)?;
        } else {
            total += std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok(total)
}

fn find_existing_ancestor(path: &Path) -> PathBuf {
    let mut p = path.to_path_buf();
    while !p.exists() {
        match p.parent() {
            Some(parent) => p = parent.to_path_buf(),
            None => return PathBuf::from("/"),
        }
    }
    p
}

#[cfg(unix)]
fn available_space(path: &Path) -> anyhow::Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(stat.f_bavail as u64 * stat.f_frsize as u64)
}

#[cfg(windows)]
fn available_space(path: &Path) -> anyhow::Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut available = 0u64;
    let mut total = 0u64;
    let mut free = 0u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available as *mut u64,
            &mut total as *mut u64,
            &mut free as *mut u64,
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(available)
}

#[cfg(not(any(unix, windows)))]
fn available_space(_path: &Path) -> anyhow::Result<u64> {
    Ok(u64::MAX) // plateforme non gérée : pas de blocage
}

fn speed_bytes_sec(bytes: u64, secs: f64) -> u64 {
    if secs < 0.001 {
        return 0;
    }
    (bytes as f64 / secs) as u64
}

fn format_unix_time(secs: u64) -> String {
    let dt =
        chrono::DateTime::from_timestamp(secs as i64, 0).unwrap_or_else(chrono::DateTime::default);
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn human_size(bytes: u64) -> String {
    const GB: u64 = 1_000_000_000;
    const MB: u64 = 1_000_000;
    const KB: u64 = 1_000;
    if bytes >= GB {
        format!("{:.2} Go", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} Mo", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} Ko", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} o")
    }
}
