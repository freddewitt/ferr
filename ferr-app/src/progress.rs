#![allow(dead_code)]
// progress.rs — parse machine-readable ferr progress lines
//
// ferr outputs structured lines when --progress-format machine is passed:
//   PROGRESS:<bytes>/<total>|<files>/<total>|<speed>|<filename>
//   COMPLETE:<files>|<bytes>|<errors>|<manifest_path>
//   ERROR:<filename>|<type>|<details>
//   SCAN_PROGRESS:<scanned>/<total>|<file>
//   SCAN_RESULT:OK|<checked>|<corrupted>
//   VERIFY_RESULT:OK|<matched>|<mismatched>|<missing>
//   WATCH_DETECTED:<volume>|<mount>
//   WATCH_COPY_START:<volume>|<dest>

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProgressEvent {
    Progress {
        bytes: u64,
        total_bytes: u64,
        files: u64,
        total_files: u64,
        speed: String,
        filename: String,
    },
    Complete {
        files: u64,
        bytes: u64,
        errors: u64,
        manifest_path: String,
    },
    Error {
        filename: String,
        kind: String,
        details: String,
    },
    ScanProgress {
        scanned: u64,
        total: u64,
        file: String,
    },
    ScanResult {
        ok: bool,
        checked: u64,
        corrupted: u64,
    },
    VerifyResult {
        ok: bool,
        matched: u64,
        mismatched: u64,
        missing: u64,
    },
    WatchDetected {
        volume: String,
        mount: String,
    },
    WatchCopyStart {
        volume: String,
        dest: String,
    },
    Unknown {
        raw: String,
    },
}

pub fn parse(line: &str) -> ProgressEvent {
    if let Some(rest) = line.strip_prefix("PROGRESS:") {
        let parts: Vec<&str> = rest.splitn(4, '|').collect();
        if parts.len() == 4 {
            let (bytes, total_bytes) = split_slash(parts[0]);
            let (files, total_files) = split_slash(parts[1]);
            return ProgressEvent::Progress {
                bytes,
                total_bytes,
                files,
                total_files,
                speed: parts[2].to_string(),
                filename: parts[3].to_string(),
            };
        }
    }

    if let Some(rest) = line.strip_prefix("COMPLETE:") {
        let p: Vec<&str> = rest.splitn(4, '|').collect();
        if p.len() == 4 {
            return ProgressEvent::Complete {
                files: p[0].parse().unwrap_or(0),
                bytes: p[1].parse().unwrap_or(0),
                errors: p[2].parse().unwrap_or(0),
                manifest_path: p[3].to_string(),
            };
        }
    }

    if let Some(rest) = line.strip_prefix("ERROR:") {
        let p: Vec<&str> = rest.splitn(3, '|').collect();
        if p.len() == 3 {
            return ProgressEvent::Error {
                filename: p[0].to_string(),
                kind: p[1].to_string(),
                details: p[2].to_string(),
            };
        }
    }

    if let Some(rest) = line.strip_prefix("SCAN_PROGRESS:") {
        let p: Vec<&str> = rest.splitn(2, '|').collect();
        if p.len() == 2 {
            let (scanned, total) = split_slash(p[0]);
            return ProgressEvent::ScanProgress {
                scanned,
                total,
                file: p[1].to_string(),
            };
        }
    }

    if let Some(rest) = line.strip_prefix("SCAN_RESULT:") {
        let p: Vec<&str> = rest.splitn(3, '|').collect();
        if p.len() == 3 {
            return ProgressEvent::ScanResult {
                ok: p[0] == "OK",
                checked: p[1].parse().unwrap_or(0),
                corrupted: p[2].parse().unwrap_or(0),
            };
        }
    }

    if let Some(rest) = line.strip_prefix("VERIFY_RESULT:") {
        let p: Vec<&str> = rest.splitn(4, '|').collect();
        if p.len() == 4 {
            return ProgressEvent::VerifyResult {
                ok: p[0] == "OK",
                matched: p[1].parse().unwrap_or(0),
                mismatched: p[2].parse().unwrap_or(0),
                missing: p[3].parse().unwrap_or(0),
            };
        }
    }

    if let Some(rest) = line.strip_prefix("WATCH_DETECTED:") {
        let p: Vec<&str> = rest.splitn(2, '|').collect();
        if p.len() == 2 {
            return ProgressEvent::WatchDetected {
                volume: p[0].to_string(),
                mount: p[1].to_string(),
            };
        }
    }

    if let Some(rest) = line.strip_prefix("WATCH_COPY_START:") {
        let p: Vec<&str> = rest.splitn(2, '|').collect();
        if p.len() == 2 {
            return ProgressEvent::WatchCopyStart {
                volume: p[0].to_string(),
                dest: p[1].to_string(),
            };
        }
    }

    ProgressEvent::Unknown { raw: line.to_string() }
}

fn split_slash(s: &str) -> (u64, u64) {
    let mut it = s.splitn(2, '/');
    let a = it.next().unwrap_or("0").parse().unwrap_or(0);
    let b = it.next().unwrap_or("0").parse().unwrap_or(0);
    (a, b)
}
