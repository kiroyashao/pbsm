use crate::modules::memory::error::{MemoryError, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug)]
pub struct SnapshotRow {
    pub snapshot_id: String,
    pub session_id: String,
    pub created_at: i64,
    pub snapshot_type: String,
    pub file_path: String,
    pub file_size: i64,
    pub checksum: String,
    pub node_count: Option<i32>,
    pub edge_count: Option<i32>,
    pub parent_snapshot_id: Option<String>,
    pub trigger_description: Option<String>,
}

#[derive(Debug)]
pub struct MemoryIndexRow {
    pub entry_id: String,
    pub layer: String,
    pub topic: String,
    pub confidence: Option<f64>,
    pub created_at: i64,
    pub last_accessed: Option<i64>,
    pub access_count: i32,
}

#[derive(Debug)]
pub struct ExperienceRow {
    pub experience_id: String,
    pub domain: String,
    pub pattern_type: String,
    pub confidence: f64,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub verification_count: i32,
    pub content_json: String,
}

pub struct SqliteStorage {
    conn: Arc<Mutex<Connection>>,
}

fn map_snapshot_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SnapshotRow> {
    Ok(SnapshotRow {
        snapshot_id: row.get(0)?,
        session_id: row.get(1)?,
        created_at: row.get(2)?,
        snapshot_type: row.get(3)?,
        file_path: row.get(4)?,
        file_size: row.get(5)?,
        checksum: row.get(6)?,
        node_count: row.get(7)?,
        edge_count: row.get(8)?,
        parent_snapshot_id: row.get(9)?,
        trigger_description: row.get(10)?,
    })
}

fn map_memory_index_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryIndexRow> {
    Ok(MemoryIndexRow {
        entry_id: row.get(0)?,
        layer: row.get(1)?,
        topic: row.get(2)?,
        confidence: row.get(3)?,
        created_at: row.get(4)?,
        last_accessed: row.get(5)?,
        access_count: row.get(6)?,
    })
}

fn map_experience_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExperienceRow> {
    Ok(ExperienceRow {
        experience_id: row.get(0)?,
        domain: row.get(1)?,
        pattern_type: row.get(2)?,
        confidence: row.get(3)?,
        created_at: row.get(4)?,
        last_used_at: row.get(5)?,
        verification_count: row.get(6)?,
        content_json: row.get(7)?,
    })
}

impl SqliteStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| MemoryError::StorageOpen(e.to_string()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| MemoryError::StorageConfig(e.to_string()))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn =
            Connection::open_in_memory().map_err(|e| MemoryError::StorageOpen(e.to_string()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| MemoryError::StorageConfig(e.to_string()))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS snapshots (
                snapshot_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                snapshot_type TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_size INTEGER NOT NULL,
                checksum TEXT NOT NULL,
                node_count INTEGER,
                edge_count INTEGER,
                parent_snapshot_id TEXT,
                trigger_description TEXT
            );

            CREATE TABLE IF NOT EXISTS memory_index (
                entry_id TEXT PRIMARY KEY,
                layer TEXT NOT NULL,
                topic TEXT NOT NULL,
                confidence REAL,
                created_at INTEGER NOT NULL,
                last_accessed INTEGER,
                access_count INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS experiences (
                experience_id TEXT PRIMARY KEY,
                domain TEXT NOT NULL,
                pattern_type TEXT NOT NULL,
                confidence REAL NOT NULL,
                created_at INTEGER NOT NULL,
                last_used_at INTEGER,
                verification_count INTEGER DEFAULT 0,
                content_json TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_session_time ON snapshots(session_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_snap_type ON snapshots(snapshot_type);
            CREATE INDEX IF NOT EXISTS idx_memory_topic ON memory_index(topic);
            CREATE INDEX IF NOT EXISTS idx_exp_domain ON experiences(domain);
            CREATE INDEX IF NOT EXISTS idx_exp_pattern ON experiences(pattern_type);",
        )
        .map_err(|e| MemoryError::SchemaInit(e.to_string()))?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_snapshot_meta(
        &self,
        snapshot_id: &str,
        session_id: &str,
        created_at: i64,
        snapshot_type: &str,
        file_path: &str,
        file_size: i64,
        checksum: &str,
        node_count: i32,
        edge_count: i32,
        trigger_description: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO snapshots (snapshot_id, session_id, created_at, snapshot_type, file_path, file_size, checksum, node_count, edge_count, parent_snapshot_id, trigger_description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10)",
            params![snapshot_id, session_id, created_at, snapshot_type, file_path, file_size, checksum, node_count, edge_count, trigger_description],
        )
        .map_err(|e| MemoryError::WriteFailed(e.to_string()))?;
        Ok(())
    }

    pub fn get_snapshot_meta(&self, snapshot_id: &str) -> Result<Option<SnapshotRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT snapshot_id, session_id, created_at, snapshot_type, file_path, file_size, checksum, node_count, edge_count, parent_snapshot_id, trigger_description FROM snapshots WHERE snapshot_id = ?1")
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
        let row = stmt
            .query_row(params![snapshot_id], map_snapshot_row)
            .optional()
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
        Ok(row)
    }

    pub fn query_snapshots_by_session(&self, session_id: &str) -> Result<Vec<SnapshotRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT snapshot_id, session_id, created_at, snapshot_type, file_path, file_size, checksum, node_count, edge_count, parent_snapshot_id, trigger_description FROM snapshots WHERE session_id = ?1 ORDER BY created_at DESC")
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
        let rows = stmt
            .query_map(params![session_id], map_snapshot_row)
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| MemoryError::ReadFailed(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn query_snapshots_by_type(&self, snapshot_type: &str) -> Result<Vec<SnapshotRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT snapshot_id, session_id, created_at, snapshot_type, file_path, file_size, checksum, node_count, edge_count, parent_snapshot_id, trigger_description FROM snapshots WHERE snapshot_type = ?1 ORDER BY created_at DESC")
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
        let rows = stmt
            .query_map(params![snapshot_type], map_snapshot_row)
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| MemoryError::ReadFailed(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn delete_snapshot(&self, snapshot_id: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn
            .execute(
                "DELETE FROM snapshots WHERE snapshot_id = ?1",
                params![snapshot_id],
            )
            .map_err(|e| MemoryError::WriteFailed(e.to_string()))?;
        Ok(affected > 0)
    }

    pub fn insert_memory_index(
        &self,
        entry_id: &str,
        layer: &str,
        topic: &str,
        confidence: f64,
        created_at: i64,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO memory_index (entry_id, layer, topic, confidence, created_at, last_accessed, access_count)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0)",
            params![entry_id, layer, topic, confidence, created_at],
        )
        .map_err(|e| MemoryError::WriteFailed(e.to_string()))?;
        Ok(())
    }

    pub fn query_memory_index_by_topic(
        &self,
        topic: &str,
        confidence_threshold: Option<f64>,
        limit: usize,
    ) -> Result<Vec<MemoryIndexRow>> {
        let conn = self.conn.lock();
        let sql = match confidence_threshold {
            Some(_) => "SELECT entry_id, layer, topic, confidence, created_at, last_accessed, access_count FROM memory_index WHERE topic = ?1 AND confidence >= ?2 ORDER BY confidence DESC LIMIT ?3",
            None => "SELECT entry_id, layer, topic, confidence, created_at, last_accessed, access_count FROM memory_index WHERE topic = ?1 ORDER BY confidence DESC LIMIT ?2",
        };
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
        let rows = match confidence_threshold {
            Some(threshold) => stmt.query_map(
                params![topic, threshold, limit as i64],
                map_memory_index_row,
            ),
            None => stmt.query_map(params![topic, limit as i64], map_memory_index_row),
        }
        .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| MemoryError::ReadFailed(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn update_memory_access(&self, entry_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE memory_index SET last_accessed = (strftime('%s','now') * 1000), access_count = access_count + 1 WHERE entry_id = ?1",
            params![entry_id],
        )
        .map_err(|e| MemoryError::WriteFailed(e.to_string()))?;
        Ok(())
    }

    pub fn delete_memory_index(&self, entry_id: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn
            .execute(
                "DELETE FROM memory_index WHERE entry_id = ?1",
                params![entry_id],
            )
            .map_err(|e| MemoryError::WriteFailed(e.to_string()))?;
        Ok(affected > 0)
    }

    pub fn insert_experience(
        &self,
        experience_id: &str,
        domain: &str,
        pattern_type: &str,
        confidence: f64,
        created_at: i64,
        content_json: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO experiences (experience_id, domain, pattern_type, confidence, created_at, last_used_at, verification_count, content_json)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, 0, ?6)",
            params![experience_id, domain, pattern_type, confidence, created_at, content_json],
        )
        .map_err(|e| MemoryError::WriteFailed(e.to_string()))?;
        Ok(())
    }

    pub fn query_experiences_by_domain(
        &self,
        domain: &str,
        confidence_threshold: Option<f64>,
    ) -> Result<Vec<ExperienceRow>> {
        let conn = self.conn.lock();
        let sql = match confidence_threshold {
            Some(_) => "SELECT experience_id, domain, pattern_type, confidence, created_at, last_used_at, verification_count, content_json FROM experiences WHERE domain = ?1 AND confidence >= ?2 ORDER BY confidence DESC",
            None => "SELECT experience_id, domain, pattern_type, confidence, created_at, last_used_at, verification_count, content_json FROM experiences WHERE domain = ?1 ORDER BY confidence DESC",
        };
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
        let rows = match confidence_threshold {
            Some(threshold) => stmt.query_map(params![domain, threshold], map_experience_row),
            None => stmt.query_map(params![domain], map_experience_row),
        }
        .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| MemoryError::ReadFailed(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn query_experiences_by_pattern(&self, pattern_type: &str) -> Result<Vec<ExperienceRow>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT experience_id, domain, pattern_type, confidence, created_at, last_used_at, verification_count, content_json FROM experiences WHERE pattern_type = ?1 ORDER BY confidence DESC")
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
        let rows = stmt
            .query_map(params![pattern_type], map_experience_row)
            .map_err(|e| MemoryError::ReadFailed(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| MemoryError::ReadFailed(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn update_experience_usage(&self, experience_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE experiences SET last_used_at = (strftime('%s','now') * 1000), verification_count = verification_count + 1 WHERE experience_id = ?1",
            params![experience_id],
        )
        .map_err(|e| MemoryError::WriteFailed(e.to_string()))?;
        Ok(())
    }

    pub fn delete_experience(&self, experience_id: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn
            .execute(
                "DELETE FROM experiences WHERE experience_id = ?1",
                params![experience_id],
            )
            .map_err(|e| MemoryError::WriteFailed(e.to_string()))?;
        Ok(affected > 0)
    }

    pub fn get_connection(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> SqliteStorage {
        let db = SqliteStorage::open_in_memory().unwrap();
        db.init_schema().unwrap();
        db
    }

    #[test]
    fn test_open_in_memory() {
        let db = SqliteStorage::open_in_memory();
        assert!(db.is_ok());
    }

    #[test]
    fn test_init_schema() {
        let db = SqliteStorage::open_in_memory().unwrap();
        assert!(db.init_schema().is_ok());
        assert!(db.init_schema().is_ok());
    }

    #[test]
    fn test_insert_and_get_snapshot() {
        let db = setup();
        db.insert_snapshot_meta(
            "snap-001",
            "sess-001",
            1700000000,
            "belief_state",
            "/data/snap-001.bin",
            4096,
            "abc123",
            10,
            5,
            "manual trigger",
        )
        .unwrap();

        let row = db.get_snapshot_meta("snap-001").unwrap();
        assert!(row.is_some());
        let row = row.unwrap();
        assert_eq!(row.snapshot_id, "snap-001");
        assert_eq!(row.session_id, "sess-001");
        assert_eq!(row.created_at, 1700000000);
        assert_eq!(row.snapshot_type, "belief_state");
        assert_eq!(row.file_path, "/data/snap-001.bin");
        assert_eq!(row.file_size, 4096);
        assert_eq!(row.checksum, "abc123");
        assert_eq!(row.node_count, Some(10));
        assert_eq!(row.edge_count, Some(5));
        assert_eq!(row.parent_snapshot_id, None);
        assert_eq!(row.trigger_description, Some("manual trigger".to_string()));
    }

    #[test]
    fn test_get_snapshot_not_found() {
        let db = setup();
        let row = db.get_snapshot_meta("nonexistent").unwrap();
        assert!(row.is_none());
    }

    #[test]
    fn test_query_snapshots_by_session() {
        let db = setup();
        db.insert_snapshot_meta(
            "snap-001",
            "sess-001",
            1700000000,
            "belief_state",
            "/a",
            100,
            "c1",
            5,
            2,
            "t1",
        )
        .unwrap();
        db.insert_snapshot_meta(
            "snap-002",
            "sess-001",
            1700000001,
            "intent_state",
            "/b",
            200,
            "c2",
            8,
            3,
            "t2",
        )
        .unwrap();
        db.insert_snapshot_meta(
            "snap-003",
            "sess-002",
            1700000002,
            "belief_state",
            "/c",
            300,
            "c3",
            1,
            0,
            "t3",
        )
        .unwrap();

        let rows = db.query_snapshots_by_session("sess-001").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].snapshot_id, "snap-002");
        assert_eq!(rows[1].snapshot_id, "snap-001");

        let rows = db.query_snapshots_by_session("sess-002").unwrap();
        assert_eq!(rows.len(), 1);

        let rows = db.query_snapshots_by_session("sess-999").unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_query_snapshots_by_type() {
        let db = setup();
        db.insert_snapshot_meta(
            "snap-001",
            "sess-001",
            1700000000,
            "belief_state",
            "/a",
            100,
            "c1",
            5,
            2,
            "t1",
        )
        .unwrap();
        db.insert_snapshot_meta(
            "snap-002",
            "sess-001",
            1700000001,
            "intent_state",
            "/b",
            200,
            "c2",
            8,
            3,
            "t2",
        )
        .unwrap();
        db.insert_snapshot_meta(
            "snap-003",
            "sess-002",
            1700000002,
            "belief_state",
            "/c",
            300,
            "c3",
            1,
            0,
            "t3",
        )
        .unwrap();

        let rows = db.query_snapshots_by_type("belief_state").unwrap();
        assert_eq!(rows.len(), 2);

        let rows = db.query_snapshots_by_type("intent_state").unwrap();
        assert_eq!(rows.len(), 1);

        let rows = db.query_snapshots_by_type("unknown").unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_delete_snapshot() {
        let db = setup();
        db.insert_snapshot_meta(
            "snap-001",
            "sess-001",
            1700000000,
            "belief_state",
            "/a",
            100,
            "c1",
            5,
            2,
            "t1",
        )
        .unwrap();

        assert!(db.delete_snapshot("snap-001").unwrap());
        assert!(!db.delete_snapshot("snap-001").unwrap());
        assert!(db.get_snapshot_meta("snap-001").unwrap().is_none());
    }

    #[test]
    fn test_delete_snapshot_not_found() {
        let db = setup();
        assert!(!db.delete_snapshot("nonexistent").unwrap());
    }

    #[test]
    fn test_insert_and_query_memory_index() {
        let db = setup();
        db.insert_memory_index("entry-001", "SNAPSHOT", "topic-a", 0.85, 1700000000)
            .unwrap();
        db.insert_memory_index("entry-002", "SNAPSHOT", "topic-a", 0.6, 1700000001)
            .unwrap();
        db.insert_memory_index("entry-003", "EXPERIENCE", "topic-b", 0.9, 1700000002)
            .unwrap();

        let rows = db.query_memory_index_by_topic("topic-a", None, 10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].entry_id, "entry-001");
        assert_eq!(rows[0].confidence, Some(0.85));

        let rows = db
            .query_memory_index_by_topic("topic-a", Some(0.7), 10)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entry_id, "entry-001");

        let rows = db.query_memory_index_by_topic("topic-b", None, 10).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_query_memory_index_with_limit() {
        let db = setup();
        for i in 0..5 {
            db.insert_memory_index(
                &format!("entry-{i:03}"),
                "SNAPSHOT",
                "topic-x",
                0.5 + i as f64 * 0.1,
                1700000000 + i,
            )
            .unwrap();
        }

        let rows = db.query_memory_index_by_topic("topic-x", None, 3).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].entry_id, "entry-004");
    }

    #[test]
    fn test_update_memory_access() {
        let db = setup();
        db.insert_memory_index("entry-001", "SNAPSHOT", "topic-a", 0.85, 1700000000)
            .unwrap();

        let before = db.query_memory_index_by_topic("topic-a", None, 1).unwrap();
        assert_eq!(before[0].access_count, 0);
        assert!(before[0].last_accessed.is_none());

        db.update_memory_access("entry-001").unwrap();

        let after = db.query_memory_index_by_topic("topic-a", None, 1).unwrap();
        assert_eq!(after[0].access_count, 1);
        assert!(after[0].last_accessed.is_some());
    }

    #[test]
    fn test_delete_memory_index() {
        let db = setup();
        db.insert_memory_index("entry-001", "SNAPSHOT", "topic-a", 0.85, 1700000000)
            .unwrap();

        assert!(db.delete_memory_index("entry-001").unwrap());
        assert!(!db.delete_memory_index("entry-001").unwrap());

        let rows = db.query_memory_index_by_topic("topic-a", None, 10).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_insert_and_query_experience() {
        let db = setup();
        db.insert_experience(
            "exp-001",
            "navigation",
            "recovery",
            0.9,
            1700000000,
            r#"{"action":"retry"}"#,
        )
        .unwrap();
        db.insert_experience(
            "exp-002",
            "navigation",
            "optimization",
            0.7,
            1700000001,
            r#"{"action":"cache"}"#,
        )
        .unwrap();
        db.insert_experience(
            "exp-003",
            "planning",
            "recovery",
            0.8,
            1700000002,
            r#"{"action":"replan"}"#,
        )
        .unwrap();

        let rows = db.query_experiences_by_domain("navigation", None).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].experience_id, "exp-001");

        let rows = db
            .query_experiences_by_domain("navigation", Some(0.8))
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].experience_id, "exp-001");

        let rows = db.query_experiences_by_domain("planning", None).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn test_query_experiences_by_pattern() {
        let db = setup();
        db.insert_experience(
            "exp-001",
            "navigation",
            "recovery",
            0.9,
            1700000000,
            r#"{"a":1}"#,
        )
        .unwrap();
        db.insert_experience(
            "exp-002",
            "planning",
            "recovery",
            0.8,
            1700000001,
            r#"{"a":2}"#,
        )
        .unwrap();
        db.insert_experience(
            "exp-003",
            "navigation",
            "optimization",
            0.7,
            1700000002,
            r#"{"a":3}"#,
        )
        .unwrap();

        let rows = db.query_experiences_by_pattern("recovery").unwrap();
        assert_eq!(rows.len(), 2);

        let rows = db.query_experiences_by_pattern("optimization").unwrap();
        assert_eq!(rows.len(), 1);

        let rows = db.query_experiences_by_pattern("unknown").unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_update_experience_usage() {
        let db = setup();
        db.insert_experience("exp-001", "nav", "recovery", 0.9, 1700000000, "{}")
            .unwrap();

        let before = db.query_experiences_by_domain("nav", None).unwrap();
        assert_eq!(before[0].verification_count, 0);
        assert!(before[0].last_used_at.is_none());

        db.update_experience_usage("exp-001").unwrap();

        let after = db.query_experiences_by_domain("nav", None).unwrap();
        assert_eq!(after[0].verification_count, 1);
        assert!(after[0].last_used_at.is_some());
    }

    #[test]
    fn test_delete_experience() {
        let db = setup();
        db.insert_experience("exp-001", "nav", "recovery", 0.9, 1700000000, "{}")
            .unwrap();

        assert!(db.delete_experience("exp-001").unwrap());
        assert!(!db.delete_experience("exp-001").unwrap());

        let rows = db.query_experiences_by_domain("nav", None).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_delete_experience_not_found() {
        let db = setup();
        assert!(!db.delete_experience("nonexistent").unwrap());
    }

    #[test]
    fn test_get_connection() {
        let db = setup();
        let conn = db.get_connection();
        {
            let c = conn.lock();
            let result: i64 = c.query_row("SELECT 1", [], |row| row.get(0)).unwrap();
            assert_eq!(result, 1);
        }
    }

    #[test]
    fn test_snapshot_row_debug() {
        let row = SnapshotRow {
            snapshot_id: "snap-001".to_string(),
            session_id: "sess-001".to_string(),
            created_at: 1700000000,
            snapshot_type: "belief_state".to_string(),
            file_path: "/a".to_string(),
            file_size: 100,
            checksum: "abc".to_string(),
            node_count: Some(5),
            edge_count: Some(2),
            parent_snapshot_id: None,
            trigger_description: Some("test".to_string()),
        };
        let debug_str = format!("{row:?}");
        assert!(debug_str.contains("snap-001"));
    }

    #[test]
    fn test_memory_index_row_debug() {
        let row = MemoryIndexRow {
            entry_id: "entry-001".to_string(),
            layer: "SNAPSHOT".to_string(),
            topic: "topic-a".to_string(),
            confidence: Some(0.85),
            created_at: 1700000000,
            last_accessed: None,
            access_count: 0,
        };
        let debug_str = format!("{row:?}");
        assert!(debug_str.contains("entry-001"));
    }

    #[test]
    fn test_experience_row_debug() {
        let row = ExperienceRow {
            experience_id: "exp-001".to_string(),
            domain: "navigation".to_string(),
            pattern_type: "recovery".to_string(),
            confidence: 0.9,
            created_at: 1700000000,
            last_used_at: None,
            verification_count: 0,
            content_json: "{}".to_string(),
        };
        let debug_str = format!("{row:?}");
        assert!(debug_str.contains("exp-001"));
    }

    #[test]
    fn test_multiple_access_updates() {
        let db = setup();
        db.insert_memory_index("entry-001", "SNAPSHOT", "topic-a", 0.85, 1700000000)
            .unwrap();

        for _ in 0..5 {
            db.update_memory_access("entry-001").unwrap();
        }

        let rows = db.query_memory_index_by_topic("topic-a", None, 1).unwrap();
        assert_eq!(rows[0].access_count, 5);
    }

    #[test]
    fn test_multiple_experience_usage_updates() {
        let db = setup();
        db.insert_experience("exp-001", "nav", "recovery", 0.9, 1700000000, "{}")
            .unwrap();

        for _ in 0..3 {
            db.update_experience_usage("exp-001").unwrap();
        }

        let rows = db.query_experiences_by_domain("nav", None).unwrap();
        assert_eq!(rows[0].verification_count, 3);
    }

    #[test]
    fn test_confidence_threshold_filtering() {
        let db = setup();
        db.insert_memory_index("e1", "SNAPSHOT", "t1", 0.3, 1700000000)
            .unwrap();
        db.insert_memory_index("e2", "SNAPSHOT", "t1", 0.5, 1700000001)
            .unwrap();
        db.insert_memory_index("e3", "SNAPSHOT", "t1", 0.8, 1700000002)
            .unwrap();
        db.insert_memory_index("e4", "SNAPSHOT", "t1", 0.95, 1700000003)
            .unwrap();

        let rows = db.query_memory_index_by_topic("t1", Some(0.5), 10).unwrap();
        assert_eq!(rows.len(), 3);

        let rows = db.query_memory_index_by_topic("t1", Some(0.9), 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].entry_id, "e4");

        let rows = db.query_memory_index_by_topic("t1", None, 10).unwrap();
        assert_eq!(rows.len(), 4);
    }

    #[test]
    fn test_experience_confidence_threshold() {
        let db = setup();
        db.insert_experience("exp-1", "domain-a", "pat", 0.3, 1700000000, "{}")
            .unwrap();
        db.insert_experience("exp-2", "domain-a", "pat", 0.7, 1700000001, "{}")
            .unwrap();
        db.insert_experience("exp-3", "domain-a", "pat", 0.95, 1700000002, "{}")
            .unwrap();

        let rows = db
            .query_experiences_by_domain("domain-a", Some(0.7))
            .unwrap();
        assert_eq!(rows.len(), 2);

        let rows = db
            .query_experiences_by_domain("domain-a", Some(0.99))
            .unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn test_snapshot_nullable_fields() {
        let db = setup();
        db.insert_snapshot_meta(
            "snap-001",
            "sess-001",
            1700000000,
            "belief_state",
            "/a",
            100,
            "c1",
            0,
            0,
            "",
        )
        .unwrap();

        let row = db.get_snapshot_meta("snap-001").unwrap().unwrap();
        assert_eq!(row.node_count, Some(0));
        assert_eq!(row.edge_count, Some(0));
        assert_eq!(row.trigger_description, Some("".to_string()));
    }

    #[test]
    fn test_open_invalid_path() {
        let result = SqliteStorage::open("/nonexistent/deeply/nested/path/db.sqlite");
        assert!(result.is_err());
    }
}
