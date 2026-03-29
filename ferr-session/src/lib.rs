use std::path::PathBuf;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

pub type SessionId = i64;

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub date: String,
    pub source: String,
    pub destinations: Vec<String>,
    pub total_files: usize,
    pub total_bytes: u64,
    pub duration_secs: f64,
    pub status: String,
    pub manifest_path: Option<String>,
    pub hash_algo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: i64,
    pub session_id: SessionId,
    pub path: String,
    pub size: u64,
    pub hash: String,
    pub status: String,
}

#[derive(Debug, Default)]
pub struct SessionFilter {
    pub since: Option<String>,
    pub limit: Option<usize>,
    pub source: Option<String>,
}

// ---------------------------------------------------------------------------
// Chemins de données
// ---------------------------------------------------------------------------

pub fn db_path() -> anyhow::Result<PathBuf> {
    if let Ok(d) = std::env::var("FERR_DATA_DIR") {
        return Ok(PathBuf::from(d).join("history.db"));
    }
    #[cfg(unix)]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        Ok(PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("ferr")
            .join("history.db"))
    }
    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA")
            .unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Roaming".to_string());
        Ok(PathBuf::from(appdata).join("ferr").join("history.db"))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Ok(PathBuf::from("/tmp/ferr_history.db"))
    }
}

fn open_db() -> anyhow::Result<Connection> {
    let path = db_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)?;
    init_schema(&conn)?;
    Ok(conn)
}

// ---------------------------------------------------------------------------
// Initialisation du schéma
// ---------------------------------------------------------------------------

pub fn init_db() -> anyhow::Result<()> {
    let conn = open_db()?;
    init_schema(&conn)?;
    Ok(())
}

fn init_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            date          TEXT NOT NULL,
            source        TEXT NOT NULL,
            destinations  TEXT NOT NULL,
            total_files   INTEGER NOT NULL DEFAULT 0,
            total_bytes   INTEGER NOT NULL DEFAULT 0,
            duration_secs REAL    NOT NULL DEFAULT 0.0,
            status        TEXT    NOT NULL DEFAULT 'Unknown',
            manifest_path TEXT,
            hash_algo     TEXT    NOT NULL DEFAULT 'xxhash64'
        );

        CREATE TABLE IF NOT EXISTS files (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id INTEGER NOT NULL,
            path       TEXT    NOT NULL,
            size       INTEGER NOT NULL DEFAULT 0,
            hash       TEXT    NOT NULL DEFAULT '',
            status     TEXT    NOT NULL DEFAULT 'Unknown',
            FOREIGN KEY (session_id) REFERENCES sessions(id)
        );

        CREATE INDEX IF NOT EXISTS idx_files_hash ON files(hash);
        CREATE INDEX IF NOT EXISTS idx_sessions_source ON sessions(source);
        ",
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Enregistrement d'une session
// ---------------------------------------------------------------------------

pub fn record_session(manifest: &ferr_report::Manifest) -> anyhow::Result<SessionId> {
    let conn = open_db()?;
    let destinations_json = serde_json::to_string(&manifest.source_path)?;
    let status = format!("{:?}", manifest.status);

    conn.execute(
        "INSERT INTO sessions
            (date, source, destinations, total_files, total_bytes,
             duration_secs, status, hash_algo)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            manifest.generated_at,
            manifest.source_path,
            destinations_json,
            manifest.total_files as i64,
            manifest.total_size_bytes as i64,
            manifest.duration_secs,
            status,
            manifest
                .files
                .first()
                .map(|f| f.hash_algo.as_str())
                .unwrap_or("xxhash64"),
        ],
    )?;

    let session_id = conn.last_insert_rowid();

    // Enregistrer les fichiers
    let mut stmt = conn.prepare(
        "INSERT INTO files (session_id, path, size, hash, status)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for entry in &manifest.files {
        stmt.execute(params![
            session_id,
            entry.path,
            entry.size as i64,
            entry.hash,
            format!("{:?}", entry.status),
        ])?;
    }

    Ok(session_id)
}

// ---------------------------------------------------------------------------
// Lecture des sessions
// ---------------------------------------------------------------------------

pub fn list_sessions(filter: SessionFilter) -> anyhow::Result<Vec<Session>> {
    let conn = open_db()?;
    let limit = filter.limit.unwrap_or(100);

    let mut query = "SELECT id, date, source, destinations, total_files, total_bytes,
                            duration_secs, status, manifest_path, hash_algo
                     FROM sessions
                     WHERE 1=1"
        .to_string();

    if filter.since.is_some() {
        query.push_str(" AND date >= ?1");
    }
    if filter.source.is_some() {
        query.push_str(" AND source LIKE ?2");
    }
    query.push_str(" ORDER BY id DESC LIMIT ?3");

    let mut stmt = conn.prepare(&query)?;

    let rows = match (&filter.since, &filter.source) {
        (Some(since), Some(src)) => stmt.query_map(
            params![since, format!("%{src}%"), limit as i64],
            row_to_session,
        )?,
        (Some(since), None) => stmt.query_map(params![since, "", limit as i64], row_to_session)?,
        (None, Some(src)) => stmt.query_map(
            params!["", format!("%{src}%"), limit as i64],
            row_to_session,
        )?,
        (None, None) => stmt.query_map(params!["", "", limit as i64], row_to_session)?,
    };

    let sessions: Result<Vec<_>, _> = rows.collect();
    Ok(sessions?)
}

pub fn get_session(id: SessionId) -> anyhow::Result<Option<Session>> {
    let conn = open_db()?;
    let result = conn.query_row(
        "SELECT id, date, source, destinations, total_files, total_bytes,
                duration_secs, status, manifest_path, hash_algo
         FROM sessions WHERE id = ?1",
        params![id],
        row_to_session,
    );
    match result {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn find_file_by_hash(hash: &str) -> anyhow::Result<Vec<FileRecord>> {
    let conn = open_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, session_id, path, size, hash, status
         FROM files WHERE hash = ?1",
    )?;
    let rows = stmt.query_map(params![hash], |row| {
        Ok(FileRecord {
            id: row.get(0)?,
            session_id: row.get(1)?,
            path: row.get(2)?,
            size: row.get::<_, i64>(3)? as u64,
            hash: row.get(4)?,
            status: row.get(5)?,
        })
    })?;
    let records: Result<Vec<_>, _> = rows.collect();
    Ok(records?)
}

pub fn find_sessions_by_source(source: &str) -> anyhow::Result<Vec<Session>> {
    list_sessions(SessionFilter {
        source: Some(source.to_string()),
        ..Default::default()
    })
}

fn row_to_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    let dests_json: String = row.get(3)?;
    let destinations: Vec<String> =
        serde_json::from_str(&dests_json).unwrap_or_else(|_| vec![dests_json.clone()]);
    Ok(Session {
        id: row.get(0)?,
        date: row.get(1)?,
        source: row.get(2)?,
        destinations,
        total_files: row.get::<_, i64>(4)? as usize,
        total_bytes: row.get::<_, i64>(5)? as u64,
        duration_secs: row.get(6)?,
        status: row.get(7)?,
        manifest_path: row.get(8)?,
        hash_algo: row.get(9)?,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifest() -> ferr_report::Manifest {
        ferr_report::Manifest {
            ferr_version: "0.1.0".into(),
            generated_at: "2025-01-01T00:00:00Z".into(),
            hostname: "host".into(),
            source_path: "/footage/A001".into(),
            total_files: 1,
            total_size_bytes: 1024,
            duration_secs: 0.5,
            status: ferr_report::JobStatus::Ok,
            files: vec![ferr_report::FileEntry {
                path: "A001_C001.braw".into(),
                size: 1024,
                hash_algo: "xxhash64".into(),
                hash: "abcdef1234567890".into(),
                modified_at: "2025-01-01T00:00:00Z".into(),
                status: ferr_report::FileStatus::Ok,
                par2_generated: false,
            }],
        }
    }

    fn set_test_db() {
        // Forcer un DB temporaire pour les tests
        let tmp = std::env::temp_dir().join(format!("ferr_session_test_{}.db", std::process::id()));
        std::env::set_var("FERR_DATA_DIR", tmp.parent().unwrap());
    }

    #[test]
    fn record_and_list() {
        set_test_db();
        let manifest = test_manifest();
        let id = record_session(&manifest).unwrap();
        assert!(id > 0);

        let sessions = list_sessions(SessionFilter::default()).unwrap();
        assert!(!sessions.is_empty());
        let found = sessions.iter().any(|s| s.id == id);
        assert!(found);
    }

    #[test]
    fn find_file_by_hash_works() {
        set_test_db();
        let manifest = test_manifest();
        record_session(&manifest).unwrap();
        let records = find_file_by_hash("abcdef1234567890").unwrap();
        assert!(!records.is_empty());
        assert_eq!(records[0].hash, "abcdef1234567890");
    }

    #[test]
    fn get_session_returns_none_for_unknown() {
        set_test_db();
        let _ = init_db();
        let result = get_session(999999).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn find_by_source() {
        set_test_db();
        let manifest = test_manifest();
        record_session(&manifest).unwrap();
        let sessions = find_sessions_by_source("/footage/A001").unwrap();
        assert!(!sessions.is_empty());
    }
}
