use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};
use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

// ---------------------------------------------------------------------------
// Display constants
// ---------------------------------------------------------------------------

/// Max display width for a file name (with "…" prefix)
const FILE_NAME_DISPLAY_MAX: usize = 40;
/// Max display width for a destination path
const DEST_PATH_DISPLAY_MAX: usize = 30;
/// Width of horizontal separators in tables
const TABLE_WIDTH: usize = 80;
/// Width of horizontal separators in history
const HISTORY_TABLE_WIDTH: usize = 70;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "ferr",
    about = "Secure file copy with hash verification and PAR2 redundancy",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, ValueEnum)]
enum HashChoice {
    Xxhash,
    Sha256,
}

#[derive(Subcommand)]
enum Commands {
    /// Copy files with hash verification
    Copy {
        /// Source directory (directories only — not single files)
        src: PathBuf,
        dest: PathBuf,
        #[arg(long)]
        dest2: Option<PathBuf>,
        #[arg(long)]
        dest3: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "xxhash")]
        hash: HashChoice,
        #[arg(long)]
        par2: Option<u8>,
        #[arg(long)]
        resume: bool,
        #[arg(long)]
        camera: bool,
        #[arg(long)]
        rename: Option<String>,
        #[arg(long)]
        eject: bool,
        #[arg(long)]
        dedup: bool,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        no_preserve_meta: bool,
        #[arg(long)]
        no_notify: bool,
        #[arg(long)]
        pdf: bool,
        /// Save a ferr-manifest.json to the destination (legacy, kept for compatibility)
        #[arg(long, hide = true)]
        report: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        quiet: bool,
        /// Progress output format: "human" (default) or "machine"
        #[arg(long, value_name = "FORMAT", default_value = "human")]
        progress_format: String,
    },
    /// Verify file integrity against a .ferrcert, source directory, or legacy manifest
    Verify {
        /// Path to a .ferrcert, a directory (auto-detects cert), or a legacy .json manifest
        cert_or_dir: PathBuf,
        dest: PathBuf,
        #[arg(long)]
        quiet: bool,
    },
    /// Repair corrupted files via PAR2
    Repair { manifest: PathBuf, dest: PathBuf },
    /// Detect bit rot on a destination (uses .ferrcert if present, else legacy manifest)
    Scan {
        dest: PathBuf,
        /// Override auto-detection: path to .ferrcert or legacy ferr-manifest.json
        #[arg(long = "manifest", alias = "cert")]
        cert_or_manifest: Option<PathBuf>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        quiet: bool,
    },
    /// Watch a mount point and copy automatically on volume detection
    Watch {
        mount_point: PathBuf,
        #[arg(long)]
        dest: Vec<PathBuf>,
        #[arg(long, value_enum, default_value = "xxhash")]
        hash: HashChoice,
        #[arg(long)]
        par2: Option<u8>,
        #[arg(long)]
        camera: bool,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long, default_value = "3")]
        delay: u64,
        #[arg(long)]
        eject: bool,
        #[arg(long)]
        quiet: bool,
    },
    /// Export a manifest to ALE or CSV
    Export {
        manifest: PathBuf,
        #[arg(long, default_value = "csv")]
        format: ExportFormat,
        #[arg(long)]
        output: PathBuf,
    },
    /// Generate a PDF report from a manifest
    Report {
        manifest: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Manage copy profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Manage session history
    History {
        #[command(subcommand)]
        action: HistoryAction,
    },
    /// Manage integrity certificates
    Cert {
        #[command(subcommand)]
        action: CertAction,
    },
}

#[derive(Clone, ValueEnum)]
enum ExportFormat {
    Ale,
    Csv,
}

#[derive(Subcommand)]
enum ProfileAction {
    Save {
        name: String,
        #[arg(long)]
        dest: Vec<PathBuf>,
        #[arg(long, value_enum, default_value = "xxhash")]
        hash: HashChoice,
        #[arg(long)]
        par2: Option<u8>,
        #[arg(long)]
        camera: bool,
        #[arg(long)]
        eject: bool,
    },
    List,
    Show {
        name: String,
    },
    Delete {
        name: String,
    },
}

#[derive(Subcommand)]
enum HistoryAction {
    List {
        #[arg(long, default_value = "20")]
        limit: usize,
        #[arg(long)]
        since: Option<String>,
    },
    Show {
        id: i64,
    },
    Find {
        hash_or_name: String,
    },
}

#[derive(Subcommand)]
enum CertAction {
    /// Certify a source directory (creates a .ferrcert at the root)
    Create {
        /// Source directory (directories only — not single files)
        src: PathBuf,
        /// Output path for the .ferrcert (default: <src>/<label>_<date>.ferrcert)
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "xxhash")]
        hash: HashChoice,
        #[arg(long)]
        quiet: bool,
    },
    /// Verify a directory against a .ferrcert
    Verify {
        /// Path to a .ferrcert file, or a directory containing one
        cert: PathBuf,
        /// Target directory to verify
        dest: PathBuf,
        #[arg(long)]
        quiet: bool,
    },
    /// Show the event journal of a .ferrcert (and verify cert integrity)
    Show {
        /// Path to a .ferrcert file, or a directory containing one
        cert_or_dir: PathBuf,
    },
    /// Check only the integrity of a .ferrcert (does not verify files)
    Check {
        /// Path to a .ferrcert file, or a directory containing one
        cert_or_dir: PathBuf,
    },
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    // Structured logging — level controllable via RUST_LOG (e.g. RUST_LOG=warn)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    // Respect NO_COLOR
    if std::env::var("NO_COLOR").is_ok() {
        console::set_colors_enabled(false);
    }

    let result = run(cli);
    match result {
        Ok(code) => process::exit(code),
        Err(e) => {
            eprintln!("{} {e}", style("Error:").red().bold());
            process::exit(2);
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        Commands::Copy {
            src,
            dest,
            dest2,
            dest3,
            hash,
            par2,
            resume,
            camera,
            rename,
            eject,
            dedup,
            profile,
            no_preserve_meta,
            no_notify,
            pdf,
            report,
            dry_run,
            quiet,
            progress_format,
        } => cmd_copy(CopyArgs {
            src,
            dest,
            dest2,
            dest3,
            hash,
            par2,
            resume,
            camera,
            rename,
            eject,
            dedup,
            profile,
            no_preserve_meta,
            no_notify,
            pdf,
            report,
            dry_run,
            quiet,
            progress_format,
        }),
        Commands::Verify {
            cert_or_dir,
            dest,
            quiet,
        } => cmd_verify(cert_or_dir, dest, quiet),
        Commands::Repair { manifest, dest } => cmd_repair(manifest, dest),
        Commands::Scan {
            dest,
            cert_or_manifest,
            since,
            quiet,
        } => cmd_scan(dest, cert_or_manifest, since, quiet),
        Commands::Watch {
            mount_point,
            dest,
            hash,
            par2,
            camera,
            profile,
            delay,
            eject,
            quiet,
        } => cmd_watch(
            mount_point,
            dest,
            hash,
            par2,
            camera,
            profile,
            delay,
            eject,
            quiet,
        ),
        Commands::Export {
            manifest,
            format,
            output,
        } => cmd_export(manifest, format, output),
        Commands::Report { manifest, output } => cmd_report(manifest, output),
        Commands::Profile { action } => cmd_profile(action),
        Commands::History { action } => cmd_history(action),
        Commands::Cert { action } => cmd_cert(action),
    }
}

// ---------------------------------------------------------------------------
// Post-copy hooks
// ---------------------------------------------------------------------------

/// Generates a PDF report in each destination directory.
struct PdfHook;
impl ferr_core::PostCopyHook for PdfHook {
    fn on_copy_done(&self, manifest: &ferr_report::Manifest) -> anyhow::Result<()> {
        let log_dir_name = ferr_core::get_log_dir_name(manifest).unwrap_or_else(|_| "logs".into());
        let pdf_name = "ferr_report.pdf";
        for dest_str in &manifest.destinations {
            let log_dir = PathBuf::from(dest_str).join(&log_dir_name);
            std::fs::create_dir_all(&log_dir).ok();
            let pdf_path = log_dir.join(pdf_name);
            if let Err(e) = ferr_pdf::generate_report(manifest, &pdf_path) {
                eprintln!("PDF not generated in {dest_str}: {e}");
            }
        }
        Ok(())
    }
}

/// Records the session in the local SQLite database.
struct SessionHook;
impl ferr_core::PostCopyHook for SessionHook {
    fn on_copy_done(&self, manifest: &ferr_report::Manifest) -> anyhow::Result<()> {
        ferr_session::record_session(manifest)?;
        Ok(())
    }
}

/// Sends a system notification when the copy finishes.
struct NotifyHook;
impl ferr_core::PostCopyHook for NotifyHook {
    fn on_copy_done(&self, manifest: &ferr_report::Manifest) -> anyhow::Result<()> {
        let title = "ferr — Copy complete";
        let msg = format!(
            "{} files · {} · {:.1}s",
            manifest.total_files,
            ferr_report::human_size(manifest.total_size_bytes),
            manifest.duration_secs,
        );
        let ok = manifest.status == ferr_report::JobStatus::Ok;
        let _ = ferr_notify::notify_done(title, &msg, ok);
        Ok(())
    }
}

/// Builds the hook vector based on CLI flags.
fn build_hooks(pdf: bool, no_notify: bool, no_session: bool) -> Vec<ferr_core::HookRef> {
    let mut hooks: Vec<ferr_core::HookRef> = Vec::new();
    if !no_session {
        hooks.push(Arc::new(SessionHook));
    }
    if pdf {
        hooks.push(Arc::new(PdfHook));
    }
    if !no_notify {
        hooks.push(Arc::new(NotifyHook));
    }
    hooks
}

// ---------------------------------------------------------------------------
// cmd_copy
// ---------------------------------------------------------------------------

/// Grouped arguments for the `copy` command.
struct CopyArgs {
    src: PathBuf,
    dest: PathBuf,
    dest2: Option<PathBuf>,
    dest3: Option<PathBuf>,
    hash: HashChoice,
    par2: Option<u8>,
    resume: bool,
    camera: bool,
    rename: Option<String>,
    eject: bool,
    dedup: bool,
    profile: Option<String>,
    no_preserve_meta: bool,
    no_notify: bool,
    pdf: bool,
    report: bool,
    dry_run: bool,
    quiet: bool,
    progress_format: String,
}

fn cmd_copy(args: CopyArgs) -> anyhow::Result<i32> {
    let CopyArgs {
        src,
        dest,
        dest2,
        dest3,
        hash,
        par2,
        resume,
        camera,
        rename,
        eject,
        dedup,
        profile,
        no_preserve_meta,
        no_notify,
        pdf,
        report,
        dry_run: dry_run_flag,
        quiet,
        progress_format,
    } = args;

    if src.is_file() {
        eprintln!(
            "{} ferr copy only accepts directories as source — not a single file: {}",
            style("Error:").red().bold(),
            src.display()
        );
        return Ok(2);
    }

    let machine_mode = progress_format == "machine";
    let mut destinations = vec![dest];
    if let Some(d) = dest2 {
        destinations.push(d);
    }
    if let Some(d) = dest3 {
        destinations.push(d);
    }
    let (hash_algo, _hash_algo_str) = hash_choice_to_algo(&hash);

    // Load profile if provided
    let (destinations, hash_algo, par2, camera, eject, rename) = if let Some(profile_name) = profile
    {
        match ferr_core::load_profile(&profile_name) {
            Ok(p) => (
                p.destinations,
                ferr_core::HashAlgo::from_lossy(&p.hash_algo),
                p.par2_redundancy,
                p.camera_mode,
                p.auto_eject,
                rename,
            ),
            Err(e) => {
                eprintln!("Profile not found: {e}");
                (destinations, hash_algo, par2, camera, eject, rename)
            }
        }
    } else {
        (destinations, hash_algo, par2, camera, eject, rename)
    };

    let hooks = build_hooks(pdf, no_notify, false);

    let job = ferr_core::CopyJob {
        source: src.clone(),
        destinations: destinations.clone(),
        hash_algo,
        resume,
        par2_redundancy: par2,
        preserve_metadata: !no_preserve_meta,
        camera_mode: camera,
        rename_template: rename,
        auto_eject: eject,
        dedup,
        save_manifest: report,
    };

    // Dry-run mode
    if dry_run_flag {
        let dry_report = ferr_core::dry_run(&job)?;
        if machine_mode {
            let space_ok = dry_report.space_checks.iter().all(|c| c.ok);
            let space_err = if space_ok { 0 } else { 1 };
            for (i, f) in dry_report.files.iter().enumerate() {
                let name = f
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                println!(
                    "PROGRESS:{}/{total}|{}/{files}|300 MB/s|{}",
                    i + 1,
                    i + 1,
                    name,
                    total = dry_report.total_size_bytes,
                    files = dry_report.total_files,
                );
            }
            println!(
                "COMPLETE:{}|{}|{}|",
                dry_report.total_files, dry_report.total_size_bytes, space_err
            );
        } else if !quiet {
            println!(
                "{}",
                style("Dry-run mode — no files written").yellow().bold()
            );
            println!("  Files       : {}", dry_report.total_files);
            println!(
                "  Size        : {}",
                human_size(dry_report.total_size_bytes)
            );
            println!("  PAR2 est.   : {}", human_size(dry_report.par2_size_bytes));
            println!(
                "  Duration est.: {}s (at 300 MB/s)",
                dry_report.estimated_secs
            );
            for check in &dry_report.space_checks {
                if check.ok {
                    println!(
                        "  {} {} — available {}",
                        style("✓").green(),
                        check.destination.display(),
                        human_size(check.available_bytes)
                    );
                } else {
                    println!(
                        "  {} {} — missing {}",
                        style("✗").red(),
                        check.destination.display(),
                        human_size(check.delta_bytes.unsigned_abs())
                    );
                }
            }
            if let Some(clips) = &dry_report.clips {
                println!("  Clips detected: {}", clips.len());
            }
        }
        return Ok(0);
    }

    use std::sync::atomic::{AtomicUsize, Ordering};
    let copy_start = Instant::now();

    if machine_mode {
        // ── Machine-readable mode ──────────────────────────────────────────
        let file_count = Arc::new(AtomicUsize::new(0));
        let file_count = Arc::clone(&file_count);
        let error_count = Arc::new(AtomicUsize::new(0));
        let on_progress = {
            let file_count = Arc::clone(&file_count);
            let error_count = Arc::clone(&error_count);
            move |p: ferr_core::CopyProgress| {
                if matches!(p.phase, ferr_core::CopyPhase::Done) {
                    return;
                }
                let name = p.current_file.to_string_lossy();
                let files_done = p.total_files_done;
                if files_done > file_count.load(Ordering::Relaxed) {
                    file_count.store(files_done, Ordering::Relaxed);
                }
                if p.errors > error_count.load(Ordering::Relaxed) {
                    error_count.store(p.errors, Ordering::Relaxed);
                    println!("ERROR:{}|copy_error|hash mismatch or write failure", name);
                }
                let speed_str = human_size(p.speed_bytes_sec) + "/s";
                println!(
                    "PROGRESS:{}/{}|{}/{}|{}|{}",
                    p.file_bytes_done,
                    p.file_bytes_total,
                    files_done,
                    p.total_files,
                    speed_str,
                    name,
                );
            }
        };

        let manifest = ferr_core::run_copy(job, on_progress, &hooks)?;

        let errors = manifest
            .files
            .iter()
            .filter(|f| matches!(f.status, ferr_report::FileStatus::Corrupted))
            .count();
        let manifest_path = destinations
            .first()
            .and_then(|d| ferr_core::find_manifest_path(d))
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();

        println!(
            "COMPLETE:{}|{}|{}|{}",
            manifest.total_files, manifest.total_size_bytes, errors, manifest_path,
        );
    } else {
        // ── Human mode (indicatif) ─────────────────────────────────────────
        let mp = MultiProgress::new();
        let global_bar = mp.add(ProgressBar::new(0));
        global_bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.cyan} [{elapsed_precise}] {pos}/{len} files  {bytes} copied  {binary_bytes_per_sec}  ETA {eta}",
            )
            .unwrap()
            .progress_chars("=>-"),
        );

        let file_bar = mp.add(ProgressBar::new(0));
        file_bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} {wide_msg:.40} {bar:30.green/white} {bytes}/{total_bytes} {percent:>3}%",
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
        );

        let file_count = Arc::new(AtomicUsize::new(0));

        let on_progress = {
            let global_bar = global_bar.clone();
            let file_bar = file_bar.clone();
            let file_count = Arc::clone(&file_count);
            move |p: ferr_core::CopyProgress| {
                if quiet {
                    return;
                }
                let phase_label = match p.phase {
                    ferr_core::CopyPhase::Copying => "[Copy]",
                    ferr_core::CopyPhase::Verifying => "[Verify]",
                    ferr_core::CopyPhase::GeneratingPar2 => "[PAR2]",
                    ferr_core::CopyPhase::Done => "[Done]",
                };
                match p.phase {
                    ferr_core::CopyPhase::Done => {
                        file_bar.finish_and_clear();
                        global_bar.finish_with_message("Done ✓");
                    }
                    ferr_core::CopyPhase::GeneratingPar2 => {
                        file_bar.set_message("[PAR2] generating…");
                    }
                    _ => {
                        let name = p
                            .current_file
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let trunc = if name.len() > FILE_NAME_DISPLAY_MAX {
                            format!("…{}", &name[name.len() - (FILE_NAME_DISPLAY_MAX - 1)..])
                        } else {
                            name
                        };
                        file_bar.set_length(p.file_bytes_total);
                        file_bar.set_position(p.file_bytes_done);
                        file_bar.set_message(format!("{phase_label} {trunc}"));
                        global_bar.set_length(p.total_files as u64);
                        let done = file_count.load(Ordering::Relaxed);
                        if p.total_files_done > done {
                            file_count.store(p.total_files_done, Ordering::Relaxed);
                            global_bar.set_position(p.total_files_done as u64);
                        }
                    }
                }
            }
        };

        let manifest = ferr_core::run_copy(job, on_progress, &hooks)?;
        if !quiet {
            mp.clear()?;
        }

        if !quiet {
            println!();
            for entry in &manifest.files {
                let icon = match entry.status {
                    ferr_report::FileStatus::Ok => style("✓").green().bold(),
                    ferr_report::FileStatus::Skipped => style("↩").cyan().bold(),
                    ferr_report::FileStatus::Corrupted => style("✗").red().bold(),
                    ferr_report::FileStatus::Missing => style("?").yellow().bold(),
                };
                let hash_preview = &entry.hash[..entry.hash.len().min(16)];
                println!(
                    "  {} {}  [{}] {}",
                    icon,
                    entry.path,
                    style(&entry.hash_algo).dim(),
                    style(hash_preview).dim()
                );
            }
            println!();
            print_summary_table(&manifest, &destinations, copy_start.elapsed(), par2);
        }
    }

    Ok(0)
}

// ---------------------------------------------------------------------------
// cmd_verify
// ---------------------------------------------------------------------------

fn cmd_verify(cert_or_dir: PathBuf, dest: PathBuf, quiet: bool) -> anyhow::Result<i32> {
    let is_ferrcert = cert_or_dir
        .extension()
        .map(|e| e.eq_ignore_ascii_case("ferrcert"))
        .unwrap_or(false);
    let is_json = cert_or_dir
        .extension()
        .map(|e| e.eq_ignore_ascii_case("json"))
        .unwrap_or(false);
    let is_dir = cert_or_dir.is_dir();

    // Cas 1 : .ferrcert explicite
    if is_ferrcert {
        return verify_via_cert(&cert_or_dir, &dest, quiet);
    }

    // Cas 2 : dossier → cherche le cert automatiquement
    if is_dir {
        if let Some(cert_path) = ferr_cert::find_cert(&cert_or_dir) {
            return verify_via_cert(&cert_path, &dest, quiet);
        }
        // Pas de cert → comparaison par répertoire (legacy)
        if !quiet {
            eprintln!(
                "  {} No .ferrcert found in {} — falling back to directory comparison.\n  \
                 Run 'ferr cert create <dir>' to generate a certificate.",
                style("Note:").yellow(),
                cert_or_dir.display()
            );
        }
        let hasher: Box<dyn ferr_hash::Hasher> = Box::new(ferr_hash::XxHasher);
        let bar = make_spinner("Verifying…", quiet);
        let report = ferr_verify::verify_dirs(&cert_or_dir, &dest, hasher.as_ref())?;
        if let Some(b) = &bar {
            b.finish_and_clear();
        }
        return display_verify_report(&report, quiet);
    }

    // Cas 3 : manifest JSON legacy
    if is_json {
        if !quiet {
            eprintln!(
                "  {} JSON manifests are legacy — use 'ferr cert create' to generate a .ferrcert.",
                style("Note:").yellow()
            );
        }
        let hasher: Box<dyn ferr_hash::Hasher> = Box::new(ferr_hash::XxHasher);
        let bar = make_spinner("Verifying…", quiet);
        let m = ferr_report::load_manifest(&cert_or_dir)?;
        let report = ferr_verify::verify_manifest(&m, &dest, hasher.as_ref())?;
        if let Some(b) = &bar {
            b.finish_and_clear();
        }
        return display_verify_report(&report, quiet);
    }

    anyhow::bail!(
        "Cannot verify: '{}' is not a .ferrcert, a directory, or a .json manifest.",
        cert_or_dir.display()
    )
}

fn verify_via_cert(cert_path: &PathBuf, dest: &PathBuf, quiet: bool) -> anyhow::Result<i32> {
    let bar = make_spinner("Verifying certificate…", quiet);
    let result = ferr_cert::cert_verify(cert_path, dest, quiet)?;
    if let Some(b) = &bar {
        b.finish_and_clear();
    }

    if !quiet {
        let result_label = match result.result {
            ferr_cert::CertResult::Pass => style("PASS ✓").green().bold().to_string(),
            ferr_cert::CertResult::PassWithMinor => {
                style("PASS_WITH_MINOR").yellow().bold().to_string()
            }
            ferr_cert::CertResult::Fail => style("FAIL ✗").red().bold().to_string(),
        };
        println!("\n  Result: {result_label}");
        println!(
            "  {} files checked  {} dirs checked  {} issue(s)",
            result.checked_files,
            result.checked_dirs,
            result.issues.len()
        );

        let criticals: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.severity == ferr_cert::IssueSeverity::Critical)
            .collect();
        let minors: Vec<_> = result
            .issues
            .iter()
            .filter(|i| i.severity == ferr_cert::IssueSeverity::Minor)
            .collect();

        for issue in &criticals {
            println!(
                "  {} {} — {}",
                style("CRITICAL").red().bold(),
                issue.path,
                style(&issue.detail).dim()
            );
        }
        for issue in &minors {
            println!(
                "  {} {} — {}",
                style("MINOR").yellow(),
                issue.path,
                style(&issue.detail).dim()
            );
        }
    }

    let has_missing = result.issues.iter().any(|i| {
        matches!(
            i.kind,
            ferr_cert::IssueKind::MissingFile | ferr_cert::IssueKind::MissingDir
        )
    });
    let has_corrupted = result.issues.iter().any(|i| {
        matches!(
            i.kind,
            ferr_cert::IssueKind::CorruptedFile | ferr_cert::IssueKind::SizeMismatch
        )
    });

    Ok(match result.result {
        ferr_cert::CertResult::Pass => 0,
        ferr_cert::CertResult::PassWithMinor => 1,
        ferr_cert::CertResult::Fail => match (has_missing, has_corrupted) {
            (true, false) => 2,
            (false, true) => 3,
            _ => 4,
        },
    })
}

fn display_verify_report(report: &ferr_verify::VerifyReport, quiet: bool) -> anyhow::Result<i32> {
    if !quiet {
        println!(
            "\n  {} {} ok  {} missing  {} corrupted  ({:.1}s)",
            style("Result:").bold(),
            report.ok.len(),
            report.missing.len(),
            report.corrupted.len(),
            report.duration_secs
        );
        for p in &report.missing {
            println!("  {} {}", style("MISSING").yellow(), p.display());
        }
        for p in &report.corrupted {
            println!("  {} {}", style("CORRUPTED").red(), p.display());
        }
        if report.exit_code() == 0 {
            println!("  {}", style("All OK ✓").green().bold());
        }
    }
    Ok(report.exit_code())
}

// ---------------------------------------------------------------------------
// cmd_repair
// ---------------------------------------------------------------------------

fn cmd_repair(manifest: PathBuf, dest: PathBuf) -> anyhow::Result<i32> {
    let bar = make_spinner("PAR2 repair in progress…", false);
    let result = ferr_par2::repair(&manifest, &dest, |pct| {
        if let Some(b) = &bar {
            b.set_message(format!("PAR2: {pct}%…"));
        }
    });
    if let Some(b) = &bar {
        b.finish_and_clear();
    }

    match result {
        Ok(ferr_par2::Par2RepairStatus::Repaired) => {
            println!("  {} Repair successful ✓", style("PAR2:").green().bold());
            Ok(0)
        }
        Ok(ferr_par2::Par2RepairStatus::Failed) => {
            println!("  {} Unrecoverable", style("PAR2:").red().bold());
            Ok(3)
        }
        Err(e) => {
            println!("  {} {e}", style("PAR2 not available:").yellow().bold());
            Ok(3)
        }
    }
}

// ---------------------------------------------------------------------------
// cmd_scan
// ---------------------------------------------------------------------------

fn cmd_scan(
    dest: PathBuf,
    cert_or_manifest: Option<PathBuf>,
    since: Option<String>,
    quiet: bool,
) -> anyhow::Result<i32> {
    let since_dt = since
        .as_deref()
        .map(|s| chrono::DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&chrono::Utc)))
        .transpose()?;

    // Résoudre la source de référence
    let reference = if let Some(override_path) = cert_or_manifest {
        // --cert-or-manifest explicite
        if override_path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("ferrcert"))
            .unwrap_or(false)
            || override_path.is_dir()
        {
            ScanReference::Cert(resolve_cert_path(&override_path)?)
        } else {
            ScanReference::Manifest(override_path)
        }
    } else {
        // Auto-détection : cert d'abord, puis manifest legacy
        if let Some(cert_path) = ferr_cert::find_cert(&dest) {
            ScanReference::Cert(cert_path)
        } else if let Some(manifest_path) = ferr_core::find_manifest_path(&dest) {
            if !quiet {
                eprintln!(
                    "  {} No .ferrcert found — falling back to legacy ferr-manifest.json.",
                    style("Note:").yellow()
                );
            }
            ScanReference::Manifest(manifest_path)
        } else {
            anyhow::bail!(
                "No .ferrcert or ferr-manifest.json found in {}.\n\
                 Run 'ferr cert create <dir>' to generate a certificate.",
                dest.display()
            );
        }
    };

    match reference {
        ScanReference::Cert(cert_path) => scan_via_cert(&cert_path, &dest, since_dt, quiet),
        ScanReference::Manifest(manifest_path) => {
            scan_via_manifest(&manifest_path, &dest, since_dt, quiet)
        }
    }
}

enum ScanReference {
    Cert(PathBuf),
    Manifest(PathBuf),
}

fn scan_via_cert(
    cert_path: &PathBuf,
    dest: &PathBuf,
    since_dt: Option<chrono::DateTime<chrono::Utc>>,
    quiet: bool,
) -> anyhow::Result<i32> {
    let cert = ferr_cert::cert_load(cert_path)?;

    if !ferr_cert::cert_check_integrity(cert_path)? {
        eprintln!(
            "  {} Certificate integrity check FAILED — cert may be tampered.\n  Path: {}",
            style("Error:").red().bold(),
            cert_path.display()
        );
        return Ok(2);
    }

    let hasher: Box<dyn ferr_hash::Hasher> = match cert.hash_algo.as_str() {
        "sha256" => Box::new(ferr_hash::Sha256Hasher),
        _ => Box::new(ferr_hash::XxHasher),
    };

    let bar = make_spinner("Scanning for bit rot…", quiet);

    let report = ferr_verify::scan_bitrot_cert(dest, &cert, hasher.as_ref(), since_dt, |p| {
        if let Some(b) = &bar {
            b.set_message(format!(
                "[{}/{}] {}",
                p.scanned,
                p.total,
                p.current.display()
            ));
        }
    })?;

    if let Some(b) = &bar {
        b.finish_and_clear();
    }

    if !quiet {
        println!("\n  Scan completed on {}", style(&report.scan_date).dim());
        println!(
            "  {} scanned  {} skipped  {} corrupted",
            report.scanned,
            report.skipped,
            report.corrupted.len()
        );
        for entry in &report.corrupted {
            println!(
                "  {} {}",
                style("BIT ROT").red().bold(),
                entry.path.display()
            );
            println!("     expected: {}", style(&entry.expected_hash).dim());
            println!("     actual  : {}", style(&entry.actual_hash).red());
        }
        if report.corrupted.is_empty() {
            println!("  {}", style("No bit rot detected ✓").green().bold());
        }
    }

    Ok(if report.corrupted.is_empty() { 0 } else { 1 })
}

fn scan_via_manifest(
    manifest_path: &PathBuf,
    dest: &PathBuf,
    since_dt: Option<chrono::DateTime<chrono::Utc>>,
    quiet: bool,
) -> anyhow::Result<i32> {
    let manifest = ferr_report::load_manifest(manifest_path)?;
    let hasher: Box<dyn ferr_hash::Hasher> = Box::new(ferr_hash::XxHasher);

    let bar = make_spinner("Scanning for bit rot…", quiet);

    let report = ferr_verify::scan_bitrot(dest, &manifest, hasher.as_ref(), since_dt, |p| {
        if let Some(b) = &bar {
            b.set_message(format!(
                "[{}/{}] {}",
                p.scanned,
                p.total,
                p.current.display()
            ));
        }
    })?;

    if let Some(b) = &bar {
        b.finish_and_clear();
    }

    if !quiet {
        println!("\n  Scan completed on {}", style(&report.scan_date).dim());
        println!(
            "  {} scanned  {} skipped  {} corrupted",
            report.scanned,
            report.skipped,
            report.corrupted.len()
        );
        for entry in &report.corrupted {
            println!(
                "  {} {}",
                style("BIT ROT").red().bold(),
                entry.path.display()
            );
            println!("     expected: {}", style(&entry.expected_hash).dim());
            println!("     actual  : {}", style(&entry.actual_hash).red());
        }
        if report.corrupted.is_empty() {
            println!("  {}", style("No bit rot detected ✓").green().bold());
        }
    }

    Ok(if report.corrupted.is_empty() { 0 } else { 1 })
}

// ---------------------------------------------------------------------------
// cmd_watch
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn cmd_watch(
    mount_point: PathBuf,
    dest: Vec<PathBuf>,
    hash: HashChoice,
    par2: Option<u8>,
    camera: bool,
    profile: Option<String>,
    delay: u64,
    eject: bool,
    quiet: bool,
) -> anyhow::Result<i32> {
    let (hash_algo_from_args, _) = hash_choice_to_algo(&hash);

    let (destinations, watch_hash_algo, par2, camera, eject) = if let Some(name) = profile {
        match ferr_core::load_profile(&name) {
            Ok(p) => (
                p.destinations,
                ferr_core::HashAlgo::from_lossy(&p.hash_algo),
                p.par2_redundancy,
                p.camera_mode,
                p.auto_eject,
            ),
            Err(e) => {
                eprintln!("Profile not found: {e}");
                (dest, hash_algo_from_args, par2, camera, eject)
            }
        }
    } else {
        (dest, hash_algo_from_args, par2, camera, eject)
    };

    let watch_hooks = build_hooks(false, false, false);

    let config = ferr_core::WatchConfig {
        mount_point: mount_point.clone(),
        copy_job: ferr_core::CopyJobTemplate {
            destinations,
            hash_algo: watch_hash_algo,
            par2_redundancy: par2,
            camera_mode: camera,
            preserve_metadata: true,
            ..Default::default()
        },
        delay_secs: delay,
        auto_eject: eject,
        hooks: watch_hooks,
    };

    if !quiet {
        println!(
            "{} {} (delay {}s)",
            style("ferr watch").cyan().bold(),
            mount_point.display(),
            delay
        );
        println!("  Waiting for volumes… (Ctrl+C to quit)");
    }

    ferr_core::run_watch(config, move |event| {
        if quiet {
            return;
        }
        match event {
            ferr_core::WatchEvent::Waiting => {
                println!("  {} Waiting…", style("●").dim());
            }
            ferr_core::WatchEvent::VolumeDetected { name, size, .. } => {
                println!(
                    "  {} Volume detected: {} ({})",
                    style("▶").cyan().bold(),
                    style(&name).bold(),
                    human_size(size)
                );
            }
            ferr_core::WatchEvent::CopyStarting { volume } => {
                println!("  {} Starting copy of {volume}…", style("→").green());
            }
            ferr_core::WatchEvent::CopyDone { volume, manifest } => {
                println!(
                    "  {} {volume} copied — {} files · {}",
                    style("✓").green().bold(),
                    manifest.total_files,
                    human_size(manifest.total_size_bytes)
                );
            }
            ferr_core::WatchEvent::Ejected { volume } => {
                println!(
                    "  {} {volume} ejected — safe to reformat",
                    style("⏏").cyan().bold()
                );
            }
            ferr_core::WatchEvent::Error { volume, error } => {
                println!("  {} {volume}: {error}", style("✗").red().bold());
            }
            ferr_core::WatchEvent::CopyProgress(_) => {}
        }
    })?;

    Ok(0)
}

// ---------------------------------------------------------------------------
// cmd_export
// ---------------------------------------------------------------------------

fn cmd_export(
    manifest_path: PathBuf,
    format: ExportFormat,
    output: PathBuf,
) -> anyhow::Result<i32> {
    let manifest = ferr_report::load_manifest(&manifest_path)?;
    match format {
        ExportFormat::Ale => ferr_report::export_ale(&manifest, &output)?,
        ExportFormat::Csv => ferr_report::export_csv(&manifest, &output)?,
    }
    println!("  {} Exported to {}", style("✓").green(), output.display());
    Ok(0)
}

// ---------------------------------------------------------------------------
// cmd_report
// ---------------------------------------------------------------------------

fn cmd_report(manifest_path: PathBuf, output: Option<PathBuf>) -> anyhow::Result<i32> {
    let manifest = ferr_report::load_manifest(&manifest_path)?;
    let output = output.unwrap_or_else(|| manifest_path.with_extension("pdf"));
    ferr_pdf::generate_report(&manifest, &output)?;
    println!(
        "  {} PDF generated: {}",
        style("✓").green(),
        output.display()
    );
    Ok(0)
}

// ---------------------------------------------------------------------------
// cmd_profile
// ---------------------------------------------------------------------------

fn cmd_profile(action: ProfileAction) -> anyhow::Result<i32> {
    match action {
        ProfileAction::Save {
            name,
            dest,
            hash,
            par2,
            camera,
            eject,
        } => {
            let (_, hash_str) = hash_choice_to_algo(&hash);
            let profile = ferr_core::CopyProfile {
                name: name.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                destinations: dest,
                hash_algo: hash_str.to_string(),
                par2_redundancy: par2,
                camera_mode: camera,
                auto_eject: eject,
            };
            ferr_core::save_profile(&profile)?;
            println!("  {} Profile '{}' saved", style("✓").green(), name);
        }
        ProfileAction::List => {
            let profiles = ferr_core::list_profiles()?;
            if profiles.is_empty() {
                println!("  No profiles configured.");
            } else {
                for p in &profiles {
                    println!(
                        "  {} — hash:{} par2:{} camera:{} eject:{}",
                        style(&p.name).bold(),
                        p.hash_algo,
                        p.par2_redundancy
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "none".into()),
                        p.camera_mode,
                        p.auto_eject,
                    );
                }
            }
        }
        ProfileAction::Show { name } => {
            let p = ferr_core::load_profile(&name)?;
            println!("{}", serde_json::to_string_pretty(&p)?);
        }
        ProfileAction::Delete { name } => {
            ferr_core::delete_profile(&name)?;
            println!("  {} Profile '{}' deleted", style("✓").green(), name);
        }
    }
    Ok(0)
}

// ---------------------------------------------------------------------------
// cmd_history
// ---------------------------------------------------------------------------

fn cmd_history(action: HistoryAction) -> anyhow::Result<i32> {
    match action {
        HistoryAction::List { limit, since } => {
            let sessions = ferr_session::list_sessions(ferr_session::SessionFilter {
                limit: Some(limit),
                since,
                ..Default::default()
            })?;
            if sessions.is_empty() {
                println!("  No sessions recorded.");
            } else {
                println!(
                    "  {:>5}  {:26}  {:>8}  {:>10}  Source",
                    "ID", "Date", "Files", "Size"
                );
                let sep = "─".repeat(HISTORY_TABLE_WIDTH);
                println!("  {sep}");
                for s in &sessions {
                    println!(
                        "  {:>5}  {:26}  {:>8}  {:>10}  {}",
                        s.id,
                        &s.date[..s.date.len().min(26)],
                        s.total_files,
                        human_size(s.total_bytes),
                        s.source,
                    );
                }
            }
        }
        HistoryAction::Show { id } => match ferr_session::get_session(id)? {
            Some(s) => println!("{}", serde_json::to_string_pretty(&s)?),
            None => println!("  Session #{id} not found."),
        },
        HistoryAction::Find { hash_or_name } => {
            let records = ferr_session::find_file_by_hash(&hash_or_name)?;
            if records.is_empty() {
                println!("  No file found for '{hash_or_name}'.");
            } else {
                for r in &records {
                    println!(
                        "  session:{:5}  {}  {}",
                        r.session_id,
                        r.path,
                        style(&r.hash).dim()
                    );
                }
            }
        }
    }
    Ok(0)
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn hash_choice_to_algo(h: &HashChoice) -> (ferr_core::HashAlgo, &'static str) {
    match h {
        HashChoice::Xxhash => (ferr_core::HashAlgo::XxHash64, "xxhash"),
        HashChoice::Sha256 => (ferr_core::HashAlgo::Sha256, "sha256"),
    }
}

fn make_spinner(msg: &str, quiet: bool) -> Option<ProgressBar> {
    if quiet {
        return None;
    }
    let bar = ProgressBar::new_spinner();
    bar.set_style(ProgressStyle::with_template("{spinner:.cyan} {msg}").unwrap());
    bar.set_message(msg.to_string());
    bar.enable_steady_tick(std::time::Duration::from_millis(80));
    Some(bar)
}

fn print_summary_table(
    manifest: &ferr_report::Manifest,
    destinations: &[PathBuf],
    elapsed: std::time::Duration,
    par2_pct: Option<u8>,
) {
    let sep = "─".repeat(TABLE_WIDTH);
    println!("{sep}");
    println!(
        "  {:<30}  {:>10}  {:>10}  {:>10}  {:>7}  Status",
        "Destination", "Files", "Size", "Duration", "Errors"
    );
    println!("{sep}");

    let errors = manifest
        .files
        .iter()
        .filter(|f| f.status == ferr_report::FileStatus::Corrupted)
        .count();
    let size_str = human_size(manifest.total_size_bytes);
    let dur_str = format!("{:.1}s", elapsed.as_secs_f64());
    let status_s = match manifest.status {
        ferr_report::JobStatus::Ok => style("OK").green().bold().to_string(),
        ferr_report::JobStatus::Partial => style("PARTIAL").yellow().bold().to_string(),
        ferr_report::JobStatus::Failed => style("FAILED").red().bold().to_string(),
    };

    for dest in destinations {
        let s = dest.to_string_lossy();
        let t = if s.len() > DEST_PATH_DISPLAY_MAX {
            format!("…{}", &s[s.len() - (DEST_PATH_DISPLAY_MAX - 1)..])
        } else {
            s.into_owned()
        };
        println!(
            "  {:<30}  {:>10}  {:>10}  {:>10}  {:>7}  {}",
            t, manifest.total_files, size_str, dur_str, errors, status_s
        );
    }

    if let Some(pct) = par2_pct {
        println!("{sep}");
        println!(
            "  {:<30}  {:>10}  {:>10}  {:>10}  {:>7}  {}",
            "PAR2",
            format!("{pct}%"),
            "(stub)",
            "-",
            "-",
            style("STUB").dim()
        );
    }

    println!("{sep}");
    println!(
        "  Total: {} files · {} · {:.1}s",
        manifest.total_files,
        size_str,
        elapsed.as_secs_f64()
    );
}

fn human_size(bytes: u64) -> String {
    ferr_report::human_size(bytes)
}

// ---------------------------------------------------------------------------
// cmd_cert
// ---------------------------------------------------------------------------

/// Résout un chemin en .ferrcert : accepte un fichier .ferrcert ou un dossier
/// (cherche automatiquement le .ferrcert à la racine).
fn resolve_cert_path(path: &PathBuf) -> anyhow::Result<PathBuf> {
    if path.is_file() {
        if path
            .extension()
            .map(|e: &std::ffi::OsStr| e.eq_ignore_ascii_case("ferrcert"))
            .unwrap_or(false)
        {
            Ok(path.to_path_buf())
        } else {
            anyhow::bail!(
                "Not a .ferrcert file: {}\nUse 'ferr cert create <dir>' to generate one.",
                path.display()
            )
        }
    } else if path.is_dir() {
        ferr_cert::find_cert(path).ok_or_else(|| {
            anyhow::anyhow!(
                "No .ferrcert found in {}.\nRun 'ferr cert create {}' to generate one,\nor 'ferr copy <src> <dest>' which creates it automatically.",
                path.display(),
                path.display()
            )
        })
    } else {
        anyhow::bail!("Path not found: {}", path.display())
    }
}

fn cmd_cert(action: CertAction) -> anyhow::Result<i32> {
    match action {
        // ── cert create ───────────────────────────────────────────────────
        CertAction::Create {
            src,
            output,
            hash,
            quiet,
        } => {
            if src.is_file() {
                eprintln!(
                    "  {} ferr cert create only accepts directories.\n  To certify a single file, put it in a folder first.",
                    style("Error:").red().bold()
                );
                return Ok(2);
            }

            let (hash_algo, _) = hash_choice_to_algo(&hash);
            let bar = make_spinner("Generating certificate…", quiet);

            let cert_path = ferr_cert::cert_create(&src, output.as_deref(), hash_algo)?;

            if let Some(b) = &bar {
                b.finish_and_clear();
            }

            if !quiet {
                let cert = ferr_cert::cert_load(&cert_path)?;
                println!(
                    "  {} Certificate: {}",
                    style("✓").green().bold(),
                    cert_path.display()
                );
                println!(
                    "  {} {} files  {} dirs ({} empty)  {}",
                    style("→").dim(),
                    cert.summary.total_files,
                    cert.summary.total_dirs,
                    cert.summary.empty_dirs,
                    human_size(cert.summary.total_bytes),
                );
                println!(
                    "  {} cert_hash: {}…",
                    style("→").dim(),
                    &cert.cert_hash[..cert.cert_hash.len().min(32)]
                );
            }
            Ok(0)
        }

        // ── cert verify ───────────────────────────────────────────────────
        CertAction::Verify { cert, dest, quiet } => {
            let cert_path = resolve_cert_path(&cert)?;
            verify_via_cert(&cert_path, &dest, quiet)
        }

        // ── cert show ─────────────────────────────────────────────────────
        CertAction::Show { cert_or_dir } => {
            let cert_path = resolve_cert_path(&cert_or_dir)?;
            let cert = ferr_cert::cert_load(&cert_path)?;
            let is_valid = ferr_cert::cert_check_integrity(&cert_path)?;

            println!();
            println!("  Certificate : {}", cert_path.display());
            println!("  ID          : {}", cert.id);
            println!("  Source      : {}", cert.source.path);
            println!("  Label       : {}", cert.source.label);
            println!("  Certified   : {}", cert.source.certified_at);
            println!("  By          : {}", cert.source.certified_by);
            println!("  Hash algo   : {}", cert.hash_algo);
            println!("  Files       : {}", cert.summary.total_files);
            println!(
                "  Dirs        : {} ({} empty)",
                cert.summary.total_dirs, cert.summary.empty_dirs
            );
            println!("  Size        : {}", human_size(cert.summary.total_bytes));
            println!();

            let sep = "─".repeat(70);
            println!("  {sep}");
            println!("  Events ({})", cert.events.len());
            println!("  {sep}");

            for (i, ev) in cert.events.iter().enumerate() {
                let result_s = match ev.result.as_str() {
                    "PASS" => style(ev.result.as_str()).green().to_string(),
                    "PASS_WITH_MINOR" => style(ev.result.as_str()).yellow().to_string(),
                    _ => style(ev.result.as_str()).red().to_string(),
                };
                let ts = if ev.at.len() >= 19 {
                    &ev.at[..19]
                } else {
                    &ev.at
                };
                println!(
                    "  {:>2}. [{}] {} — {}",
                    i + 1,
                    ts,
                    style(&ev.kind).bold(),
                    result_s
                );
                println!("      {}", style(&ev.detail).dim());
                if !ev.issues.is_empty() {
                    for issue in &ev.issues {
                        let sev = match issue.severity {
                            ferr_cert::IssueSeverity::Critical => {
                                style("CRITICAL").red().to_string()
                            }
                            ferr_cert::IssueSeverity::Minor => style("MINOR").yellow().to_string(),
                        };
                        println!("      {} {}", sev, issue.path);
                    }
                }
            }

            println!("  {sep}");
            if is_valid {
                println!("  {} Certificate integrity: OK", style("✓").green().bold());
                Ok(0)
            } else {
                println!(
                    "  {} Certificate integrity: TAMPERED",
                    style("✗").red().bold()
                );
                Ok(2)
            }
        }

        // ── cert check ────────────────────────────────────────────────────
        CertAction::Check { cert_or_dir } => {
            let cert_path = resolve_cert_path(&cert_or_dir)?;
            let is_valid = ferr_cert::cert_check_integrity(&cert_path)?;

            if is_valid {
                println!(
                    "  {} Certificate integrity: OK  ({})",
                    style("✓").green().bold(),
                    cert_path.display()
                );
                Ok(0)
            } else {
                println!(
                    "  {} Certificate integrity: TAMPERED  ({})",
                    style("✗").red().bold(),
                    cert_path.display()
                );
                Ok(2)
            }
        }
    }
}
