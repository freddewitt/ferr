use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use xxhash_rust::xxh64::Xxh64;

const CHUNK_SIZE: usize = 8 * 1024 * 1024; // 8 MiB

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashAlgo {
    XxHash64,
    Sha256,
}

impl std::fmt::Display for HashAlgo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashAlgo::XxHash64 => write!(f, "xxhash64"),
            HashAlgo::Sha256 => write!(f, "sha256"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HashResult {
    pub algo: HashAlgo,
    pub hex: String,
    pub bytes_read: u64,
}

// ---------------------------------------------------------------------------
// Trait Hasher
// ---------------------------------------------------------------------------

pub trait Hasher: Send + Sync {
    fn hash_file(&self, path: &Path) -> anyhow::Result<HashResult>;
    fn hash_reader(&self, r: &mut dyn Read) -> anyhow::Result<HashResult>;
}

// ---------------------------------------------------------------------------
// XxHasher
// ---------------------------------------------------------------------------

pub struct XxHasher;

impl Hasher for XxHasher {
    fn hash_file(&self, path: &Path) -> anyhow::Result<HashResult> {
        let mut file = File::open(path)?;
        self.hash_reader(&mut file)
    }

    fn hash_reader(&self, r: &mut dyn Read) -> anyhow::Result<HashResult> {
        let mut hasher = Xxh64::new(0);
        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut bytes_read = 0u64;
        loop {
            let n = r.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            bytes_read += n as u64;
        }
        Ok(HashResult {
            algo: HashAlgo::XxHash64,
            hex: format!("{:016x}", hasher.digest()),
            bytes_read,
        })
    }
}

// ---------------------------------------------------------------------------
// Sha256Hasher
// ---------------------------------------------------------------------------

pub struct Sha256Hasher;

impl Hasher for Sha256Hasher {
    fn hash_file(&self, path: &Path) -> anyhow::Result<HashResult> {
        let mut file = File::open(path)?;
        self.hash_reader(&mut file)
    }

    fn hash_reader(&self, r: &mut dyn Read) -> anyhow::Result<HashResult> {
        let mut hasher = sha2::Sha256::new();
        let mut buf = vec![0u8; CHUNK_SIZE];
        let mut bytes_read = 0u64;
        loop {
            let n = r.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            bytes_read += n as u64;
        }
        let result = hasher.finalize();
        let hex: String = result.iter().map(|b| format!("{:02x}", b)).collect();
        Ok(HashResult {
            algo: HashAlgo::Sha256,
            hex,
            bytes_read,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const KNOWN_DATA: &[u8] = b"hello world ferr test data 1234567890";

    // --- XxHasher ---

    #[test]
    fn xxhasher_known_data() {
        let hasher = XxHasher;
        let result = hasher.hash_reader(&mut Cursor::new(KNOWN_DATA)).unwrap();
        assert_eq!(result.algo, HashAlgo::XxHash64);
        assert_eq!(result.bytes_read, KNOWN_DATA.len() as u64);
        assert_eq!(result.hex.len(), 16); // 64-bit = 16 hex chars
    }

    #[test]
    fn xxhasher_empty_reader() {
        let hasher = XxHasher;
        let result = hasher.hash_reader(&mut Cursor::new(b"")).unwrap();
        assert_eq!(result.bytes_read, 0);
        assert_eq!(result.hex.len(), 16);
    }

    #[test]
    fn xxhasher_idempotent() {
        let hasher = XxHasher;
        let a = hasher.hash_reader(&mut Cursor::new(KNOWN_DATA)).unwrap();
        let b = hasher.hash_reader(&mut Cursor::new(KNOWN_DATA)).unwrap();
        assert_eq!(a.hex, b.hex);
    }

    #[test]
    fn xxhasher_file() {
        let dir = std::env::temp_dir().join("ferr_hash_test_xx");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.bin");
        std::fs::write(&path, KNOWN_DATA).unwrap();

        let hasher = XxHasher;
        let result = hasher.hash_file(&path).unwrap();
        assert_eq!(result.bytes_read, KNOWN_DATA.len() as u64);

        // Must match hash_reader on same data
        let expected = hasher.hash_reader(&mut Cursor::new(KNOWN_DATA)).unwrap();
        assert_eq!(result.hex, expected.hex);

        std::fs::remove_dir_all(&dir).ok();
    }

    // --- Sha256Hasher ---

    #[test]
    fn sha256hasher_known_data() {
        let hasher = Sha256Hasher;
        let result = hasher.hash_reader(&mut Cursor::new(KNOWN_DATA)).unwrap();
        assert_eq!(result.algo, HashAlgo::Sha256);
        assert_eq!(result.bytes_read, KNOWN_DATA.len() as u64);
        assert_eq!(result.hex.len(), 64); // 256-bit = 64 hex chars
    }

    #[test]
    fn sha256hasher_empty_reader() {
        let hasher = Sha256Hasher;
        let result = hasher.hash_reader(&mut Cursor::new(b"")).unwrap();
        assert_eq!(result.bytes_read, 0);
        assert_eq!(result.hex.len(), 64);
        // SHA-256 of empty string is well-known
        assert_eq!(
            result.hex,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256hasher_idempotent() {
        let hasher = Sha256Hasher;
        let a = hasher.hash_reader(&mut Cursor::new(KNOWN_DATA)).unwrap();
        let b = hasher.hash_reader(&mut Cursor::new(KNOWN_DATA)).unwrap();
        assert_eq!(a.hex, b.hex);
    }

    #[test]
    fn sha256hasher_file() {
        let dir = std::env::temp_dir().join("ferr_hash_test_sha");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.bin");
        std::fs::write(&path, KNOWN_DATA).unwrap();

        let hasher = Sha256Hasher;
        let result = hasher.hash_file(&path).unwrap();
        let expected = hasher.hash_reader(&mut Cursor::new(KNOWN_DATA)).unwrap();
        assert_eq!(result.hex, expected.hex);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn different_data_different_hash() {
        let hasher = XxHasher;
        let a = hasher.hash_reader(&mut Cursor::new(b"data_a")).unwrap();
        let b = hasher.hash_reader(&mut Cursor::new(b"data_b")).unwrap();
        assert_ne!(a.hex, b.hex);
    }
}
