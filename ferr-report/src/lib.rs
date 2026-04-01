//! Structures de données du rapport de copie ferr (manifest).
//!
//! Définit [`Manifest`], [`FileEntry`], [`JobStatus`] et [`FileStatus`],
//! ainsi que la sérialisation JSON et les exports ALE/CSV.

use std::path::Path;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Ok,
    Partial,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatus {
    Ok,
    Missing,
    Corrupted,
    Skipped,
}

// ---------------------------------------------------------------------------
// Structures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub hash_algo: String,
    pub hash: String,
    pub modified_at: String,
    pub status: FileStatus,
    pub par2_generated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub ferr_version: String,
    pub generated_at: String,
    pub hostname: String,
    pub source_path: String,
    #[serde(default)]
    pub destinations: Vec<String>,
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub duration_secs: f64,
    pub status: JobStatus,
    pub files: Vec<FileEntry>,
}

// ---------------------------------------------------------------------------
// Fonctions publiques
// ---------------------------------------------------------------------------

/// Formate une taille en octets en chaîne lisible (Go, Mo, Ko, o).
pub fn human_size(bytes: u64) -> String {
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

pub fn save_manifest(m: &Manifest, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(m)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load_manifest(path: &Path) -> anyhow::Result<Manifest> {
    let data = std::fs::read_to_string(path)?;
    let manifest = serde_json::from_str(&data)?;
    Ok(manifest)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> Manifest {
        Manifest {
            ferr_version: "0.1.0".into(),
            generated_at: "2025-01-01T00:00:00Z".into(),
            hostname: "test-host".into(),
            source_path: "/source".into(),
            destinations: vec!["/src".to_string()],
            total_files: 2,
            total_size_bytes: 1024,
            duration_secs: 1.5,
            status: JobStatus::Ok,
            files: vec![
                FileEntry {
                    path: "file_a.mov".into(),
                    size: 512,
                    hash_algo: "xxhash64".into(),
                    hash: "abcdef1234567890".into(),
                    modified_at: "2025-01-01T00:00:00Z".into(),
                    status: FileStatus::Ok,
                    par2_generated: false,
                },
                FileEntry {
                    path: "subdir/file_b.mov".into(),
                    size: 512,
                    hash_algo: "sha256".into(),
                    hash: "deadbeef00112233445566778899aabbccddeeff00112233445566778899aabb".into(),
                    modified_at: "2025-01-01T00:00:00Z".into(),
                    status: FileStatus::Ok,
                    par2_generated: true,
                },
            ],
        }
    }

    #[test]
    fn serialization_round_trip() {
        let m = sample_manifest();
        let json = serde_json::to_string(&m).unwrap();
        let m2: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(m.ferr_version, m2.ferr_version);
        assert_eq!(m.total_files, m2.total_files);
        assert_eq!(m.status, m2.status);
        assert_eq!(m.files.len(), m2.files.len());
        assert_eq!(m.files[0].path, m2.files[0].path);
        assert_eq!(m.files[1].status, m2.files[1].status);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join("ferr_report_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("manifest.json");

        let m = sample_manifest();
        save_manifest(&m, &path).unwrap();
        let loaded = load_manifest(&path).unwrap();

        assert_eq!(m.ferr_version, loaded.ferr_version);
        assert_eq!(m.hostname, loaded.hostname);
        assert_eq!(m.total_size_bytes, loaded.total_size_bytes);
        assert_eq!(m.files.len(), loaded.files.len());
        assert_eq!(m.files[0].hash, loaded.files[0].hash);
        assert_eq!(m.files[1].par2_generated, loaded.files[1].par2_generated);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn file_status_serialization() {
        let statuses = [
            FileStatus::Ok,
            FileStatus::Missing,
            FileStatus::Corrupted,
            FileStatus::Skipped,
        ];
        for status in &statuses {
            let json = serde_json::to_string(status).unwrap();
            let back: FileStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, &back);
        }
    }

    #[test]
    fn job_status_serialization() {
        for status in &[JobStatus::Ok, JobStatus::Partial, JobStatus::Failed] {
            let json = serde_json::to_string(status).unwrap();
            let back: JobStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, &back);
        }
    }
}

// ---------------------------------------------------------------------------
// Export ALE (Avid Log Exchange)
// ---------------------------------------------------------------------------

/// Exporte un manifest au format ALE (Avid Log Exchange).
pub fn export_ale(manifest: &Manifest, path: &Path) -> anyhow::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;

    // En-tête ALE
    writeln!(f, "Heading")?;
    writeln!(f, "FIELD_DELIM\tTABS")?;
    writeln!(f, "VIDEO_FORMAT\t1080")?;
    writeln!(f, "AUDIO_FORMAT\t48khz")?;
    writeln!(f, "FPS\t25")?;
    writeln!(f)?;
    writeln!(f, "Column")?;
    writeln!(
        f,
        "Name\tTape\tStart\tEnd\tFPS\tReel\tCamera\tResolution\tferr_hash\tferr_status"
    )?;
    writeln!(f)?;
    writeln!(f, "Data")?;

    for entry in &manifest.files {
        let name = std::path::Path::new(&entry.path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| entry.path.clone());
        let tape = manifest.source_path.clone();
        let start = "00:00:00:00";
        let end = "00:00:00:00";
        let fps = "25";
        let reel = "";
        let cam = "";
        let res = "";
        let hash = &entry.hash;
        let status = format!("{:?}", entry.status);

        writeln!(
            f,
            "{name}\t{tape}\t{start}\t{end}\t{fps}\t{reel}\t{cam}\t{res}\t{hash}\t{status}"
        )?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Export CSV
// ---------------------------------------------------------------------------

/// Exporte un manifest au format CSV.
pub fn export_csv(manifest: &Manifest, path: &Path) -> anyhow::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;

    writeln!(
        f,
        "path,size_bytes,hash_algo,hash,modified_at,status,par2_generated"
    )?;
    for entry in &manifest.files {
        writeln!(
            f,
            "{},{},{},{},{},{:?},{}",
            csv_escape(&entry.path),
            entry.size,
            csv_escape(&entry.hash_algo),
            csv_escape(&entry.hash),
            csv_escape(&entry.modified_at),
            entry.status,
            entry.par2_generated,
        )?;
    }
    Ok(())
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod export_tests {
    use super::*;

    fn sample() -> Manifest {
        Manifest {
            ferr_version: "0.1.0".into(),
            generated_at: "2025-01-01T00:00:00Z".into(),
            hostname: "host".into(),
            source_path: "/src".into(),
            destinations: Vec::new(),
            total_files: 1,
            total_size_bytes: 100,
            duration_secs: 1.0,
            status: JobStatus::Ok,
            files: vec![FileEntry {
                path: "clip.mov".into(),
                size: 100,
                hash_algo: "xxhash64".into(),
                hash: "abcdef1234567890".into(),
                modified_at: "2025-01-01T00:00:00Z".into(),
                status: FileStatus::Ok,
                par2_generated: false,
            }],
        }
    }

    #[test]
    fn ale_export_produces_file() {
        let dir = std::env::temp_dir().join("ferr_ale_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.ale");
        export_ale(&sample(), &path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Heading"));
        assert!(content.contains("ferr_hash"));
        assert!(content.contains("clip"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn csv_export_produces_file() {
        let dir = std::env::temp_dir().join("ferr_csv_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.csv");
        export_csv(&sample(), &path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("path,size_bytes"));
        assert!(content.contains("clip.mov"));
        assert!(content.contains("abcdef1234567890"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
