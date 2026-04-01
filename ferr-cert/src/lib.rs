//! Certificat d'intégrité Ferr (Ferr Certificate).
//!
//! Un certificat encapsule un [`Manifest`][ferr_report::Manifest] signé par un
//! checksum SHA-256 dans un enveloppe Base64 délimitée par des marqueurs PEM.
//!
//! # Limitation de sécurité
//!
//! La protection repose sur SHA-256 (symétrique) : elle garantit l'**intégrité**
//! du manifest (détection de toute altération) mais **pas l'authenticité** —
//! un attaquant ayant accès au fichier peut recalculer un checksum valide.
//! Pour une authenticité garantie, envisagez une signature asymétrique Ed25519.

use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use ferr_report::Manifest;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const CERT_HEADER: &str = "-----BEGIN FERR CERTIFICATE-----";
pub const CERT_FOOTER: &str = "-----END FERR CERTIFICATE-----";

#[derive(Error, Debug)]
pub enum CertError {
    #[error("Certificat malformé ou manquant l'en-tête/pied de page")]
    MalformedFormat,
    #[error("Erreur de décodage Base64: {0}")]
    Base64Error(#[from] base64::DecodeError),
    #[error("Erreur de parsing JSON: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Erreur d'intégrité (le contenu a été altéré)")]
    IntegrityError,
}

pub fn pack(manifest: &Manifest) -> Result<String> {
    let json_bytes = serde_json::to_vec(manifest)?;

    let mut hasher = Sha256::new();
    hasher.update(&json_bytes);
    let checksum = hasher.finalize();

    let mut payload = Vec::with_capacity(checksum.len() + json_bytes.len());
    payload.extend_from_slice(&checksum);
    payload.extend_from_slice(&json_bytes);

    let b64 = general_purpose::STANDARD.encode(payload);

    let mut lines = Vec::new();
    let mut current = 0;
    while current < b64.len() {
        let end = std::cmp::min(current + 64, b64.len());
        lines.push(&b64[current..end]);
        current = end;
    }

    let joined = lines.join("\n");
    Ok(format!("{}\n{}\n{}", CERT_HEADER, joined, CERT_FOOTER))
}

pub fn unpack(cert_data: &str) -> Result<Manifest, CertError> {
    let lines: Vec<&str> = cert_data
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    let start_idx = lines
        .iter()
        .position(|&l| l == CERT_HEADER)
        .ok_or(CertError::MalformedFormat)?;
    let end_idx = lines
        .iter()
        .position(|&l| l == CERT_FOOTER)
        .ok_or(CertError::MalformedFormat)?;

    if start_idx >= end_idx {
        return Err(CertError::MalformedFormat);
    }

    let b64_lines = &lines[start_idx + 1..end_idx];
    let b64_str = b64_lines.join("");

    let payload = general_purpose::STANDARD.decode(b64_str)?;

    const SHA256_LEN: usize = 32; // SHA-256 produit toujours 32 octets
    if payload.len() < SHA256_LEN {
        return Err(CertError::MalformedFormat);
    }

    let (expected_checksum, json_bytes) = payload.split_at(SHA256_LEN);

    let mut hasher = Sha256::new();
    hasher.update(json_bytes);
    let actual_checksum = hasher.finalize();

    if expected_checksum != actual_checksum.as_slice() {
        return Err(CertError::IntegrityError);
    }

    let manifest: Manifest = serde_json::from_slice(json_bytes)?;

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> ferr_report::Manifest {
        ferr_report::Manifest {
            ferr_version: "1.0".into(),
            generated_at: "2026-01-01T00:00:00Z".into(),
            hostname: "test-host".into(),
            source_path: "/test/path".into(),
            destinations: Vec::new(),
            total_files: 1,
            total_size_bytes: 100,
            duration_secs: 1.0,
            status: ferr_report::JobStatus::Ok,
            files: vec![ferr_report::FileEntry {
                path: "test.txt".into(),
                size: 100,
                hash_algo: "xxhash64".into(),
                hash: "dummyhash".into(),
                modified_at: "2026-01-01T00:00:00Z".into(),
                status: ferr_report::FileStatus::Ok,
                par2_generated: false,
            }],
        }
    }

    #[test]
    fn test_pack_unpack() {
        let manifest = sample_manifest();
        let packed = pack(&manifest).unwrap();

        assert!(packed.starts_with(CERT_HEADER));
        assert!(packed.ends_with(CERT_FOOTER));

        let unpacked = unpack(&packed).unwrap();
        assert_eq!(manifest.source_path, unpacked.source_path);
        assert_eq!(manifest.files.len(), unpacked.files.len());
    }

    #[test]
    fn test_tampering() {
        let manifest = sample_manifest();
        let mut packed = pack(&manifest).unwrap();

        let b64_index = packed.find("\n").unwrap() + 1;
        let c = packed.chars().nth(b64_index).unwrap();
        let alt_c = if c == 'A' { 'B' } else { 'A' };
        packed.replace_range(b64_index..b64_index + 1, &alt_c.to_string());

        let result = unpack(&packed);
        assert!(matches!(
            result,
            Err(CertError::IntegrityError) | Err(CertError::Base64Error(_))
        ));
    }
}
