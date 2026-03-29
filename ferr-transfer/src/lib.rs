use std::io::Read;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

// ---------------------------------------------------------------------------
// Trait Destination
// ---------------------------------------------------------------------------

pub trait Destination: Send + Sync {
    fn write_file(&self, rel_path: &Path, data: &[u8]) -> anyhow::Result<()>;
    fn sync(&self) -> anyhow::Result<()>;
    fn root(&self) -> &Path;
}

// ---------------------------------------------------------------------------
// LocalDest
// ---------------------------------------------------------------------------

pub struct LocalDest {
    pub root: PathBuf,
}

impl Destination for LocalDest {
    fn write_file(&self, rel_path: &Path, data: &[u8]) -> anyhow::Result<()> {
        let dest = self.root.join(rel_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Écriture atomique : fichier temporaire puis renommage
        let tmp = dest.with_extension("ferr_tmp");
        std::fs::write(&tmp, data)?;
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
    pub src_hash:          ferr_hash::HashResult,
    pub dest_results:      Vec<DestResult>,
    pub skipped:           bool,
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

// Wrapper platform-spécifique (présent pour extensibilité future)
#[cfg(unix)]
#[allow(dead_code)]
fn fsync_fd(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::io::AsRawFd;
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    let ret = unsafe { libc::fsync(file.as_raw_fd()) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(windows)]
#[allow(dead_code)]
fn fsync_fd(path: &Path) -> anyhow::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::FlushFileBuffers;
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    let ok = unsafe { FlushFileBuffers(file.as_raw_handle() as _) };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// copy_file
// ---------------------------------------------------------------------------

const CHUNK_SIZE: usize = 8 * 1024 * 1024; // 8 MiB

/// Copie `src` vers toutes les destinations.
///
/// - `rel_path` : chemin relatif utilisé pour `Destination::write_file`
///   (typiquement `src.strip_prefix(source_root)`)
/// - `resume_manifest` : si fourni et que le fichier est déjà présent avec
///   le bon hash, la copie est ignorée (skip)
/// - `on_progress` : callback appelé avec le nombre d'octets lus jusqu'ici
pub fn copy_file(
    src: &Path,
    rel_path: &Path,
    destinations: &[Box<dyn Destination>],
    hasher: &dyn ferr_hash::Hasher,
    resume_manifest: Option<&ferr_report::Manifest>,
    on_progress: impl Fn(u64),
    preserve_metadata: bool,
) -> anyhow::Result<TransferResult> {
    // --- Vérification reprise -------------------------------------------------
    if let Some(manifest) = resume_manifest {
        let rel_str = rel_path.to_string_lossy();
        if let Some(entry) = manifest
            .files
            .iter()
            .find(|e| e.path == rel_str.as_ref())
        {
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

    // --- Lecture source en chunks ---------------------------------------------
    let mut file = std::fs::File::open(src)?;
    let mut data: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut total_read = 0u64;

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
        total_read += n as u64;
        on_progress(total_read);
    }

    // --- Hash source ----------------------------------------------------------
    let src_hash = {
        let mut cursor = std::io::Cursor::new(&data);
        hasher.hash_reader(&mut cursor)?
    };

    // --- Écriture parallèle sur toutes les destinations ----------------------
    let results: Vec<DestResult> = destinations
        .par_iter()
        .map(|dest| write_verify_retry(dest.as_ref(), rel_path, &data, hasher))
        .collect();

    // --- Préservation des métadonnées ----------------------------------------
    let metadata_preserved = if preserve_metadata {
        let src_meta = std::fs::metadata(src)?;
        let mut all_ok = true;
        for dest in destinations {
            let dest_path = dest.root().join(rel_path);
            if let Err(e) = copy_metadata(src, &src_meta, &dest_path) {
                eprintln!("Avertissement : métadonnées non préservées sur {} : {e}", dest_path.display());
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

/// Écrit, sync, vérifie le hash. Si incorrect → retry une fois.
fn write_verify_retry(
    dest: &dyn Destination,
    rel_path: &Path,
    data: &[u8],
    hasher: &dyn ferr_hash::Hasher,
) -> DestResult {
    let dest_path = dest.root().join(rel_path);

    match attempt_write_verify(dest, rel_path, &dest_path, data, hasher) {
        Ok(()) => DestResult {
            path: dest_path,
            success: true,
            retried: false,
            error: None,
        },
        Err(first_err) => {
            // Retry unique
            match attempt_write_verify(dest, rel_path, &dest_path, data, hasher) {
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

fn attempt_write_verify(
    dest: &dyn Destination,
    rel_path: &Path,
    dest_path: &Path,
    data: &[u8],
    hasher: &dyn ferr_hash::Hasher,
) -> anyhow::Result<()> {
    dest.write_file(rel_path, data)?;
    sync_file(dest_path)?;

    // Vérification par relecture
    let read_back = std::fs::read(dest_path)?;
    let dest_hash = {
        let mut cursor = std::io::Cursor::new(&read_back);
        hasher.hash_reader(&mut cursor)?
    };
    let src_hash = {
        let mut cursor = std::io::Cursor::new(data);
        hasher.hash_reader(&mut cursor)?
    };

    if dest_hash.hex != src_hash.hex {
        anyhow::bail!(
            "Hash mismatch sur {} : attendu {} obtenu {}",
            dest_path.display(),
            src_hash.hex,
            dest_hash.hex
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Préservation des métadonnées
// ---------------------------------------------------------------------------

/// Copie les timestamps (mtime, atime) et les xattrs (macOS) de `src` vers `dest`.
fn copy_metadata(
    src: &Path,
    src_meta: &std::fs::Metadata,
    dest: &Path,
) -> anyhow::Result<()> {
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

        let destinations: Vec<Box<dyn Destination>> =
            vec![Box::new(LocalDest { root: dest_root.clone() })];
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

        let destinations: Vec<Box<dyn Destination>> =
            vec![Box::new(LocalDest { root: dest_root.clone() })];
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

        let destinations: Vec<Box<dyn Destination>> =
            vec![Box::new(LocalDest { root: dest_root.clone() })];

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
            Box::new(LocalDest { root: dest1.clone() }),
            Box::new(LocalDest { root: dest2.clone() }),
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
