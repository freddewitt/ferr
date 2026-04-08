//! Certificat d'intégrité Ferr (.ferrcert).
//!
//! Un certificat JSON auto-signé qui documente l'état exact d'un dossier
//! (arborescence + hashes) et enregistre chaque événement ultérieur
//! (copie, vérification). Le champ `cert_hash` est un SHA-256 de l'ensemble
//! du document (avec `cert_hash = "PLACEHOLDER"`) : toute modification du
//! fichier est détectable mathématiquement.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use ferr_hash::HashAlgo;

const CERT_VERSION: &str = "1.0";
const PLACEHOLDER: &str = "PLACEHOLDER";

// ---------------------------------------------------------------------------
// Erreurs
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum CertError {
    #[error("Certificate integrity check failed — file may have been tampered with")]
    IntegrityError,
    #[error("Malformed certificate: {0}")]
    MalformedFormat(String),
    #[error("Source is not a directory: {0}")]
    NotADirectory(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Structures principales
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FerrCert {
    pub ferr_cert: String,
    pub id: String,
    pub source: CertSource,
    pub hash_algo: String,
    pub summary: CertSummary,
    pub tree: Vec<TreeEntry>,
    pub events: Vec<CertEvent>,
    pub cert_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertSource {
    pub certified_at: String,
    pub certified_by: String,
    pub hostname: String,
    pub path: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertSummary {
    pub total_dirs: usize,
    pub total_files: usize,
    pub total_bytes: u64,
    pub empty_dirs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    #[serde(rename = "type")]
    pub kind: TreeEntryKind,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TreeEntryKind {
    Dir,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertEvent {
    #[serde(rename = "type")]
    pub kind: String,
    pub at: String,
    pub by: String,
    pub hostname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,
    pub result: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub issues: Vec<CertIssue>,
}

// ---------------------------------------------------------------------------
// Résultats de vérification
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct CertVerifyResult {
    pub result: CertResult,
    pub issues: Vec<CertIssue>,
    pub checked_files: usize,
    pub checked_dirs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertResult {
    Pass,
    PassWithMinor,
    Fail,
}

impl CertResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            CertResult::Pass => "PASS",
            CertResult::PassWithMinor => "PASS_WITH_MINOR",
            CertResult::Fail => "FAIL",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertIssue {
    pub severity: IssueSeverity,
    #[serde(rename = "type")]
    pub kind: IssueKind,
    pub path: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IssueSeverity {
    Critical,
    Minor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    MissingFile,
    CorruptedFile,
    SizeMismatch,
    MissingDir,
    UnexpectedFile,
    UnexpectedDir,
    MissingEmptyDir,
}

// ---------------------------------------------------------------------------
// API publique
// ---------------------------------------------------------------------------

/// Certifie un dossier : scanne l'arborescence, hash les fichiers, écrit le
/// .ferrcert signé.
///
/// Si `output` est `None`, le fichier est créé dans `src` sous le nom
/// `{label}_{YYYYMMDD}_{HHMMSS}.ferrcert`.
///
/// Retourne le chemin du fichier créé.
pub fn cert_create(
    src: &Path,
    output: Option<&Path>,
    hash_algo: HashAlgo,
) -> Result<PathBuf> {
    if !src.is_dir() {
        return Err(CertError::NotADirectory(src.display().to_string()).into());
    }

    let now = Utc::now();
    let label = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string());

    let date_str = now.format("%Y%m%d").to_string();
    let time_str = now.format("%H%M%S").to_string();
    let certified_at = now.to_rfc3339();

    // Identifiant reproductible mais unique : hash de (chemin + timestamp)
    let id_input = format!("{}{}", src.display(), certified_at);
    let id_hash = Sha256::digest(id_input.as_bytes());
    let id_short: String = id_hash.iter().take(4).map(|b| format!("{b:02x}")).collect();
    let id = format!("fcert_{date_str}_{time_str}_{id_short}");

    let (tree, summary) = build_tree(src, &hash_algo)
        .with_context(|| format!("Failed to scan {}", src.display()))?;

    let hostname = get_hostname();
    let by = format!("ferr {}", env!("CARGO_PKG_VERSION"));

    let init_event = CertEvent {
        kind: "certified".to_string(),
        at: certified_at.clone(),
        by: by.clone(),
        hostname: hostname.clone(),
        path: Some(src.to_string_lossy().into_owned()),
        role: Some("source".to_string()),
        source: None,
        dest: None,
        result: "PASS".to_string(),
        detail: format!(
            "Initial certification — {} files, {} dirs ({} empty)",
            summary.total_files, summary.total_dirs, summary.empty_dirs
        ),
        issues: Vec::new(),
    };

    let mut cert = FerrCert {
        ferr_cert: CERT_VERSION.to_string(),
        id,
        source: CertSource {
            certified_at,
            certified_by: by,
            hostname,
            path: src.to_string_lossy().into_owned(),
            label: label.clone(),
        },
        hash_algo: hash_algo.to_string(),
        summary,
        tree,
        events: vec![init_event],
        cert_hash: PLACEHOLDER.to_string(),
    };

    sign_cert(&mut cert)?;

    let out_path = match output {
        Some(p) => p.to_path_buf(),
        None => src.join(format!("{label}_{date_str}_{time_str}.ferrcert")),
    };

    let json = serde_json::to_string_pretty(&cert)
        .context("Failed to serialize certificate")?;
    std::fs::write(&out_path, &json)
        .with_context(|| format!("Cannot write certificate to {}", out_path.display()))?;

    Ok(out_path)
}

/// Vérifie un dossier `target` contre un .ferrcert.
///
/// Vérifie d'abord l'intégrité du cert lui-même (cert_hash), puis compare
/// l'arborescence et les hashes. Ajoute automatiquement un event "verified"
/// dans le cert après la vérification.
pub fn cert_verify(
    cert_path: &Path,
    target: &Path,
    _quiet: bool,
) -> Result<CertVerifyResult> {
    // 1. Intégrité du cert
    let cert = cert_load(cert_path)?;
    if !check_integrity_of(&cert)? {
        anyhow::bail!(
            "Certificate integrity check failed: the certificate has been tampered with or corrupted.\n\
             Run 'ferr cert check {}' for details.",
            cert_path.display()
        );
    }

    // 2. Construire les ensembles attendus depuis cert.tree
    let mut expected_files: HashMap<String, (u64, String)> = HashMap::new();
    let mut expected_dirs: HashSet<String> = HashSet::new();
    let mut expected_empty_dirs: HashSet<String> = HashSet::new();

    for entry in &cert.tree {
        match entry.kind {
            TreeEntryKind::File => {
                let size = entry.size.unwrap_or(0);
                let hash = entry.hash.clone().unwrap_or_default();
                expected_files.insert(entry.path.clone(), (size, hash));
            }
            TreeEntryKind::Dir => {
                let path = entry.path.trim_end_matches('/').to_string();
                if entry.empty == Some(true) {
                    expected_empty_dirs.insert(path.clone());
                }
                expected_dirs.insert(path);
            }
        }
    }

    // 3. Parcourir le target
    let hasher = make_hasher(&cert.hash_algo);
    let mut actual_files: HashMap<String, PathBuf> = HashMap::new();
    let mut actual_dirs: HashSet<String> = HashSet::new();
    collect_actual(target, target, &mut actual_files, &mut actual_dirs)?;

    // 4. Construire la liste des issues
    let mut issues: Vec<CertIssue> = Vec::new();

    // Fichiers certifiés absents ou altérés
    for (rel, (exp_size, exp_hash)) in &expected_files {
        match actual_files.get(rel.as_str()) {
            None => {
                issues.push(CertIssue {
                    severity: IssueSeverity::Critical,
                    kind: IssueKind::MissingFile,
                    path: rel.clone(),
                    detail: "File certified but absent from target".to_string(),
                });
            }
            Some(actual_path) => {
                let actual_size = std::fs::metadata(actual_path)
                    .map(|m| m.len())
                    .unwrap_or(0);

                if actual_size != *exp_size {
                    issues.push(CertIssue {
                        severity: IssueSeverity::Critical,
                        kind: IssueKind::SizeMismatch,
                        path: rel.clone(),
                        detail: format!(
                            "Expected {} bytes, found {}",
                            exp_size, actual_size
                        ),
                    });
                } else {
                    match hasher.hash_file(actual_path) {
                        Ok(h) if h.hex != *exp_hash => {
                            issues.push(CertIssue {
                                severity: IssueSeverity::Critical,
                                kind: IssueKind::CorruptedFile,
                                path: rel.clone(),
                                detail: format!(
                                    "Hash mismatch: expected {}…, got {}…",
                                    &exp_hash[..exp_hash.len().min(16)],
                                    &h.hex[..h.hex.len().min(16)],
                                ),
                            });
                        }
                        Err(e) => {
                            issues.push(CertIssue {
                                severity: IssueSeverity::Critical,
                                kind: IssueKind::CorruptedFile,
                                path: rel.clone(),
                                detail: format!("Cannot hash file: {e}"),
                            });
                        }
                        Ok(_) => {} // hash OK
                    }
                }
            }
        }
    }

    // Fichiers présents non certifiés
    for rel in actual_files.keys() {
        if !expected_files.contains_key(rel.as_str()) {
            issues.push(CertIssue {
                severity: IssueSeverity::Minor,
                kind: IssueKind::UnexpectedFile,
                path: rel.clone(),
                detail: "File present but not in certificate".to_string(),
            });
        }
    }

    // Dossiers vides certifiés absents (MINOR)
    for dir in &expected_empty_dirs {
        if !target.join(dir).exists() {
            issues.push(CertIssue {
                severity: IssueSeverity::Minor,
                kind: IssueKind::MissingEmptyDir,
                path: dir.clone(),
                detail: "Empty directory certified but absent from target".to_string(),
            });
        }
    }

    // Dossiers non-vides certifiés absents (CRITICAL)
    for dir in &expected_dirs {
        if !expected_empty_dirs.contains(dir) && !target.join(dir).exists() {
            issues.push(CertIssue {
                severity: IssueSeverity::Critical,
                kind: IssueKind::MissingDir,
                path: dir.clone(),
                detail: "Directory certified but absent from target".to_string(),
            });
        }
    }

    // Dossiers présents non certifiés (MINOR)
    for dir in &actual_dirs {
        if !expected_dirs.contains(dir.as_str()) {
            issues.push(CertIssue {
                severity: IssueSeverity::Minor,
                kind: IssueKind::UnexpectedDir,
                path: dir.clone(),
                detail: "Directory present but not in certificate".to_string(),
            });
        }
    }

    // 5. Résultat
    let has_critical = issues.iter().any(|i| i.severity == IssueSeverity::Critical);
    let has_minor = issues.iter().any(|i| i.severity == IssueSeverity::Minor);

    let result = if has_critical {
        CertResult::Fail
    } else if has_minor {
        CertResult::PassWithMinor
    } else {
        CertResult::Pass
    };

    let checked_files = expected_files.len();
    let checked_dirs = expected_dirs.len();

    // 6. Ajouter event "verified" dans le cert
    let now = Utc::now();
    let event = CertEvent {
        kind: "verified".to_string(),
        at: now.to_rfc3339(),
        by: format!("ferr {}", env!("CARGO_PKG_VERSION")),
        hostname: get_hostname(),
        path: Some(target.to_string_lossy().into_owned()),
        role: None,
        source: None,
        dest: None,
        result: result.as_str().to_string(),
        detail: format!(
            "{} files checked, {} dirs checked, {} issue(s)",
            checked_files,
            checked_dirs,
            issues.len()
        ),
        issues: issues.clone(),
    };
    cert_append_event(cert_path, event)?;

    Ok(CertVerifyResult {
        result,
        issues,
        checked_files,
        checked_dirs,
    })
}

/// Vérifie uniquement l'intégrité du cert lui-même (cert_hash) sans
/// toucher aux fichiers.
///
/// Retourne `true` si le cert est intact, `false` s'il a été altéré.
pub fn cert_check_integrity(cert_path: &Path) -> Result<bool> {
    let cert = cert_load(cert_path)?;
    check_integrity_of(&cert)
}

/// Cherche le premier .ferrcert à la racine d'un dossier.
///
/// Retourne `None` si aucun n'est trouvé.
pub fn find_cert(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut found: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .map(|ext| ext.eq_ignore_ascii_case("ferrcert"))
                    .unwrap_or(false)
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

/// Charge et désérialise un .ferrcert existant.
pub fn cert_load(cert_path: &Path) -> Result<FerrCert> {
    let content = std::fs::read_to_string(cert_path)
        .with_context(|| format!("Cannot read certificate {}", cert_path.display()))?;
    let cert: FerrCert = serde_json::from_str(&content)
        .with_context(|| format!("Invalid certificate format in {}", cert_path.display()))?;
    Ok(cert)
}

/// Ajoute un événement dans un .ferrcert existant et recalcule le cert_hash.
///
/// Vérifie l'intégrité du cert avant de modifier.
pub fn cert_append_event(cert_path: &Path, event: CertEvent) -> Result<()> {
    let mut cert = cert_load(cert_path)?;

    if !check_integrity_of(&cert)? {
        anyhow::bail!(
            "Cannot append event: certificate integrity check failed for {}",
            cert_path.display()
        );
    }

    cert.events.push(event);
    sign_cert(&mut cert)?;

    let json = serde_json::to_string_pretty(&cert)
        .context("Failed to serialize updated certificate")?;
    std::fs::write(cert_path, json)
        .with_context(|| format!("Cannot write updated certificate to {}", cert_path.display()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Mécanisme d'auto-signature
// ---------------------------------------------------------------------------

/// Signe le cert : met cert_hash = SHA-256(compact_json avec PLACEHOLDER).
fn sign_cert(cert: &mut FerrCert) -> Result<()> {
    cert.cert_hash = PLACEHOLDER.to_string();
    let compact = serde_json::to_string(cert).context("Cannot serialize cert for signing")?;
    let hash_bytes = Sha256::digest(compact.as_bytes());
    cert.cert_hash = hex_encode(&hash_bytes);
    Ok(())
}

/// Vérifie l'intégrité d'un `FerrCert` déjà chargé en mémoire.
fn check_integrity_of(cert: &FerrCert) -> Result<bool> {
    let stored = cert.cert_hash.clone();
    let mut copy = cert.clone();
    copy.cert_hash = PLACEHOLDER.to_string();
    let compact = serde_json::to_string(&copy).context("Cannot serialize cert for verification")?;
    let hash_bytes = Sha256::digest(compact.as_bytes());
    Ok(hex_encode(&hash_bytes) == stored)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Construction de l'arbre
// ---------------------------------------------------------------------------

/// Scanne récursivement `root` et retourne l'arbre + le résumé.
///
/// Les fichiers .ferrcert et les éléments système sont ignorés.
fn build_tree(root: &Path, hash_algo: &HashAlgo) -> Result<(Vec<TreeEntry>, CertSummary)> {
    let hasher = make_hasher(&hash_algo.to_string());

    let mut raw_files: Vec<(PathBuf, u64, String)> = Vec::new(); // (abs, size, hash)
    let mut raw_dirs: Vec<PathBuf> = Vec::new(); // abs paths

    collect_all(root, root, &*hasher, &mut raw_files, &mut raw_dirs)?;

    let mut entries: Vec<TreeEntry> = Vec::new();
    let mut total_files = 0usize;
    let mut total_bytes = 0u64;
    let mut empty_dirs = 0usize;

    // Dirs
    for dir_abs in &raw_dirs {
        let rel = dir_abs
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        // Un dossier est "vide" si aucun fichier n'existe dans son sous-arbre
        let is_empty = !raw_files.iter().any(|(f, _, _)| f.starts_with(dir_abs));

        if is_empty {
            empty_dirs += 1;
        }

        entries.push(TreeEntry {
            kind: TreeEntryKind::Dir,
            path: format!("{rel}/"),
            empty: if is_empty { Some(true) } else { None },
            size: None,
            hash: None,
        });
    }

    // Files
    for (file_abs, size, hash) in &raw_files {
        let rel = file_abs
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        total_files += 1;
        total_bytes += size;

        entries.push(TreeEntry {
            kind: TreeEntryKind::File,
            path: rel,
            empty: None,
            size: Some(*size),
            hash: Some(hash.clone()),
        });
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));

    let summary = CertSummary {
        total_dirs: raw_dirs.len(),
        total_files,
        total_bytes,
        empty_dirs,
    };

    Ok((entries, summary))
}

/// Collecte récursivement tous les fichiers et dossiers sous `dir`,
/// en ignorant les éléments système et les .ferrcert.
fn collect_all(
    root: &Path,
    dir: &Path,
    hasher: &dyn ferr_hash::Hasher,
    raw_files: &mut Vec<(PathBuf, u64, String)>,
    raw_dirs: &mut Vec<PathBuf>,
) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("Cannot read directory {}", dir.display()))?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if should_skip_in_cert(&name_str) {
            continue;
        }

        if path.is_dir() {
            raw_dirs.push(path.clone());
            collect_all(root, &path, hasher, raw_files, raw_dirs)?;
        } else if path.is_file() {
            let h = hasher
                .hash_file(&path)
                .with_context(|| format!("Cannot hash {}", path.display()))?;
            raw_files.push((path, h.bytes_read, h.hex));
        }
    }

    Ok(())
}

/// Collecte fichiers et dossiers du `target` pour la vérification.
fn collect_actual(
    root: &Path,
    dir: &Path,
    actual_files: &mut HashMap<String, PathBuf>,
    actual_dirs: &mut HashSet<String>,
) -> Result<()> {
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("Cannot read directory {}", dir.display()))?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if should_skip_in_cert(&name_str) {
            continue;
        }

        let rel = path
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();

        if path.is_dir() {
            actual_dirs.insert(rel.clone());
            collect_actual(root, &path, actual_files, actual_dirs)?;
        } else if path.is_file() {
            actual_files.insert(rel, path);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Utilitaires internes
// ---------------------------------------------------------------------------

fn should_skip_in_cert(name: &str) -> bool {
    is_system_noise(name)
        || name == "ferr-manifest.json"
        || name == "_par2"
        || name.starts_with("_ferr_logs_")
        || name.ends_with(".ferrcert")
}

fn is_system_noise(name: &str) -> bool {
    matches!(
        name,
        ".DS_Store"
            | ".localized"
            | ".Spotlight-V100"
            | ".fseventsd"
            | ".Trashes"
            | ".TemporaryItems"
            | "Thumbs.db"
            | "desktop.ini"
            | ".AppleDouble"
            | ".AppleDB"
            | ".AppleDesktop"
    ) || name.starts_with("._")
}

fn make_hasher(algo: &str) -> Box<dyn ferr_hash::Hasher> {
    match algo {
        "sha256" => Box::new(ferr_hash::Sha256Hasher),
        _ => Box::new(ferr_hash::XxHasher),
    }
}

fn get_hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ferr_cert_{name}"));
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn make_test_tree(base: &Path) {
        fs::create_dir_all(base.join("subdir")).unwrap();
        fs::create_dir_all(base.join("empty_dir")).unwrap();
        fs::write(base.join("file_a.txt"), b"hello ferr").unwrap();
        fs::write(base.join("subdir").join("file_b.txt"), b"world").unwrap();
    }

    #[test]
    fn cert_create_and_load() {
        let base = tmp_dir("create_load");
        make_test_tree(&base);

        let cert_path = cert_create(&base, None, HashAlgo::XxHash64).unwrap();
        assert!(cert_path.exists());
        assert!(cert_path.extension().unwrap() == "ferrcert");

        let cert = cert_load(&cert_path).unwrap();
        assert_eq!(cert.ferr_cert, "1.0");
        assert_eq!(cert.summary.total_files, 2);
        assert!(cert.summary.total_dirs >= 2); // subdir + empty_dir
        assert_eq!(cert.summary.empty_dirs, 1); // only empty_dir
        assert_eq!(cert.events.len(), 1);
        assert_eq!(cert.events[0].kind, "certified");

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn cert_integrity_ok() {
        let base = tmp_dir("integrity_ok");
        make_test_tree(&base);
        let cert_path = cert_create(&base, None, HashAlgo::XxHash64).unwrap();
        assert!(cert_check_integrity(&cert_path).unwrap());
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn cert_integrity_tamper_detected() {
        let base = tmp_dir("integrity_tamper");
        make_test_tree(&base);
        let cert_path = cert_create(&base, None, HashAlgo::XxHash64).unwrap();

        // Tamper with the cert file
        let mut content = fs::read_to_string(&cert_path).unwrap();
        content = content.replace("hello ferr", "TAMPERED"); // change a hash value
        // Actually modify the cert_hash field directly to simulate tampering
        let cert = cert_load(&cert_path).unwrap();
        let mut tampered = cert.clone();
        tampered.events[0].detail = "TAMPERED".to_string();
        // Write without re-signing
        let json = serde_json::to_string_pretty(&tampered).unwrap();
        fs::write(&cert_path, json).unwrap();

        assert!(!cert_check_integrity(&cert_path).unwrap());
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn cert_verify_pass() {
        let src = tmp_dir("verify_pass_src");
        let dst = tmp_dir("verify_pass_dst");
        make_test_tree(&src);

        // Copy tree to dst (mimicking ferr copy)
        fs::create_dir_all(dst.join("subdir")).unwrap();
        fs::create_dir_all(dst.join("empty_dir")).unwrap();
        fs::copy(src.join("file_a.txt"), dst.join("file_a.txt")).unwrap();
        fs::copy(
            src.join("subdir").join("file_b.txt"),
            dst.join("subdir").join("file_b.txt"),
        )
        .unwrap();

        let cert_path = cert_create(&src, None, HashAlgo::XxHash64).unwrap();
        // Copy cert to dst
        let dst_cert = dst.join(cert_path.file_name().unwrap());
        fs::copy(&cert_path, &dst_cert).unwrap();

        let result = cert_verify(&dst_cert, &dst, false).unwrap();
        assert_eq!(result.result, CertResult::Pass);
        assert!(result.issues.is_empty());

        // Event was appended
        let cert = cert_load(&dst_cert).unwrap();
        assert_eq!(cert.events.len(), 2);
        assert_eq!(cert.events[1].kind, "verified");

        fs::remove_dir_all(&src).ok();
        fs::remove_dir_all(&dst).ok();
    }

    #[test]
    fn cert_verify_missing_file() {
        let src = tmp_dir("verify_missing_src");
        let dst = tmp_dir("verify_missing_dst");
        make_test_tree(&src);

        fs::create_dir_all(dst.join("subdir")).unwrap();
        // Intentionally omit file_b.txt
        fs::copy(src.join("file_a.txt"), dst.join("file_a.txt")).unwrap();

        let cert_path = cert_create(&src, None, HashAlgo::XxHash64).unwrap();
        let dst_cert = dst.join(cert_path.file_name().unwrap());
        fs::copy(&cert_path, &dst_cert).unwrap();

        let result = cert_verify(&dst_cert, &dst, false).unwrap();
        assert_eq!(result.result, CertResult::Fail);
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.kind == IssueKind::MissingFile)
        );

        fs::remove_dir_all(&src).ok();
        fs::remove_dir_all(&dst).ok();
    }

    #[test]
    fn cert_verify_corrupted_file() {
        let src = tmp_dir("verify_corrupt_src");
        let dst = tmp_dir("verify_corrupt_dst");
        make_test_tree(&src);

        fs::create_dir_all(dst.join("subdir")).unwrap();
        fs::copy(src.join("file_a.txt"), dst.join("file_a.txt")).unwrap();
        fs::copy(
            src.join("subdir").join("file_b.txt"),
            dst.join("subdir").join("file_b.txt"),
        )
        .unwrap();
        // Corrupt file_a in dst
        fs::write(dst.join("file_a.txt"), b"CORRUPTED").unwrap();

        let cert_path = cert_create(&src, None, HashAlgo::XxHash64).unwrap();
        let dst_cert = dst.join(cert_path.file_name().unwrap());
        fs::copy(&cert_path, &dst_cert).unwrap();

        let result = cert_verify(&dst_cert, &dst, false).unwrap();
        assert_eq!(result.result, CertResult::Fail);
        // SizeMismatch or CorruptedFile both indicate detected corruption
        assert!(
            result.issues.iter().any(|i| matches!(
                i.kind,
                IssueKind::CorruptedFile | IssueKind::SizeMismatch
            ))
        );

        fs::remove_dir_all(&src).ok();
        fs::remove_dir_all(&dst).ok();
    }

    #[test]
    fn find_cert_returns_ferrcert() {
        let base = tmp_dir("find_cert");
        make_test_tree(&base);
        let cert_path = cert_create(&base, None, HashAlgo::XxHash64).unwrap();

        let found = find_cert(&base);
        assert_eq!(found, Some(cert_path));

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn cert_skips_itself_in_tree() {
        let base = tmp_dir("skip_self");
        make_test_tree(&base);
        let cert_path = cert_create(&base, None, HashAlgo::XxHash64).unwrap();
        let cert = cert_load(&cert_path).unwrap();

        // The .ferrcert file must NOT appear in the tree
        let cert_name = cert_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            !cert.tree.iter().any(|e| e.path.ends_with(&cert_name)),
            ".ferrcert should not appear in the tree"
        );

        fs::remove_dir_all(&base).ok();
    }
}
