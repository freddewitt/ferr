//! Tests d'intégration ferr-core — exercent le pipeline complet via l'API Rust.
//!
//! Chaque test est autonome : il crée son propre répertoire temporaire,
//! exécute l'opération, vérifie le résultat et nettoie.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use ferr_core::{CopyJob, HashAlgo};
use ferr_report::{FileStatus, JobStatus};

// ---------------------------------------------------------------------------
// Utilitaires
// ---------------------------------------------------------------------------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn tmp(label: &str) -> PathBuf {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ferr_integ_{label}_{id}"))
}

/// Crée `n` fichiers de `size` octets dans `dir`.
fn make_source(dir: &Path, n: usize, size: usize) {
    std::fs::create_dir_all(dir).unwrap();
    for i in 0..n {
        let data: Vec<u8> = (0..size).map(|j| ((j + i * 7) % 256) as u8).collect();
        std::fs::write(dir.join(format!("clip{i:03}.dat")), &data).unwrap();
    }
}

/// Construit un CopyJob minimal pour les tests (pas de hooks).
fn job(src: PathBuf, dst: PathBuf) -> CopyJob {
    CopyJob {
        source: src,
        destinations: vec![dst],
        hash_algo: HashAlgo::XxHash64,
        resume: false,
        par2_redundancy: None,
        preserve_metadata: false,
        camera_mode: false,
        rename_template: None,
        auto_eject: false,
        dedup: false,
    }
}

// ---------------------------------------------------------------------------
// 1. Copie de base
// ---------------------------------------------------------------------------

#[test]
fn copy_basic_creates_files_and_manifest() {
    let src = tmp("copy_basic_src");
    let dst = tmp("copy_basic_dst");
    make_source(&src, 5, 4096);

    let manifest = ferr_core::run_copy(job(src.clone(), dst.clone()), |_| {}, &[]).unwrap();

    // Résumé global
    assert_eq!(manifest.total_files, 5);
    assert_eq!(manifest.status, JobStatus::Ok);
    assert!(manifest.total_size_bytes > 0);
    assert!(manifest.duration_secs >= 0.0);

    // Chaque fichier est présent à la destination avec le bon statut
    for entry in &manifest.files {
        let path = dst.join(&entry.path);
        assert!(path.exists(), "Fichier manquant à la dest : {}", entry.path);
        assert_eq!(
            entry.status,
            FileStatus::Ok,
            "Statut inattendu pour {}",
            entry.path
        );
        assert!(!entry.hash.is_empty(), "Hash vide pour {}", entry.path);
        assert_eq!(entry.hash_algo, "xxhash64");
    }

    // Le manifest JSON est sauvegardé et rechargeable
    let manifest_path = ferr_core::find_manifest_path(&dst).unwrap();
    assert!(manifest_path.exists(), "ferr-manifest.json absent");
    let loaded = ferr_report::load_manifest(&manifest_path).unwrap();
    assert_eq!(loaded.total_files, manifest.total_files);
    assert_eq!(loaded.files.len(), manifest.files.len());

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 2. Copie miroir (2 destinations)
// ---------------------------------------------------------------------------

#[test]
fn copy_mirror_two_destinations() {
    let src = tmp("mirror_src");
    let dst1 = tmp("mirror_dst1");
    let dst2 = tmp("mirror_dst2");
    make_source(&src, 3, 2048);

    let mut j = job(src.clone(), dst1.clone());
    j.destinations.push(dst2.clone());

    let manifest = ferr_core::run_copy(j, |_| {}, &[]).unwrap();
    assert_eq!(manifest.total_files, 3);

    // Les fichiers existent sur les deux destinations
    for entry in &manifest.files {
        assert!(
            dst1.join(&entry.path).exists(),
            "dst1 manque {}",
            entry.path
        );
        assert!(
            dst2.join(&entry.path).exists(),
            "dst2 manque {}",
            entry.path
        );
    }

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst1).ok();
    std::fs::remove_dir_all(&dst2).ok();
}

// ---------------------------------------------------------------------------
// 3. Copie avec SHA-256
// ---------------------------------------------------------------------------

#[test]
fn copy_sha256() {
    let src = tmp("sha256_src");
    let dst = tmp("sha256_dst");
    make_source(&src, 2, 1024);

    let mut j = job(src.clone(), dst.clone());
    j.hash_algo = HashAlgo::Sha256;

    let manifest = ferr_core::run_copy(j, |_| {}, &[]).unwrap();

    assert_eq!(manifest.total_files, 2);
    for entry in &manifest.files {
        assert_eq!(entry.hash_algo, "sha256");
        // SHA-256 = 64 hex chars
        assert_eq!(entry.hash.len(), 64, "Longueur hash SHA-256 inattendue");
    }

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 4. Vérification d'une copie correcte
// ---------------------------------------------------------------------------

#[test]
fn verify_ok_after_copy() {
    let src = tmp("verify_ok_src");
    let dst = tmp("verify_ok_dst");
    make_source(&src, 4, 2048);

    let manifest = ferr_core::run_copy(job(src.clone(), dst.clone()), |_| {}, &[]).unwrap();
    let manifest_path = ferr_core::find_manifest_path(&dst).unwrap();

    let hasher: Box<dyn ferr_hash::Hasher> = Box::new(ferr_hash::XxHasher);
    let report = ferr_verify::verify_manifest(&manifest, &dst, hasher.as_ref()).unwrap();

    assert_eq!(report.exit_code(), 0, "Vérification devrait être OK");
    assert!(report.missing.is_empty());
    assert!(report.corrupted.is_empty());
    assert_eq!(report.ok.len(), manifest.total_files);

    // Aussi depuis les répertoires
    let report2 = ferr_verify::verify_dirs(&src, &dst, hasher.as_ref()).unwrap();
    assert_eq!(report2.exit_code(), 0);

    drop(manifest_path);
    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 5. Détection d'un fichier corrompu
// ---------------------------------------------------------------------------

#[test]
fn verify_detects_corrupted_file() {
    let src = tmp("corrupt_src");
    let dst = tmp("corrupt_dst");
    make_source(&src, 3, 4096);

    let manifest = ferr_core::run_copy(job(src.clone(), dst.clone()), |_| {}, &[]).unwrap();

    // Corrompre le premier fichier à la destination
    let first = &manifest.files[0].path;
    let corrupt_path = dst.join(first);
    let mut data = std::fs::read(&corrupt_path).unwrap();
    data[0] ^= 0xFF;
    data[1] ^= 0xFF;
    std::fs::write(&corrupt_path, &data).unwrap();

    let hasher: Box<dyn ferr_hash::Hasher> = Box::new(ferr_hash::XxHasher);
    let report = ferr_verify::verify_manifest(&manifest, &dst, hasher.as_ref()).unwrap();

    assert!(
        !report.corrupted.is_empty(),
        "La corruption n'a pas été détectée"
    );
    assert_ne!(report.exit_code(), 0);

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 6. Détection d'un fichier manquant
// ---------------------------------------------------------------------------

#[test]
fn verify_detects_missing_file() {
    let src = tmp("missing_src");
    let dst = tmp("missing_dst");
    make_source(&src, 3, 1024);

    let manifest = ferr_core::run_copy(job(src.clone(), dst.clone()), |_| {}, &[]).unwrap();

    // Supprimer un fichier à la destination
    let removed = dst.join(&manifest.files[0].path);
    std::fs::remove_file(&removed).unwrap();

    let hasher: Box<dyn ferr_hash::Hasher> = Box::new(ferr_hash::XxHasher);
    let report = ferr_verify::verify_manifest(&manifest, &dst, hasher.as_ref()).unwrap();

    assert!(
        !report.missing.is_empty(),
        "Le fichier manquant n'a pas été détecté"
    );
    assert_eq!(report.exit_code(), 1); // code 1 = manquants uniquement

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 7. Scan bit rot
// ---------------------------------------------------------------------------

#[test]
fn scan_bitrot_clean() {
    let src = tmp("bitrot_clean_src");
    let dst = tmp("bitrot_clean_dst");
    make_source(&src, 4, 2048);

    let manifest = ferr_core::run_copy(job(src.clone(), dst.clone()), |_| {}, &[]).unwrap();

    let hasher: Box<dyn ferr_hash::Hasher> = Box::new(ferr_hash::XxHasher);
    let report = ferr_verify::scan_bitrot(&dst, &manifest, hasher.as_ref(), None, |_| {}).unwrap();

    assert!(report.corrupted.is_empty(), "Faux positif bit rot");
    assert_eq!(report.scanned, manifest.total_files);
    assert_eq!(report.skipped, 0);

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

#[test]
fn scan_bitrot_detects_corruption() {
    let src = tmp("bitrot_detect_src");
    let dst = tmp("bitrot_detect_dst");
    make_source(&src, 3, 4096);

    let manifest = ferr_core::run_copy(job(src.clone(), dst.clone()), |_| {}, &[]).unwrap();

    // Introduire du bit rot (modification in-place)
    let target = dst.join(&manifest.files[1].path);
    let mut data = std::fs::read(&target).unwrap();
    data[512] ^= 0xAA;
    std::fs::write(&target, &data).unwrap();

    let hasher: Box<dyn ferr_hash::Hasher> = Box::new(ferr_hash::XxHasher);
    let scan_calls = std::sync::atomic::AtomicUsize::new(0);
    let report = ferr_verify::scan_bitrot(&dst, &manifest, hasher.as_ref(), None, |p| {
        scan_calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        assert!(p.scanned <= p.total);
    })
    .unwrap();

    assert_eq!(
        report.corrupted.len(),
        1,
        "Devrait détecter exactement 1 fichier corrompu"
    );
    assert!(
        scan_calls.load(std::sync::atomic::Ordering::Relaxed) > 0,
        "La callback de progrès n'a jamais été appelée"
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 8. Mode dry-run
// ---------------------------------------------------------------------------

#[test]
fn dry_run_writes_nothing() {
    let src = tmp("dryrun_src");
    let dst = tmp("dryrun_dst");
    make_source(&src, 3, 1024);

    let j = job(src.clone(), dst.clone());
    let report = ferr_core::dry_run(&j).unwrap();

    assert_eq!(report.total_files, 3);
    assert!(report.total_size_bytes > 0);

    // Aucun fichier créé à la destination
    assert!(
        !dst.exists() || std::fs::read_dir(&dst).unwrap().next().is_none(),
        "dry_run ne devrait créer aucun fichier"
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 9. Export ALE
// ---------------------------------------------------------------------------

#[test]
fn export_ale_produces_valid_file() {
    let src = tmp("ale_src");
    let dst = tmp("ale_dst");
    make_source(&src, 4, 512);

    let manifest = ferr_core::run_copy(job(src.clone(), dst.clone()), |_| {}, &[]).unwrap();
    let manifest_path = ferr_core::find_manifest_path(&dst).unwrap();

    let ale_path = dst.join("report.ale");
    ferr_report::export_ale(&manifest, &ale_path).unwrap();

    assert!(ale_path.exists());
    let content = std::fs::read_to_string(&ale_path).unwrap();

    // Structure ALE minimale
    assert!(content.contains("Heading"), "ALE: section Heading absente");
    assert!(content.contains("Column"), "ALE: section Column absente");
    assert!(content.contains("Data"), "ALE: section Data absente");
    assert!(content.contains("Name"), "ALE: colonne Name absente");
    // Chaque fichier doit apparaître (l'ALE utilise le file_stem, sans extension)
    for entry in &manifest.files {
        let stem = std::path::Path::new(&entry.path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| entry.path.clone());
        assert!(
            content.contains(&stem),
            "ALE: stem '{}' absent (depuis '{}')",
            stem,
            entry.path
        );
    }

    drop(manifest_path);
    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 10. Export CSV
// ---------------------------------------------------------------------------

#[test]
fn export_csv_produces_valid_file() {
    let src = tmp("csv_src");
    let dst = tmp("csv_dst");
    make_source(&src, 3, 512);

    let manifest = ferr_core::run_copy(job(src.clone(), dst.clone()), |_| {}, &[]).unwrap();

    let csv_path = dst.join("report.csv");
    ferr_report::export_csv(&manifest, &csv_path).unwrap();

    assert!(csv_path.exists());
    let content = std::fs::read_to_string(&csv_path).unwrap();

    // En-tête CSV
    assert!(content.starts_with("path,"), "CSV: en-tête incorrect");
    // Autant de lignes de données que de fichiers
    let data_lines = content.lines().count() - 1; // -1 pour l'en-tête
    assert_eq!(
        data_lines, manifest.total_files,
        "CSV: nombre de lignes incorrect"
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 11. Rapport PDF
// ---------------------------------------------------------------------------

#[test]
fn pdf_report_is_non_empty() {
    let src = tmp("pdf_src");
    let dst = tmp("pdf_dst");
    make_source(&src, 5, 1024);

    let _manifest = ferr_core::run_copy(job(src.clone(), dst.clone()), |_| {}, &[]).unwrap();
    let manifest_path = ferr_core::find_manifest_path(&dst).unwrap();
    let loaded = ferr_report::load_manifest(&manifest_path).unwrap();

    let pdf_path = dst.join("report.pdf");
    ferr_pdf::generate_report(&loaded, &pdf_path).unwrap();

    assert!(pdf_path.exists());
    let size = std::fs::metadata(&pdf_path).unwrap().len();
    assert!(size > 1024, "PDF trop petit ({size} octets)");

    // Vérifier que c'est un vrai PDF (magic bytes %PDF)
    let header = &std::fs::read(&pdf_path).unwrap()[..4];
    assert_eq!(header, b"%PDF", "Magic bytes PDF absents");

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 12. Resume — saute les fichiers déjà copiés
// ---------------------------------------------------------------------------

#[test]
fn resume_skips_already_copied_files() {
    let src = tmp("resume_src");
    let dst = tmp("resume_dst");
    make_source(&src, 5, 2048);

    // Première copie complète
    let m1 = ferr_core::run_copy(job(src.clone(), dst.clone()), |_| {}, &[]).unwrap();
    assert_eq!(m1.total_files, 5);

    // Ajouter un fichier supplémentaire à la source
    std::fs::write(src.join("extra_new.dat"), vec![0xAB; 1024]).unwrap();

    // Reprise
    let mut j = job(src.clone(), dst.clone());
    j.resume = true;
    let m2 = ferr_core::run_copy(j, |_| {}, &[]).unwrap();

    // Le nouveau fichier doit avoir été copié
    assert!(dst.join("extra_new.dat").exists());
    // Les fichiers skipped ont le statut Skipped
    let skipped = m2
        .files
        .iter()
        .filter(|f| f.status == FileStatus::Skipped)
        .count();
    assert!(
        skipped > 0,
        "Des fichiers auraient dû être sautés en mode resume"
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 13. Progrès — callback appelé pour chaque fichier
// ---------------------------------------------------------------------------

#[test]
fn copy_progress_callback_invoked() {
    let src = tmp("progress_src");
    let dst = tmp("progress_dst");
    make_source(&src, 6, 1024);

    let calls = std::sync::atomic::AtomicUsize::new(0);
    ferr_core::run_copy(
        job(src.clone(), dst.clone()),
        |p| {
            calls.fetch_add(1, Ordering::Relaxed);
            assert!(p.total_files_done <= p.total_files);
            assert!(p.file_bytes_done <= p.file_bytes_total || p.file_bytes_total == 0);
        },
        &[],
    )
    .unwrap();

    assert!(
        calls.load(Ordering::Relaxed) > 0,
        "La callback n'a jamais été appelée"
    );

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 14. PAR2 — ignoré si par2cmdline non installé
// ---------------------------------------------------------------------------

#[test]
#[ignore = "nécessite par2cmdline installé sur le système"]
fn copy_with_par2_generates_files() {
    let src = tmp("par2_src");
    let dst = tmp("par2_dst");
    // Fichiers assez grands pour PAR2
    make_source(&src, 3, 128 * 1024);

    let mut j = job(src.clone(), dst.clone());
    j.par2_redundancy = Some(10);

    let manifest = ferr_core::run_copy(j, |_| {}, &[]).unwrap();
    assert_eq!(manifest.total_files, 3);

    // Les fichiers PAR2 doivent exister dans _par2/
    let par2_dir = dst.join("_par2");
    assert!(par2_dir.exists(), "_par2/ absent");
    let par2_count = std::fs::read_dir(&par2_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "par2").unwrap_or(false))
        .count();
    assert!(par2_count >= 1, "Aucun fichier .par2 généré");

    // Tous les fichiers marqués par2_generated
    for entry in &manifest.files {
        assert!(
            entry.par2_generated,
            "{} n'est pas marqué par2_generated",
            entry.path
        );
    }

    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
}

// ---------------------------------------------------------------------------
// 15. Session history — enregistrement via hook et lecture
// ---------------------------------------------------------------------------

#[test]
fn session_record_and_retrieve() {
    // Utiliser une DB isolée dans un répertoire temporaire
    let db_dir = tmp("session_db");
    std::fs::create_dir_all(&db_dir).unwrap();
    std::env::set_var("FERR_DATA_DIR", &db_dir);

    let src = tmp("session_src");
    let dst = tmp("session_dst");
    make_source(&src, 2, 512);

    // Hook de session injecté — equivalent à SessionHook dans ferr-cli
    struct RecordHook;
    impl ferr_core::PostCopyHook for RecordHook {
        fn on_copy_done(&self, manifest: &ferr_report::Manifest) -> anyhow::Result<()> {
            ferr_session::record_session(manifest).map(|_| ())
        }
    }
    let hooks: Vec<ferr_core::HookRef> = vec![Arc::new(RecordHook)];

    let manifest = ferr_core::run_copy(job(src.clone(), dst.clone()), |_| {}, &hooks).unwrap();
    assert_eq!(manifest.total_files, 2);

    // La session doit être enregistrée
    let sessions = ferr_session::list_sessions(ferr_session::SessionFilter::default()).unwrap();
    assert!(!sessions.is_empty(), "Aucune session enregistrée");

    let latest = sessions.first().unwrap();
    assert_eq!(latest.total_files, 2);
    assert_eq!(latest.source, src.to_string_lossy().as_ref());

    // Recherche par hash
    let first_hash = &manifest.files[0].hash;
    let records = ferr_session::find_file_by_hash(first_hash).unwrap();
    assert!(!records.is_empty(), "Fichier introuvable par hash");

    std::env::remove_var("FERR_DATA_DIR");
    std::fs::remove_dir_all(&src).ok();
    std::fs::remove_dir_all(&dst).ok();
    std::fs::remove_dir_all(&db_dir).ok();
}
