use std::path::PathBuf;
use std::process;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};
use console::style;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "ferr",
    about = "Copie sécurisée de fichiers vidéo avec vérification hash et redondance PAR2",
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
    /// Copie des fichiers avec vérification hash
    Copy {
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
        no_pdf: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        quiet: bool,
    },
    /// Vérifie l'intégrité des fichiers
    Verify {
        src_or_manifest: PathBuf,
        dest: PathBuf,
        #[arg(long)]
        quiet: bool,
    },
    /// Répare via PAR2
    Repair { manifest: PathBuf, dest: PathBuf },
    /// Détecte le bit rot sur une destination
    Scan {
        dest: PathBuf,
        #[arg(long)]
        manifest: Option<PathBuf>,
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        quiet: bool,
    },
    /// Surveille un point de montage et copie automatiquement
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
    /// Exporte un manifest vers ALE ou CSV
    Export {
        manifest: PathBuf,
        #[arg(long, default_value = "csv")]
        format: ExportFormat,
        #[arg(long)]
        output: PathBuf,
    },
    /// Génère un rapport PDF depuis un manifest
    Report {
        manifest: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Gère les profils de copie
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Gère l'historique des sessions
    History {
        #[command(subcommand)]
        action: HistoryAction,
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

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    // Respect NO_COLOR
    if std::env::var("NO_COLOR").is_ok() {
        console::set_colors_enabled(false);
    }

    let result = run(cli);
    match result {
        Ok(code) => process::exit(code),
        Err(e) => {
            eprintln!("{} {e}", style("Erreur :").red().bold());
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
            no_pdf,
            dry_run,
            quiet,
        } => cmd_copy(
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
            no_pdf,
            dry_run,
            quiet,
        ),
        Commands::Verify {
            src_or_manifest,
            dest,
            quiet,
        } => cmd_verify(src_or_manifest, dest, quiet),
        Commands::Repair { manifest, dest } => cmd_repair(manifest, dest),
        Commands::Scan {
            dest,
            manifest,
            since,
            quiet,
        } => cmd_scan(dest, manifest, since, quiet),
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
    }
}

// ---------------------------------------------------------------------------
// cmd_copy
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn cmd_copy(
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
    no_pdf: bool,
    dry_run_flag: bool,
    quiet: bool,
) -> anyhow::Result<i32> {
    let mut destinations = vec![dest];
    if let Some(d) = dest2 {
        destinations.push(d);
    }
    if let Some(d) = dest3 {
        destinations.push(d);
    }
    let (hash_algo, _hash_algo_str) = hash_choice_to_algo(&hash);

    // Charger le profil si fourni
    let (destinations, hash_algo, par2, camera, eject, rename) = if let Some(profile_name) = profile
    {
        match ferr_core::load_profile(&profile_name) {
            Ok(p) => {
                let (ha, _) = match p.hash_algo.as_str() {
                    "sha256" => (ferr_core::HashAlgo::Sha256, "sha256"),
                    _ => (ferr_core::HashAlgo::XxHash64, "xxhash"),
                };
                (
                    p.destinations,
                    ha,
                    p.par2_redundancy,
                    p.camera_mode,
                    p.auto_eject,
                    rename,
                )
            }
            Err(e) => {
                eprintln!("Profil non trouvé : {e}");
                (destinations, hash_algo, par2, camera, eject, rename)
            }
        }
    } else {
        (destinations, hash_algo, par2, camera, eject, rename)
    };

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
        generate_pdf: !no_pdf,
        send_notify: !no_notify,
        record_session: true,
    };

    // Mode dry-run
    if dry_run_flag {
        let report = ferr_core::dry_run(&job)?;
        if !quiet {
            println!(
                "{}",
                style("Mode dry-run — aucun fichier écrit").yellow().bold()
            );
            println!("  Fichiers    : {}", report.total_files);
            println!("  Taille      : {}", human_size(report.total_size_bytes));
            println!("  PAR2 estimé : {}", human_size(report.par2_size_bytes));
            println!("  Durée est.  : {}s (à 300 Mo/s)", report.estimated_secs);
            for check in &report.space_checks {
                if check.ok {
                    println!(
                        "  {} {} — disponible {}",
                        style("✓").green(),
                        check.destination.display(),
                        human_size(check.available_bytes)
                    );
                } else {
                    println!(
                        "  {} {} — manque {}",
                        style("✗").red(),
                        check.destination.display(),
                        human_size(check.delta_bytes.unsigned_abs())
                    );
                }
            }
            if let Some(clips) = &report.clips {
                println!("  Clips détectés : {}", clips.len());
            }
        }
        return Ok(0);
    }

    // Mode normal
    let mp = MultiProgress::new();
    let global_bar = mp.add(ProgressBar::new(0));
    global_bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.cyan} [{elapsed_precise}] {pos}/{len} fichiers  {bytes} copiés  {binary_bytes_per_sec}  ETA {eta}",
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

    let file_count = Arc::new(Mutex::new(0usize));
    let copy_start = Instant::now();

    let on_progress = {
        let global_bar = global_bar.clone();
        let file_bar = file_bar.clone();
        let file_count = Arc::clone(&file_count);
        let quiet = quiet;

        move |p: ferr_core::CopyProgress| {
            if quiet {
                return;
            }
            let phase_label = match p.phase {
                ferr_core::CopyPhase::Copying => "[Copie]",
                ferr_core::CopyPhase::Verifying => "[Vérif]",
                ferr_core::CopyPhase::GeneratingPar2 => "[PAR2]",
                ferr_core::CopyPhase::Done => "[Terminé]",
            };
            match p.phase {
                ferr_core::CopyPhase::Done => {
                    file_bar.finish_and_clear();
                    global_bar.finish_with_message("Terminé ✓");
                }
                ferr_core::CopyPhase::GeneratingPar2 => {
                    file_bar.set_message("[PAR2] génération…");
                }
                _ => {
                    let name = p
                        .current_file
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let trunc = if name.len() > 40 {
                        format!("…{}", &name[name.len() - 39..])
                    } else {
                        name
                    };

                    file_bar.set_length(p.file_bytes_total);
                    file_bar.set_position(p.file_bytes_done);
                    file_bar.set_message(format!("{phase_label} {trunc}"));
                    global_bar.set_length(p.total_files as u64);
                    let done = *file_count.lock().unwrap();
                    if p.total_files_done > done {
                        *file_count.lock().unwrap() = p.total_files_done;
                        global_bar.set_position(p.total_files_done as u64);
                    }
                }
            }
        }
    };

    let manifest = ferr_core::run_copy(job, on_progress)?;
    if !quiet {
        mp.clear()?;
    }

    // Résumé hash par fichier
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

    Ok(0)
}

// ---------------------------------------------------------------------------
// cmd_verify
// ---------------------------------------------------------------------------

fn cmd_verify(src_or_manifest: PathBuf, dest: PathBuf, quiet: bool) -> anyhow::Result<i32> {
    let hasher: Box<dyn ferr_hash::Hasher> = Box::new(ferr_hash::XxHasher);
    let bar = make_spinner("Vérification en cours…", quiet);

    let report = if src_or_manifest
        .extension()
        .map(|e| e.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
    {
        let m = ferr_report::load_manifest(&src_or_manifest)?;
        ferr_verify::verify_manifest(&m, &dest, hasher.as_ref())?
    } else {
        ferr_verify::verify_dirs(&src_or_manifest, &dest, hasher.as_ref())?
    };

    if let Some(b) = &bar {
        b.finish_and_clear();
    }

    if !quiet {
        println!(
            "\n  {} {} ok  {} manquants  {} corrompus  ({:.1}s)",
            style("Résultat :").bold(),
            report.ok.len(),
            report.missing.len(),
            report.corrupted.len(),
            report.duration_secs
        );
        for p in &report.missing {
            println!("  {} {}", style("MANQUANT").yellow(), p.display());
        }
        for p in &report.corrupted {
            println!("  {} {}", style("CORROMPU").red(), p.display());
        }
        if report.exit_code() == 0 {
            println!("  {}", style("Tout OK ✓").green().bold());
        }
    }

    Ok(report.exit_code())
}

// ---------------------------------------------------------------------------
// cmd_repair
// ---------------------------------------------------------------------------

fn cmd_repair(manifest: PathBuf, dest: PathBuf) -> anyhow::Result<i32> {
    let bar = make_spinner("Réparation PAR2 en cours…", false);
    let result = ferr_par2::repair(&manifest, &dest, |pct| {
        if let Some(b) = &bar {
            b.set_message(format!("PAR2 : {pct}%…"));
        }
    });
    if let Some(b) = &bar {
        b.finish_and_clear();
    }

    match result {
        Ok(ferr_par2::Par2RepairStatus::Repaired) => {
            println!("  {} Réparation réussie ✓", style("PAR2 :").green().bold());
            Ok(0)
        }
        Ok(ferr_par2::Par2RepairStatus::Failed) => {
            println!("  {} Irrécupérable", style("PAR2 :").red().bold());
            Ok(3)
        }
        Err(e) => {
            println!("  {} {e}", style("PAR2 non disponible :").yellow().bold());
            Ok(3)
        }
    }
}

// ---------------------------------------------------------------------------
// cmd_scan
// ---------------------------------------------------------------------------

fn cmd_scan(
    dest: PathBuf,
    manifest_path: Option<PathBuf>,
    since: Option<String>,
    quiet: bool,
) -> anyhow::Result<i32> {
    let manifest_path = manifest_path.unwrap_or_else(|| dest.join("ferr-manifest.json"));

    let manifest = ferr_report::load_manifest(&manifest_path)?;
    let hasher: Box<dyn ferr_hash::Hasher> = Box::new(ferr_hash::XxHasher);

    let since_dt = since
        .as_deref()
        .map(|s| chrono::DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&chrono::Utc)))
        .transpose()?;

    let bar = make_spinner("Scan bit rot en cours…", quiet);

    let report = ferr_verify::scan_bitrot(&dest, &manifest, hasher.as_ref(), since_dt, |p| {
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
        println!("\n  Scan terminé le {}", style(&report.scan_date).dim());
        println!(
            "  {} scannés  {} ignorés  {} corrompus",
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
            println!("     attendu : {}", style(&entry.expected_hash).dim());
            println!("     actuel  : {}", style(&entry.actual_hash).red());
        }
        if report.corrupted.is_empty() {
            println!("  {}", style("Aucun bit rot détecté ✓").green().bold());
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
    let (_, hash_algo_str) = hash_choice_to_algo(&hash);

    let (destinations, hash_str, par2, camera, eject) = if let Some(name) = profile {
        match ferr_core::load_profile(&name) {
            Ok(p) => (
                p.destinations,
                p.hash_algo,
                p.par2_redundancy,
                p.camera_mode,
                p.auto_eject,
            ),
            Err(e) => {
                eprintln!("Profil non trouvé : {e}");
                (dest, hash_algo_str.to_string(), par2, camera, eject)
            }
        }
    } else {
        (dest, hash_algo_str.to_string(), par2, camera, eject)
    };

    let config = ferr_core::WatchConfig {
        mount_point: mount_point.clone(),
        copy_job: ferr_core::CopyJobTemplate {
            destinations,
            hash_algo_str: hash_str,
            par2_redundancy: par2,
            camera_mode: camera,
            preserve_metadata: true,
            auto_eject: eject,
            ..Default::default()
        },
        delay_secs: delay,
        auto_eject: eject,
    };

    if !quiet {
        println!(
            "{} {} (délai {}s)",
            style("ferr watch").cyan().bold(),
            mount_point.display(),
            delay
        );
        println!("  En attente de volumes… (Ctrl+C pour quitter)");
    }

    ferr_core::run_watch(config, move |event| {
        if quiet {
            return;
        }
        match event {
            ferr_core::WatchEvent::Waiting => {
                println!("  {} En attente…", style("●").dim());
            }
            ferr_core::WatchEvent::VolumeDetected { name, size, .. } => {
                println!(
                    "  {} Volume détecté : {} ({})",
                    style("▶").cyan().bold(),
                    style(&name).bold(),
                    human_size(size)
                );
            }
            ferr_core::WatchEvent::CopyStarting { volume } => {
                println!("  {} Démarrage copie de {volume}…", style("→").green());
            }
            ferr_core::WatchEvent::CopyDone { volume, manifest } => {
                println!(
                    "  {} {volume} copié — {} fichiers · {}",
                    style("✓").green().bold(),
                    manifest.total_files,
                    human_size(manifest.total_size_bytes)
                );
            }
            ferr_core::WatchEvent::Ejected { volume } => {
                println!(
                    "  {} {volume} éjectée — vous pouvez reformater cette carte",
                    style("⏏").cyan().bold()
                );
            }
            ferr_core::WatchEvent::Error { volume, error } => {
                println!("  {} {volume} : {error}", style("✗").red().bold());
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
    println!("  {} Export vers {}", style("✓").green(), output.display());
    Ok(0)
}

// ---------------------------------------------------------------------------
// cmd_report
// ---------------------------------------------------------------------------

fn cmd_report(manifest_path: PathBuf, output: Option<PathBuf>) -> anyhow::Result<i32> {
    let manifest = ferr_report::load_manifest(&manifest_path)?;
    let output = output.unwrap_or_else(|| manifest_path.with_extension("pdf"));
    ferr_pdf::generate_report(&manifest, &output)?;
    println!("  {} PDF généré : {}", style("✓").green(), output.display());
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
            println!("  {} Profil '{}' sauvegardé", style("✓").green(), name);
        }
        ProfileAction::List => {
            let profiles = ferr_core::list_profiles()?;
            if profiles.is_empty() {
                println!("  Aucun profil configuré.");
            } else {
                for p in &profiles {
                    println!(
                        "  {} — hash:{} par2:{} caméra:{} éjection:{}",
                        style(&p.name).bold(),
                        p.hash_algo,
                        p.par2_redundancy
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "non".into()),
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
            println!("  {} Profil '{}' supprimé", style("✓").green(), name);
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
                println!("  Aucune session enregistrée.");
            } else {
                println!(
                    "  {:>5}  {:26}  {:>8}  {:>10}  {}",
                    "ID", "Date", "Fichiers", "Taille", "Source"
                );
                let sep = "─".repeat(70);
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
            None => println!("  Session #{id} non trouvée."),
        },
        HistoryAction::Find { hash_or_name } => {
            let records = ferr_session::find_file_by_hash(&hash_or_name)?;
            if records.is_empty() {
                println!("  Aucun fichier trouvé pour '{hash_or_name}'.");
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
// Utilitaires
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
    let sep = "─".repeat(80);
    println!("{sep}");
    println!(
        "  {:<30}  {:>10}  {:>10}  {:>10}  {:>7}  {}",
        "Destination", "Fichiers", "Taille", "Durée", "Erreurs", "Statut"
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
        ferr_report::JobStatus::Partial => style("PARTIEL").yellow().bold().to_string(),
        ferr_report::JobStatus::Failed => style("ÉCHEC").red().bold().to_string(),
    };

    for dest in destinations {
        let s = dest.to_string_lossy();
        let t = if s.len() > 30 {
            format!("…{}", &s[s.len() - 29..])
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
        "  Total : {} fichiers · {} · {:.1}s",
        manifest.total_files,
        size_str,
        elapsed.as_secs_f64()
    );
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
