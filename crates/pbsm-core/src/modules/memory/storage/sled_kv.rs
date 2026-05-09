use sled::{Db, Tree};
use std::path::Path;
use std::time::Duration;

use crate::modules::memory::error::{MemoryError, Result};

fn map_sled_error(context: &str, e: sled::Error) -> MemoryError {
    match e {
        sled::Error::Io(_) => MemoryError::StorageOpen(format!("{context}: {e}")),
        sled::Error::Corruption { .. } => MemoryError::ReadFailed(format!("{context}: {e}")),
        sled::Error::ReportableBug(_) => MemoryError::ReadFailed(format!("{context}: {e}")),
        sled::Error::Unsupported(_) => MemoryError::StorageConfig(format!("{context}: {e}")),
        _ => MemoryError::StorageOpen(format!("{context}: {e}")),
    }
}

#[derive(Clone, Debug)]
pub struct SledConfig {
    pub cache_capacity_bytes: usize,
    pub segment_size_bytes: usize,
    pub flush_interval: Duration,
}

impl Default for SledConfig {
    fn default() -> Self {
        Self {
            cache_capacity_bytes: 128 * 1024 * 1024,
            segment_size_bytes: 8 * 1024 * 1024,
            flush_interval: Duration::from_secs(5 * 60),
        }
    }
}

#[allow(dead_code)]
pub struct SledKvStore {
    db: Db,
    snapshots_tree: Tree,
    raw_logs_tree: Tree,
    experiences_tree: Tree,
    config: SledConfig,
}

impl SledKvStore {
    pub fn open(path: impl AsRef<Path>, config: SledConfig) -> Result<Self> {
        let db = sled::Config::default()
            .path(path)
            .cache_capacity(config.cache_capacity_bytes as u64)
            .segment_size(config.segment_size_bytes)
            .flush_every_ms(Some(config.flush_interval.as_millis() as u64))
            .open()
            .map_err(|e| MemoryError::StorageOpen(format!("failed to open sled database: {e}")))?;

        let snapshots_tree = db
            .open_tree("snapshots")
            .map_err(|e| MemoryError::TreeOpen(format!("failed to open snapshots tree: {e}")))?;

        let raw_logs_tree = db
            .open_tree("raw_logs")
            .map_err(|e| MemoryError::TreeOpen(format!("failed to open raw_logs tree: {e}")))?;

        let experiences_tree = db
            .open_tree("experiences")
            .map_err(|e| MemoryError::TreeOpen(format!("failed to open experiences tree: {e}")))?;

        Ok(Self {
            db,
            snapshots_tree,
            raw_logs_tree,
            experiences_tree,
            config,
        })
    }

    pub fn open_temp() -> Result<Self> {
        let db = sled::Config::new().temporary(true).open().map_err(|e| {
            MemoryError::StorageOpen(format!("failed to open temporary sled database: {e}"))
        })?;

        let snapshots_tree = db
            .open_tree("snapshots")
            .map_err(|e| MemoryError::TreeOpen(format!("failed to open snapshots tree: {e}")))?;

        let raw_logs_tree = db
            .open_tree("raw_logs")
            .map_err(|e| MemoryError::TreeOpen(format!("failed to open raw_logs tree: {e}")))?;

        let experiences_tree = db
            .open_tree("experiences")
            .map_err(|e| MemoryError::TreeOpen(format!("failed to open experiences tree: {e}")))?;

        Ok(Self {
            db,
            snapshots_tree,
            raw_logs_tree,
            experiences_tree,
            config: SledConfig::default(),
        })
    }

    pub fn insert_snapshot(&self, snapshot_id: &str, data: &[u8]) -> Result<()> {
        let key = format!("snap:{snapshot_id}");
        self.snapshots_tree
            .insert(key.as_bytes(), data)
            .map_err(|e| map_sled_error("insert_snapshot", e))?;
        Ok(())
    }

    pub fn get_snapshot(&self, snapshot_id: &str) -> Result<Option<Vec<u8>>> {
        let key = format!("snap:{snapshot_id}");
        self.snapshots_tree
            .get(key.as_bytes())
            .map(|opt| opt.map(|ivec| ivec.to_vec()))
            .map_err(|e| map_sled_error("get_snapshot", e))
    }

    pub fn remove_snapshot(&self, snapshot_id: &str) -> Result<bool> {
        let key = format!("snap:{snapshot_id}");
        self.snapshots_tree
            .remove(key.as_bytes())
            .map(|opt| opt.is_some())
            .map_err(|e| map_sled_error("remove_snapshot", e))
    }

    pub fn insert_raw_log(&self, session_id: &str, log_id: &str, data: &[u8]) -> Result<()> {
        let key = format!("log:{session_id}:{log_id}");
        self.raw_logs_tree
            .insert(key.as_bytes(), data)
            .map_err(|e| map_sled_error("insert_raw_log", e))?;
        Ok(())
    }

    pub fn get_raw_log(&self, session_id: &str, log_id: &str) -> Result<Option<Vec<u8>>> {
        let key = format!("log:{session_id}:{log_id}");
        self.raw_logs_tree
            .get(key.as_bytes())
            .map(|opt| opt.map(|ivec| ivec.to_vec()))
            .map_err(|e| map_sled_error("get_raw_log", e))
    }

    pub fn remove_raw_log(&self, session_id: &str, log_id: &str) -> Result<bool> {
        let key = format!("log:{session_id}:{log_id}");
        self.raw_logs_tree
            .remove(key.as_bytes())
            .map(|opt| opt.is_some())
            .map_err(|e| map_sled_error("remove_raw_log", e))
    }

    pub fn scan_logs_by_session(&self, session_id: &str) -> Result<Vec<(String, Vec<u8>)>> {
        let prefix = format!("log:{session_id}:");
        let mut results = Vec::new();
        for item in self.raw_logs_tree.scan_prefix(prefix.as_bytes()) {
            let (key, value) = item.map_err(|e| map_sled_error("scan_logs_by_session", e))?;
            let key_str = String::from_utf8_lossy(&key).to_string();
            results.push((key_str, value.to_vec()));
        }
        Ok(results)
    }

    pub fn insert_experience(&self, experience_id: &str, data: &[u8]) -> Result<()> {
        let key = format!("exp:{experience_id}");
        self.experiences_tree
            .insert(key.as_bytes(), data)
            .map_err(|e| map_sled_error("insert_experience", e))?;
        Ok(())
    }

    pub fn get_experience(&self, experience_id: &str) -> Result<Option<Vec<u8>>> {
        let key = format!("exp:{experience_id}");
        self.experiences_tree
            .get(key.as_bytes())
            .map(|opt| opt.map(|ivec| ivec.to_vec()))
            .map_err(|e| map_sled_error("get_experience", e))
    }

    pub fn remove_experience(&self, experience_id: &str) -> Result<bool> {
        let key = format!("exp:{experience_id}");
        self.experiences_tree
            .remove(key.as_bytes())
            .map(|opt| opt.is_some())
            .map_err(|e| map_sled_error("remove_experience", e))
    }

    pub fn flush(&self) -> Result<()> {
        self.db
            .flush()
            .map_err(|e| MemoryError::FlushFailed(format!("flush failed: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_temp() {
        let store = SledKvStore::open_temp();
        assert!(store.is_ok());
    }

    #[test]
    fn test_snapshot_crud() {
        let store = SledKvStore::open_temp().unwrap();

        assert!(store.get_snapshot("snap-001").unwrap().is_none());

        store.insert_snapshot("snap-001", b"snapshot data").unwrap();
        let data = store.get_snapshot("snap-001").unwrap();
        assert!(data.is_some());
        assert_eq!(data.unwrap(), b"snapshot data");

        let removed = store.remove_snapshot("snap-001").unwrap();
        assert!(removed);

        let removed_again = store.remove_snapshot("snap-001").unwrap();
        assert!(!removed_again);

        assert!(store.get_snapshot("snap-001").unwrap().is_none());
    }

    #[test]
    fn test_raw_log_crud() {
        let store = SledKvStore::open_temp().unwrap();

        store
            .insert_raw_log("sess-001", "log-001", b"log entry 1")
            .unwrap();
        store
            .insert_raw_log("sess-001", "log-002", b"log entry 2")
            .unwrap();
        store
            .insert_raw_log("sess-002", "log-003", b"log entry 3")
            .unwrap();

        let log = store.get_raw_log("sess-001", "log-001").unwrap();
        assert!(log.is_some());
        assert_eq!(log.unwrap(), b"log entry 1");

        let logs = store.scan_logs_by_session("sess-001").unwrap();
        assert_eq!(logs.len(), 2);
    }

    #[test]
    fn test_scan_logs_by_session_isolation() {
        let store = SledKvStore::open_temp().unwrap();

        store.insert_raw_log("sess-a", "log-1", b"a1").unwrap();
        store.insert_raw_log("sess-a", "log-2", b"a2").unwrap();
        store.insert_raw_log("sess-b", "log-1", b"b1").unwrap();

        let logs_a = store.scan_logs_by_session("sess-a").unwrap();
        assert_eq!(logs_a.len(), 2);

        let logs_b = store.scan_logs_by_session("sess-b").unwrap();
        assert_eq!(logs_b.len(), 1);
        assert_eq!(logs_b[0].1, b"b1");
    }

    #[test]
    fn test_experience_crud() {
        let store = SledKvStore::open_temp().unwrap();

        assert!(store.get_experience("exp-001").unwrap().is_none());

        store
            .insert_experience("exp-001", b"experience data")
            .unwrap();
        let data = store.get_experience("exp-001").unwrap();
        assert!(data.is_some());
        assert_eq!(data.unwrap(), b"experience data");

        let removed = store.remove_experience("exp-001").unwrap();
        assert!(removed);

        let removed_again = store.remove_experience("exp-001").unwrap();
        assert!(!removed_again);

        assert!(store.get_experience("exp-001").unwrap().is_none());
    }

    #[test]
    fn test_flush() {
        let store = SledKvStore::open_temp().unwrap();
        store.insert_snapshot("snap-1", b"data").unwrap();
        assert!(store.flush().is_ok());
    }

    #[test]
    fn test_sled_config_default() {
        let config = SledConfig::default();
        assert_eq!(config.cache_capacity_bytes, 128 * 1024 * 1024);
        assert_eq!(config.segment_size_bytes, 8 * 1024 * 1024);
        assert_eq!(config.flush_interval, Duration::from_secs(5 * 60));
    }

    #[test]
    fn test_snapshot_overwrite() {
        let store = SledKvStore::open_temp().unwrap();

        store.insert_snapshot("snap-1", b"v1").unwrap();
        store.insert_snapshot("snap-1", b"v2").unwrap();

        let data = store.get_snapshot("snap-1").unwrap().unwrap();
        assert_eq!(data, b"v2");
    }
}
