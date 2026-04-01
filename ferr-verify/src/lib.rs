//! Vérification d'intégrité et détection de bit rot.
//!
//! Fournit [`verify_manifest`] et [`verify_dirs`] pour comparer les hashes
//! source/destination, ainsi que [`scan_bitrot`] pour détecter toute
//! modification silencieuse sur les fichiers déjà copiés.

use std::path::{Path, PathBuf};
use std::time::Instant;

// ---------------------------------------------------------------------------
// VerifyReport
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct VerifyReport {
    pub ok: Vec<PathBuf>,
    pub missing: Vec<PathBuf>,
    pub corrupted: Vec<PathBuf>,
    pub total_size_bytes: u64,
    pub duration_secs: f64,
}

impl VerifyReport {
    /// Code de sortie POSIX :
    /// 0 = tout ok · 1 = manquants · 2 = corrompus · 3 = les deux
    pub fn exit_code(&self) -> i32 {
        match (!self.missing.is_empty(), !self.corrupted.is_empty()) {
            (false, false) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (true, true) => 3,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.missing.is_empty() && self.corrupted.is_empty()
    }
}

// ---------------------------------------------------------------------------
// verify_dirs
// ---------------------------------------------------------------------------

/// Compare deux dossiers fichier par fichier.
/// La liste de référence est construite depuis `src`.
pub fn verify_dirs(
    src: &Path,
    dest: &Path,
    hasher: &dyn ferr_hash::Hasher,
) -> anyhow::Result<VerifyReport> {
    let start = Instant::now();
    let mut report = VerifyReport::default();

    let src_files = collect_files(src)?;

    for src_file in &src_files {
        let rel = src_file.strip_prefix(src)?;
        let dest_file = dest.join(rel);

        if !dest_file.exists() {
            report.missing.push(rel.to_path_buf());
            continue;
        }

        let src_hash = hasher.hash_file(src_file)?;
        report.total_size_bytes += src_hash.bytes_read;

        let dest_hash = hasher.hash_file(&dest_file)?;

        if src_hash.hex == dest_hash.hex {
            report.ok.push(rel.to_path_buf());
        } else {
            report.corrupted.push(rel.to_path_buf());
        }
    }

    report.duration_secs = start.elapsed().as_secs_f64();
    Ok(report)
}

// ---------------------------------------------------------------------------
// verify_manifest
// ---------------------------------------------------------------------------

/// Vérifie les fichiers listés dans un manifest contre un dossier destination.
pub fn verify_manifest(
    manifest: &ferr_report::Manifest,
    dest: &Path,
    hasher: &dyn ferr_hash::Hasher,
) -> anyhow::Result<VerifyReport> {
    let start = Instant::now();
    let mut report = VerifyReport::default();

    for entry in &manifest.files {
        let rel = PathBuf::from(&entry.path);
        let dest_file = safe_join(dest, &rel)?;

        if !dest_file.exists() {
            report.missing.push(rel);
            continue;
        }

        let dest_hash = hasher.hash_file(&dest_file)?;
        report.total_size_bytes += dest_hash.bytes_read;

        if dest_hash.hex == entry.hash {
            report.ok.push(rel);
        } else {
            report.corrupted.push(rel);
        }
    }

    report.duration_secs = start.elapsed().as_secs_f64();
    Ok(report)
}

// ---------------------------------------------------------------------------
// Utilitaires
// ---------------------------------------------------------------------------

/// Parcourt récursivement un dossier et retourne tous les fichiers.
fn collect_files(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_recursive(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// Vérifie qu'un chemin relatif issu d'un manifest externe ne contient pas
/// de composantes traversantes (`..` ou chemin absolu).
///
/// # Errors
/// Retourne une erreur si `rel` est absolu ou contient `..`.
fn safe_join(base: &Path, rel: &Path) -> anyhow::Result<PathBuf> {
    if rel.is_absolute() {
        anyhow::bail!("Chemin absolu refusé dans le manifest : {}", rel.display());
    }
    for component in rel.components() {
        if matches!(component, std::path::Component::ParentDir) {
            anyhow::bail!(
                "Traversée de répertoire refusée dans le manifest : {}",
                rel.display()
            );
        }
    }
    Ok(base.join(rel))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ferr_hash::{Hasher as _, XxHasher};

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ferr_verify_{name}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Crée une arborescence de test :
    ///   dir/file_a.bin  = "content_a"
    ///   dir/sub/file_b.bin = "content_b"
    fn create_test_tree(base: &Path) {
        std::fs::create_dir_all(base.join("sub")).unwrap();
        std::fs::write(base.join("file_a.bin"), b"content_a").unwrap();
        std::fs::write(base.join("sub").join("file_b.bin"), b"content_b").unwrap();
    }

    #[test]
    fn identical_dirs_all_ok() {
        let base = tmp_dir("identical");
        let src = base.join("src");
        let dst = base.join("dst");
        create_test_tree(&src);
        create_test_tree(&dst);

        let report = verify_dirs(&src, &dst, &XxHasher).unwrap();
        assert_eq!(report.exit_code(), 0);
        assert!(report.missing.is_empty());
        assert!(report.corrupted.is_empty());
        assert_eq!(report.ok.len(), 2);

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn missing_file_detected() {
        let base = tmp_dir("missing");
        let src = base.join("src");
        let dst = base.join("dst");
        create_test_tree(&src);
        create_test_tree(&dst);

        // Supprimer un fichier en destination
        std::fs::remove_file(dst.join("file_a.bin")).unwrap();

        let report = verify_dirs(&src, &dst, &XxHasher).unwrap();
        assert_eq!(report.exit_code(), 1);
        assert_eq!(report.missing.len(), 1);
        assert_eq!(report.missing[0], PathBuf::from("file_a.bin"));
        assert!(report.corrupted.is_empty());

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn corrupted_file_detected() {
        let base = tmp_dir("corrupted");
        let src = base.join("src");
        let dst = base.join("dst");
        create_test_tree(&src);
        create_test_tree(&dst);

        // Corrompre un octet en destination
        let mut data = std::fs::read(dst.join("file_a.bin")).unwrap();
        data[0] ^= 0xFF;
        std::fs::write(dst.join("file_a.bin"), &data).unwrap();

        let report = verify_dirs(&src, &dst, &XxHasher).unwrap();
        assert_eq!(report.exit_code(), 2);
        assert!(report.missing.is_empty());
        assert_eq!(report.corrupted.len(), 1);
        assert_eq!(report.corrupted[0], PathBuf::from("file_a.bin"));

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn both_missing_and_corrupted() {
        let base = tmp_dir("both");
        let src = base.join("src");
        let dst = base.join("dst");
        create_test_tree(&src);
        create_test_tree(&dst);

        // Supprimer file_a
        std::fs::remove_file(dst.join("file_a.bin")).unwrap();
        // Corrompre file_b
        let mut data = std::fs::read(dst.join("sub").join("file_b.bin")).unwrap();
        data[0] ^= 0xFF;
        std::fs::write(dst.join("sub").join("file_b.bin"), &data).unwrap();

        let report = verify_dirs(&src, &dst, &XxHasher).unwrap();
        assert_eq!(report.exit_code(), 3);

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn exit_code_values() {
        let mut r = VerifyReport::default();
        assert_eq!(r.exit_code(), 0);

        r.missing.push(PathBuf::from("x"));
        assert_eq!(r.exit_code(), 1);

        r.missing.clear();
        r.corrupted.push(PathBuf::from("y"));
        assert_eq!(r.exit_code(), 2);

        r.missing.push(PathBuf::from("x"));
        assert_eq!(r.exit_code(), 3);
    }

    #[test]
    fn verify_manifest_ok() {
        let base = tmp_dir("manifest_ok");
        let dst = base.join("dst");
        create_test_tree(&dst);

        let hasher = XxHasher;
        let hash_a = hasher.hash_file(&dst.join("file_a.bin")).unwrap();
        let hash_b = hasher
            .hash_file(&dst.join("sub").join("file_b.bin"))
            .unwrap();

        let manifest = ferr_report::Manifest {
            ferr_version: "0.1.0".into(),
            generated_at: "2025-01-01T00:00:00Z".into(),
            hostname: "host".into(),
            source_path: "/src".into(),
            destinations: Vec::new(),
            total_files: 2,
            total_size_bytes: hash_a.bytes_read + hash_b.bytes_read,
            duration_secs: 0.1,
            status: ferr_report::JobStatus::Ok,
            files: vec![
                ferr_report::FileEntry {
                    path: "file_a.bin".into(),
                    size: hash_a.bytes_read,
                    hash_algo: "xxhash64".into(),
                    hash: hash_a.hex.clone(),
                    modified_at: "2025-01-01T00:00:00Z".into(),
                    status: ferr_report::FileStatus::Ok,
                    par2_generated: false,
                },
                ferr_report::FileEntry {
                    path: "sub/file_b.bin".into(),
                    size: hash_b.bytes_read,
                    hash_algo: "xxhash64".into(),
                    hash: hash_b.hex.clone(),
                    modified_at: "2025-01-01T00:00:00Z".into(),
                    status: ferr_report::FileStatus::Ok,
                    par2_generated: false,
                },
            ],
        };

        let report = verify_manifest(&manifest, &dst, &hasher).unwrap();
        assert_eq!(report.exit_code(), 0);
        assert_eq!(report.ok.len(), 2);

        std::fs::remove_dir_all(&base).ok();
    }
}

// ---------------------------------------------------------------------------
// Détection de bit rot
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ScanProgress {
    pub scanned: usize,
    pub total: usize,
    pub current: PathBuf,
}

#[derive(Debug)]
pub struct BitRotEntry {
    pub path: PathBuf,
    pub expected_hash: String,
    pub actual_hash: String,
    pub last_ok_date: Option<String>,
}

#[derive(Debug)]
pub struct BitRotReport {
    pub scanned: usize,
    pub skipped: usize,
    pub corrupted: Vec<BitRotEntry>,
    pub scan_date: String,
}

/// Scanne tous les fichiers d'une destination et compare leurs hash au manifest.
///
/// - `since` : si fourni, ignore les fichiers dont `modified_at` est postérieur
///   à cette date (fichiers récemment modifiés → déjà couverts par la copie initiale).
///   Concrètement, sauter les fichiers dont la date de scan précédente est plus
///   récente que `since`.
pub fn scan_bitrot(
    dest: &Path,
    manifest: &ferr_report::Manifest,
    hasher: &dyn ferr_hash::Hasher,
    since: Option<chrono::DateTime<chrono::Utc>>,
    on_progress: impl Fn(ScanProgress) + Send,
) -> anyhow::Result<BitRotReport> {
    let scan_date = chrono::Utc::now().to_rfc3339();
    let total = manifest.files.len();
    let mut scanned = 0usize;
    let mut skipped = 0usize;
    let mut corrupted = Vec::new();

    for entry in &manifest.files {
        let rel = PathBuf::from(&entry.path);

        // Filtre --since : skip si le fichier a une date enregistrée après since
        if let Some(since_dt) = since {
            if let Ok(file_dt) = chrono::DateTime::parse_from_rfc3339(&entry.modified_at) {
                if file_dt.with_timezone(&chrono::Utc) > since_dt {
                    skipped += 1;
                    continue;
                }
            }
        }

        on_progress(ScanProgress {
            scanned,
            total,
            current: rel.clone(),
        });

        let dest_file = safe_join(dest, &rel)?;
        if !dest_file.exists() {
            corrupted.push(BitRotEntry {
                path: rel,
                expected_hash: entry.hash.clone(),
                actual_hash: "(manquant)".to_string(),
                last_ok_date: Some(entry.modified_at.clone()),
            });
            scanned += 1;
            continue;
        }

        let actual = hasher.hash_file(&dest_file)?;
        if actual.hex != entry.hash {
            corrupted.push(BitRotEntry {
                path: rel,
                expected_hash: entry.hash.clone(),
                actual_hash: actual.hex,
                last_ok_date: Some(entry.modified_at.clone()),
            });
        }
        scanned += 1;
    }

    Ok(BitRotReport {
        scanned,
        skipped,
        corrupted,
        scan_date,
    })
}

#[cfg(test)]
mod scan_tests {
    use super::*;
    use ferr_hash::{Hasher as _, XxHasher};

    fn make_manifest_for_dir(dir: &Path) -> ferr_report::Manifest {
        let hasher = XxHasher;
        let files = vec![{
            let h = hasher.hash_file(&dir.join("file_a.bin")).unwrap();
            ferr_report::FileEntry {
                path: "file_a.bin".into(),
                size: h.bytes_read,
                hash_algo: "xxhash64".into(),
                hash: h.hex,
                modified_at: "2020-01-01T00:00:00Z".into(),
                status: ferr_report::FileStatus::Ok,
                par2_generated: false,
            }
        }];
        ferr_report::Manifest {
            ferr_version: "0.1.0".into(),
            generated_at: "2020-01-01T00:00:00Z".into(),
            hostname: "host".into(),
            source_path: "/src".into(),
            destinations: Vec::new(),
            total_files: 1,
            total_size_bytes: 9,
            duration_secs: 0.1,
            status: ferr_report::JobStatus::Ok,
            files,
        }
    }

    #[test]
    fn scan_no_bitrot() {
        let base = std::env::temp_dir().join("ferr_scan_ok");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("file_a.bin"), b"content_a").unwrap();

        let manifest = make_manifest_for_dir(&base);
        let report = scan_bitrot(&base, &manifest, &XxHasher, None, |_| {}).unwrap();

        assert_eq!(report.scanned, 1);
        assert!(report.corrupted.is_empty());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn scan_detects_bitrot() {
        let base = std::env::temp_dir().join("ferr_scan_corrupt");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("file_a.bin"), b"content_a").unwrap();

        let manifest = make_manifest_for_dir(&base);

        // Corrompre le fichier
        std::fs::write(base.join("file_a.bin"), b"CORRUPTED").unwrap();

        let report = scan_bitrot(&base, &manifest, &XxHasher, None, |_| {}).unwrap();
        assert_eq!(report.corrupted.len(), 1);
        assert_eq!(report.corrupted[0].path, PathBuf::from("file_a.bin"));
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn scan_since_skips_recent() {
        let base = std::env::temp_dir().join("ferr_scan_since");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("file_a.bin"), b"content_a").unwrap();

        let manifest = make_manifest_for_dir(&base);

        // since = future → le fichier (modified_at = 2020) est AVANT since
        // donc il sera scanné, pas ignoré
        let since = chrono::DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let report = scan_bitrot(&base, &manifest, &XxHasher, Some(since), |_| {}).unwrap();
        assert_eq!(report.skipped, 0); // 2020 < 2025 → pas skippé
        assert_eq!(report.scanned, 1);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn scan_since_skips_file_modified_after_since() {
        let base = std::env::temp_dir().join("ferr_scan_since_skip");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("file_a.bin"), b"content_a").unwrap();

        // Manifest avec modified_at en 2030 (futur par rapport à since)
        let files = vec![{
            let hasher = XxHasher;
            let h = hasher.hash_file(&base.join("file_a.bin")).unwrap();
            ferr_report::FileEntry {
                path: "file_a.bin".into(),
                size: h.bytes_read,
                hash_algo: "xxhash64".into(),
                hash: h.hex,
                modified_at: "2030-01-01T00:00:00Z".into(), // après since
                status: ferr_report::FileStatus::Ok,
                par2_generated: false,
            }
        }];
        let manifest = ferr_report::Manifest {
            ferr_version: "0.1.0".into(),
            generated_at: "2030-01-01T00:00:00Z".into(),
            hostname: "host".into(),
            source_path: "/src".into(),
            destinations: Vec::new(),
            total_files: 1,
            total_size_bytes: 9,
            duration_secs: 0.1,
            status: ferr_report::JobStatus::Ok,
            files,
        };

        // since = 2025 → fichier modified_at=2030 EST APRÈS since → doit être skippé
        let since = chrono::DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let report = scan_bitrot(&base, &manifest, &XxHasher, Some(since), |_| {}).unwrap();
        assert_eq!(
            report.skipped, 1,
            "Le fichier postérieur à since doit être ignoré"
        );
        assert_eq!(report.scanned, 0);

        std::fs::remove_dir_all(&base).ok();
    }
}
