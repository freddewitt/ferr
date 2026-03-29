//! Tests d'intégration ferr-cli — exercent le binaire `ferr` via subprocess.
//!
//! Chaque test invoque le binaire compilé et vérifie le code de retour,
//! la présence des fichiers produits et, quand pertinent, le contenu stdout.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// Utilitaires
// ---------------------------------------------------------------------------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn tmp(label: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ferr_cli_{label}_{id}"))
}

fn make_source(dir: &Path, n: usize, size: usize) {
    std::fs::create_dir_all(dir).unwrap();
    for i in 0..n {
        let data: Vec<u8> = (0..size).map(|j| ((j + i * 7) % 256) as u8).collect();
        std::fs::write(dir.join(format!("clip{i:03}.dat")), &data).unwrap();
    }
}

/// Retourne un Command pointant vers le binaire ferr compilé par cargo.
fn ferr() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ferr"))
}

/// Lance la commande et retourne Output — panique si le spawn échoue.
fn run(cmd: &mut Command) -> Output {
    cmd.output().expect("Impossible de lancer le binaire ferr")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}
fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

// ---------------------------------------------------------------------------
// 1. `ferr --help` — aide globale
// ---------------------------------------------------------------------------

#[test]
fn help_lists_all_commands() {
    let out = run(ferr().arg("--help"));
    assert!(out.status.success(), "ferr --help a échoué");
    let s = stdout(&out);
    for cmd in &[
        "copy", "verify", "repair", "scan", "watch", "export", "report", "profile", "history",
    ] {
        assert!(s.contains(cmd), "commande '{cmd}' absente du --help");
    }
}

// ---------------------------------------------------------------------------
// 2. `ferr copy` — copie basique
// ---------------------------------------------------------------------------

#[test]
fn copy_basic() {
    let src = tmp("copy_src");
    let dst = tmp("copy_dst");
    make_source(&src, 4, 2048);

    let out = run(ferr()
        .arg("copy")
        .arg(&src)
        .arg(&dst)
        .arg("--no-notify")
        .arg("--no-pdf")
        .arg("--quiet"));

    assert!(
        out.status.success(),
        "ferr copy a échoué (code {:?})\nstderr: {}",
        out.status.code(),
        stderr(&out)
    );

    // Les 4 fichiers existent à la destination
    for i in 0..4 {
        assert!(
            dst.join(format!("clip{i:03}.dat")).exists(),
            "clip{i:03}.dat absent de la destination"
        );
    }

    // Le manifest JSON est généré
    assert!(
        dst.join("ferr-manifest.json").exists(),
        "ferr-manifest.json absent"
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 3. `ferr copy --dry-run` — aucun fichier créé
// ---------------------------------------------------------------------------

#[test]
fn copy_dry_run_writes_nothing() {
    let src = tmp("dryrun_src");
    let dst = tmp("dryrun_dst");
    make_source(&src, 3, 1024);

    let out = run(ferr().arg("copy").arg(&src).arg(&dst).arg("--dry-run"));

    assert!(
        out.status.success(),
        "ferr copy --dry-run a échoué: {}",
        stderr(&out)
    );

    // La destination ne doit contenir aucun fichier source
    for i in 0..3 {
        assert!(
            !dst.join(format!("clip{i:03}.dat")).exists(),
            "clip{i:03}.dat ne devrait pas exister en dry-run"
        );
    }

    let s = stdout(&out);
    assert!(
        s.contains("dry-run") || s.contains("Fichiers"),
        "Sortie dry-run inattendue : {s}"
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 4. `ferr verify` — vérification depuis manifest
// ---------------------------------------------------------------------------

#[test]
fn verify_ok_from_manifest() {
    let src = tmp("verify_src");
    let dst = tmp("verify_dst");
    make_source(&src, 3, 1024);

    // Copie d'abord
    run(ferr()
        .arg("copy")
        .arg(&src)
        .arg(&dst)
        .arg("--no-notify")
        .arg("--no-pdf")
        .arg("--quiet"));

    let manifest = dst.join("ferr-manifest.json");
    assert!(manifest.exists());

    let out = run(ferr().arg("verify").arg(&manifest).arg(&dst));
    assert_eq!(
        out.status.code(),
        Some(0),
        "ferr verify devrait retourner 0 : {}",
        stdout(&out)
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

#[test]
fn verify_detects_corruption() {
    let src = tmp("vcorrupt_src");
    let dst = tmp("vcorrupt_dst");
    make_source(&src, 3, 4096);

    run(ferr()
        .arg("copy")
        .arg(&src)
        .arg(&dst)
        .arg("--no-notify")
        .arg("--no-pdf")
        .arg("--quiet"));

    // Corrompre un fichier
    let file = dst.join("clip000.dat");
    let mut data = std::fs::read(&file).unwrap();
    data[0] ^= 0xFF;
    std::fs::write(&file, &data).unwrap();

    let manifest = dst.join("ferr-manifest.json");
    let out = run(ferr().arg("verify").arg(&manifest).arg(&dst));
    assert_ne!(
        out.status.code(),
        Some(0),
        "ferr verify devrait détecter la corruption"
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 5. `ferr scan` — bit rot
// ---------------------------------------------------------------------------

#[test]
fn scan_clean_returns_zero() {
    let src = tmp("scan_src");
    let dst = tmp("scan_dst");
    make_source(&src, 3, 2048);

    run(ferr()
        .arg("copy")
        .arg(&src)
        .arg(&dst)
        .arg("--no-notify")
        .arg("--no-pdf")
        .arg("--quiet"));

    let manifest = dst.join("ferr-manifest.json");
    let out = run(ferr()
        .arg("scan")
        .arg(&dst)
        .arg("--manifest")
        .arg(&manifest)
        .arg("--quiet"));

    assert_eq!(
        out.status.code(),
        Some(0),
        "ferr scan devrait retourner 0 sur fichiers sains : {}",
        stdout(&out)
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

#[test]
fn scan_detects_bitrot() {
    let src = tmp("bitrot_src");
    let dst = tmp("bitrot_dst");
    make_source(&src, 3, 4096);

    run(ferr()
        .arg("copy")
        .arg(&src)
        .arg(&dst)
        .arg("--no-notify")
        .arg("--no-pdf")
        .arg("--quiet"));

    // Bit rot in-place
    let file = dst.join("clip001.dat");
    let mut data = std::fs::read(&file).unwrap();
    data[200] ^= 0xAB;
    std::fs::write(&file, &data).unwrap();

    let manifest = dst.join("ferr-manifest.json");
    let out = run(ferr()
        .arg("scan")
        .arg(&dst)
        .arg("--manifest")
        .arg(&manifest)
        .arg("--quiet"));

    assert_eq!(
        out.status.code(),
        Some(1),
        "ferr scan devrait retourner 1 sur bit rot : {}",
        stdout(&out)
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 6. `ferr export` — ALE et CSV
// ---------------------------------------------------------------------------

#[test]
fn export_ale_creates_file() {
    let src = tmp("export_ale_src");
    let dst = tmp("export_ale_dst");
    make_source(&src, 2, 512);

    run(ferr()
        .arg("copy")
        .arg(&src)
        .arg(&dst)
        .arg("--no-notify")
        .arg("--no-pdf")
        .arg("--quiet"));

    let manifest = dst.join("ferr-manifest.json");
    let ale = dst.join("export.ale");

    let out = run(ferr()
        .arg("export")
        .arg(&manifest)
        .arg("--format")
        .arg("ale")
        .arg("--output")
        .arg(&ale));

    assert!(
        out.status.success(),
        "ferr export ALE a échoué: {}",
        stderr(&out)
    );
    assert!(ale.exists(), "Fichier .ale absent");

    let content = std::fs::read_to_string(&ale).unwrap();
    assert!(content.contains("Heading"), "ALE: section Heading absente");

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

#[test]
fn export_csv_creates_file() {
    let src = tmp("export_csv_src");
    let dst = tmp("export_csv_dst");
    make_source(&src, 2, 512);

    run(ferr()
        .arg("copy")
        .arg(&src)
        .arg(&dst)
        .arg("--no-notify")
        .arg("--no-pdf")
        .arg("--quiet"));

    let manifest = dst.join("ferr-manifest.json");
    let csv = dst.join("export.csv");

    let out = run(ferr()
        .arg("export")
        .arg(&manifest)
        .arg("--format")
        .arg("csv")
        .arg("--output")
        .arg(&csv));

    assert!(
        out.status.success(),
        "ferr export CSV a échoué: {}",
        stderr(&out)
    );
    assert!(csv.exists(), "Fichier .csv absent");

    let content = std::fs::read_to_string(&csv).unwrap();
    assert!(content.starts_with("path,"), "CSV: en-tête incorrect");

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 7. `ferr report` — PDF
// ---------------------------------------------------------------------------

#[test]
fn report_creates_pdf() {
    let src = tmp("report_src");
    let dst = tmp("report_dst");
    make_source(&src, 3, 512);

    run(ferr()
        .arg("copy")
        .arg(&src)
        .arg(&dst)
        .arg("--no-notify")
        .arg("--no-pdf")
        .arg("--quiet"));

    let manifest = dst.join("ferr-manifest.json");
    let pdf = dst.join("test_report.pdf");

    let out = run(ferr()
        .arg("report")
        .arg(&manifest)
        .arg("--output")
        .arg(&pdf));

    assert!(
        out.status.success(),
        "ferr report a échoué: {}",
        stderr(&out)
    );
    assert!(pdf.exists(), "Fichier PDF absent");

    let header = &std::fs::read(&pdf).unwrap()[..4];
    assert_eq!(header, b"%PDF", "Magic bytes PDF absents");

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 8. `ferr profile` — save / list / show / delete
// ---------------------------------------------------------------------------

#[test]
fn profile_lifecycle() {
    let profile_name = format!("test_profile_{}", COUNTER.fetch_add(1, Ordering::Relaxed));

    // Sauvegarder
    let out = run(ferr()
        .arg("profile")
        .arg("save")
        .arg(&profile_name)
        .arg("--dest")
        .arg("/tmp/dummy")
        .arg("--hash")
        .arg("sha256"));
    assert!(
        out.status.success(),
        "profile save a échoué: {}",
        stderr(&out)
    );

    // Lister — doit contenir le profil
    let out = run(ferr().arg("profile").arg("list"));
    assert!(out.status.success());
    assert!(
        stdout(&out).contains(&profile_name),
        "Profil absent de la liste"
    );

    // Afficher — JSON valide
    let out = run(ferr().arg("profile").arg("show").arg(&profile_name));
    assert!(
        out.status.success(),
        "profile show a échoué: {}",
        stderr(&out)
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&out)).expect("profile show devrait retourner du JSON valide");
    assert_eq!(json["name"], profile_name.as_str());
    assert_eq!(json["hash_algo"], "sha256");

    // Supprimer
    let out = run(ferr().arg("profile").arg("delete").arg(&profile_name));
    assert!(
        out.status.success(),
        "profile delete a échoué: {}",
        stderr(&out)
    );

    // La liste ne doit plus contenir le profil
    let out = run(ferr().arg("profile").arg("list"));
    assert!(
        !stdout(&out).contains(&profile_name),
        "Profil encore présent après suppression"
    );
}

// ---------------------------------------------------------------------------
// 9. `ferr history` — enregistrement et liste
// ---------------------------------------------------------------------------

#[test]
fn history_records_session() {
    let db_dir = tmp("history_db");
    std::fs::create_dir_all(&db_dir).unwrap();

    let src = tmp("history_src");
    let dst = tmp("history_dst");
    make_source(&src, 2, 512);

    // Copie avec enregistrement de session, DB isolée
    let out = ferr()
        .env("FERR_DATA_DIR", &db_dir)
        .arg("copy")
        .arg(&src)
        .arg(&dst)
        .arg("--no-notify")
        .arg("--no-pdf")
        .arg("--quiet")
        .output()
        .unwrap();
    assert!(out.status.success(), "copy a échoué: {}", stderr(&out));

    // Lister les sessions
    let out = ferr()
        .env("FERR_DATA_DIR", &db_dir)
        .arg("history")
        .arg("list")
        .output()
        .unwrap();
    assert!(out.status.success());
    // La sortie doit contenir au moins une ligne de session
    let s = stdout(&out);
    assert!(
        !s.contains("Aucune session"),
        "Aucune session trouvée après copie"
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
    std::fs::remove_dir_all(&db_dir).ok();
}

// ---------------------------------------------------------------------------
// 10. NO_COLOR
// ---------------------------------------------------------------------------

#[test]
fn no_color_env_disables_ansi() {
    let src = tmp("nocolor_src");
    let dst = tmp("nocolor_dst");
    make_source(&src, 2, 512);

    let out = ferr()
        .env("NO_COLOR", "1")
        .arg("copy")
        .arg(&src)
        .arg(&dst)
        .arg("--no-notify")
        .arg("--no-pdf")
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "ferr copy avec NO_COLOR a échoué: {}",
        stderr(&out)
    );

    let combined = format!("{}{}", stdout(&out), stderr(&out));
    // Pas de séquences ANSI
    assert!(
        !combined.contains("\x1b["),
        "Des séquences ANSI ont été trouvées alors que NO_COLOR=1"
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}
