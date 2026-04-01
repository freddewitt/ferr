//! Transfert de fichiers sécurisé avec vérification d'intégrité.
//!
//! Ce module implémente la copie streaming vers plusieurs destinations simultanément,
//! avec vérification hash post-écriture et retry automatique en cas d'échec.

use std::path::{Path, PathBuf};

use rayon::prelude::*;

// ---------------------------------------------------------------------------
// Trait Destination
// ---------------------------------------------------------------------------

/// Abstraction d'une destination de copie.
pub trait Destination: Send + Sync {
    /// Copie le fichier source `src` vers cette destination sous le chemin relatif `rel_path`.
    /// L'écriture est atomique (fichier temporaire puis renommage).
    fn write_file(&self, rel_path: &Path, src: &Path) -> anyhow::Result<()>;
    fn sync(&self) -> anyhow::Result<()>;
    fn root(&self) -> &Path;
}

// ---------------------------------------------------------------------------
// LocalDest
// ---------------------------------------------------------------------------

/// Destination sur le système de fichiers local.
pub struct LocalDest {
    pub(crate) root: PathBuf,
}

impl LocalDest {
    /// Crée une nouvelle destination locale pointant vers `root`.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl Destination for LocalDest {
    /// Copie `src` vers `root/rel_path` via streaming (aucun tampon mémoire complet).
    fn write_file(&self, rel_path: &Path, src: &Path) -> anyhow::Result<()> {
        let dest = self.root.join(rel_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Écriture atomique : copie vers fichier temporaire, puis renommage
        let tmp = dest.with_extension("ferr_tmp");
        std::fs::copy(src, &tmp)?;
        std::fs::rename(&tmp, &dest)?;
        Ok(())
    }

    fn sync(&self) -> anyhow::Result<()> {
        // sync() au niveau dossier racine — les fichiers individuels
        // sont sync'd dans copy_file via File::sync_all()
        Ok(())
    }

    fn root(&self) -> &Path {
        &self.root
    }
}

// ---------------------------------------------------------------------------
// Types de résultats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DestResult {
    pub path: PathBuf,
    pub success: bool,
    pub retried: bool,
    pub error: Option<String>,
}

pub struct TransferResult {
    pub src_hash: ferr_hash::HashResult,
    pub dest_results: Vec<DestResult>,
    pub skipped: bool,
    pub metadata_preserved: bool,
}

// ---------------------------------------------------------------------------
// Flush disque cross-platform
// ---------------------------------------------------------------------------

fn sync_file(path: &Path) -> anyhow::Result<()> {
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.sync_all()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// copy_file
// ---------------------------------------------------------------------------

/// Copie `src` vers toutes les destinations en mode streaming (pas de tampon mémoire complet).
///
/// - `rel_path` : chemin relatif utilisé pour `Destination::write_file`
/// - `resume_manifest` : si fourni et que le fichier est déjà OK, la copie est ignorée
/// - `on_progress` : appelé avec le nombre d'octets lus depuis la source
pub fn copy_file(
    src: &Path,
    rel_path: &Path,
    destinations: &[Box<dyn Destination>],
    hasher: &dyn ferr_hash::Hasher,
    resume_manifest: Option<&ferr_report::Manifest>,
    on_progress: impl Fn(u64),
    preserve_metadata: bool,
) -> anyhow::Result<TransferResult> {
    // --- Vérification reprise ------------------------------------------------
    if let Some(manifest) = resume_manifest {
        let rel_str = rel_path.to_string_lossy();
        if let Some(entry) = manifest.files.iter().find(|e| e.path == rel_str.as_ref()) {
            if entry.status == ferr_report::FileStatus::Ok {
                on_progress(entry.size);
                return Ok(TransferResult {
                    src_hash: ferr_hash::HashResult {
                        algo: if entry.hash_algo == "sha256" {
                            ferr_hash::HashAlgo::Sha256
                        } else {
                            ferr_hash::HashAlgo::XxHash64
                        },
                        hex: entry.hash.clone(),
                        bytes_read: entry.size,
                    },
                    dest_results: destinations
                        .iter()
                        .map(|d| DestResult {
                            path: d.root().join(rel_path),
                            success: true,
                            retried: false,
                            error: None,
                        })
                        .collect(),
                    skipped: true,
                    metadata_preserved: false,
                });
            }
        }
    }

    // --- Hash source en streaming (aucun tampon complet en RAM) --------------
    let src_hash = hasher.hash_file(src)?;
    on_progress(src_hash.bytes_read);

    // --- Écriture parallèle + vérification sur toutes les destinations -------
    let results: Vec<DestResult> = destinations
        .par_iter()
        .map(|dest| {
            write_verify_retry(dest.as_ref(), rel_path, src, &src_hash.hex, hasher)
        })
        .collect();

    // --- Préservation des métadonnées ----------------------------------------
    let metadata_preserved = if preserve_metadata {
        let src_meta = std::fs::metadata(src)?;
        let mut all_ok = true;
        for dest in destinations {
            let dest_path = dest.root().join(rel_path);
            if let Err(e) = copy_metadata(src, &src_meta, &dest_path) {
                tracing::warn!(
                    path = %dest_path.display(),
                    "métadonnées non préservées : {e}"
                );
                all_ok = false;
            }
        }
        all_ok
    } else {
        false
    };

    Ok(TransferResult {
        src_hash,
        dest_results: results,
        skipped: false,
        metadata_preserved,
    })
}

// ---------------------------------------------------------------------------
// Helpers internes
// ---------------------------------------------------------------------------

/// Écrit `src` vers `dest`, vérifie le hash, retry une fois en cas d'échec.
fn write_verify_retry(
    dest: &dyn Destination,
    rel_path: &Path,
    src: &Path,
    expected_hex: &str,
    hasher: &dyn ferr_hash::Hasher,
) -> DestResult {
    let dest_path = dest.root().join(rel_path);

    match attempt_write_verify(dest, rel_path, &dest_path, src, expected_hex, hasher) {
        Ok(()) => DestResult {
            path: dest_path,
            success: true,
            retried: false,
            error: None,
        },
        Err(first_err) => {
            // Retry unique
            match attempt_write_verify(dest, rel_path, &dest_path, src, expected_hex, hasher) {
                Ok(()) => DestResult {
                    path: dest_path,
                    success: true,
                    retried: true,
                    error: None,
                },
                Err(e) => DestResult {
                    path: dest_path,
                    success: false,
                    retried: true,
                    error: Some(format!("Première erreur: {first_err} | Retry: {e}")),
                },
            }
        }
    }
}

/// Écrit le fichier source vers la destination et vérifie l'intégrité par rehash.
/// Le hash source `expected_hex` est calculé une seule fois en amont (pas de double-hash).
fn attempt_write_verify(
    dest: &dyn Destination,
    rel_path: &Path,
    dest_path: &Path,
    src: &Path,
    expected_hex: &str,
    hasher: &dyn ferr_hash::Hasher,
) -> anyhow::Result<()> {
    dest.write_file(rel_path, src)?;
    sync_file(dest_path)?;

    // Vérification par relecture streaming (pas de tampon complet en RAM)
    let dest_hash = hasher.hash_file(dest_path)?;
    if dest_hash.hex != expected_hex {
        anyhow::bail!(
            "Hash mismatch sur {} : attendu {} obtenu {}",
            dest_path.display(),
            expected_hex,
            dest_hash.hex
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Préservation des métadonnées
// ---------------------------------------------------------------------------

/// Copie les timestamps (mtime) et les xattrs (macOS) de `src` vers `dest`.
fn copy_metadata(src: &Path, src_meta: &std::fs::Metadata, dest: &Path) -> anyhow::Result<()> {
    use filetime::FileTime;

    let mtime = FileTime::from_last_modification_time(src_meta);
    filetime::set_file_mtime(dest, mtime)?;

    #[cfg(target_os = "macos")]
    copy_xattrs(src, dest);

    Ok(())
}

#[cfg(target_os = "macos")]
fn copy_xattrs(src: &Path, dest: &Path) {
    if let Ok(names) = xattr::list(src) {
        for name in names {
            if let Ok(Some(value)) = xattr::get(src, &name) {
                let _ = xattr::set(dest, &name, &value);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ferr_hash::{Hasher as _, XxHasher};

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ferr_transfer_{name}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn copy_simple_file() {
        let base = tmp_dir("simple");
        let src = base.join("source.bin");
        let dest_root = base.join("dest");
        std::fs::create_dir_all(&dest_root).unwrap();
        std::fs::write(&src, b"hello ferr copy test").unwrap();

        let destinations: Vec<Box<dyn Destination>> = vec![Box::new(LocalDest::new(
            dest_root.clone(),
        ))];
        let hasher = XxHasher;

        let result = copy_file(
            &src,
            Path::new("source.bin"),
            &destinations,
            &hasher,
            None,
            |_| {},
            false,
        )
        .unwrap();

        assert!(!result.skipped);
        assert!(result.dest_results[0].success);
        assert!(!result.dest_results[0].retried);

        let dest_data = std::fs::read(dest_root.join("source.bin")).unwrap();
        assert_eq!(dest_data, b"hello ferr copy test");

        // Hash source == hash dest
        let dest_hash = hasher
            .hash_reader(&mut std::io::Cursor::new(&dest_data))
            .unwrap();
        assert_eq!(result.src_hash.hex, dest_hash.hex);

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn copy_with_subdirectory() {
        let base = tmp_dir("subdir");
        let src_dir = base.join("src");
        let dest_root = base.join("dest");
        std::fs::create_dir_all(src_dir.join("day1")).unwrap();
        std::fs::create_dir_all(&dest_root).unwrap();

        let src_file = src_dir.join("day1").join("clip.mov");
        std::fs::write(&src_file, b"video data").unwrap();

        let destinations: Vec<Box<dyn Destination>> = vec![Box::new(LocalDest::new(
            dest_root.clone(),
        ))];
        let hasher = XxHasher;

        let result = copy_file(
            &src_file,
            Path::new("day1/clip.mov"),
            &destinations,
            &hasher,
            None,
            |_| {},
            false,
        )
        .unwrap();

        assert!(result.dest_results[0].success);
        assert!(dest_root.join("day1").join("clip.mov").exists());

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn resume_skips_already_copied() {
        let base = tmp_dir("resume");
        let src = base.join("file.bin");
        let dest_root = base.join("dest");
        std::fs::create_dir_all(&dest_root).unwrap();
        std::fs::write(&src, b"content").unwrap();

        let hasher = XxHasher;
        let hash_result = hasher
            .hash_reader(&mut std::io::Cursor::new(b"content"))
            .unwrap();

        let manifest = ferr_report::Manifest {
            ferr_version: "0.1.0".into(),
            generated_at: "2025-01-01T00:00:00Z".into(),
            hostname: "host".into(),
            source_path: "/source".into(),
            destinations: Vec::new(),
            total_files: 1,
            total_size_bytes: 7,
            duration_secs: 0.1,
            status: ferr_report::JobStatus::Ok,
            files: vec![ferr_report::FileEntry {
                path: "file.bin".into(),
                size: 7,
                hash_algo: "xxhash64".into(),
                hash: hash_result.hex.clone(),
                modified_at: "2025-01-01T00:00:00Z".into(),
                status: ferr_report::FileStatus::Ok,
                par2_generated: false,
            }],
        };

        let destinations: Vec<Box<dyn Destination>> = vec![Box::new(LocalDest::new(
            dest_root.clone(),
        ))];

        let result = copy_file(
            &src,
            Path::new("file.bin"),
            &destinations,
            &hasher,
            Some(&manifest),
            |_| {},
            false,
        )
        .unwrap();

        assert!(result.skipped);
        assert_eq!(result.src_hash.hex, hash_result.hex);

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn copy_multiple_destinations() {
        let base = tmp_dir("multi");
        let src = base.join("file.bin");
        let dest1 = base.join("dest1");
        let dest2 = base.join("dest2");
        std::fs::create_dir_all(&dest1).unwrap();
        std::fs::create_dir_all(&dest2).unwrap();
        std::fs::write(&src, b"multi destination data").unwrap();

        let destinations: Vec<Box<dyn Destination>> = vec![
            Box::new(LocalDest::new(dest1.clone())),
            Box::new(LocalDest::new(dest2.clone())),
        ];
        let hasher = XxHasher;

        let result = copy_file(
            &src,
            Path::new("file.bin"),
            &destinations,
            &hasher,
            None,
            |_| {},
            false,
        )
        .unwrap();

        assert_eq!(result.dest_results.len(), 2);
        assert!(result.dest_results.iter().all(|r| r.success));
        assert!(dest1.join("file.bin").exists());
        assert!(dest2.join("file.bin").exists());

        std::fs::remove_dir_all(&base).ok();
    }
}
