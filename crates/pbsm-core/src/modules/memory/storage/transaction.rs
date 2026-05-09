use parking_lot::Mutex;
use rusqlite::Transaction;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::modules::memory::error::{MemoryError, Result};
use crate::modules::memory::storage::sqlite::SqliteStorage;

pub struct TransactionManager {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl TransactionManager {
    pub fn new(storage: &SqliteStorage) -> Self {
        Self {
            conn: storage.get_connection(),
        }
    }

    pub fn execute_transaction<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Transaction<'_>) -> Result<T>,
    {
        let conn = self.conn.lock();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| MemoryError::TransactionBegin(e.to_string()))?;
        match f(&tx) {
            Ok(result) => {
                tx.commit()
                    .map_err(|e| MemoryError::TransactionCommit(e.to_string()))?;
                Ok(result)
            }
            Err(e) => {
                let _ = tx.rollback();
                Err(e)
            }
        }
    }

    pub fn batch_insert_memory_index(
        &self,
        entries: Vec<(String, String, String, f64, i64)>,
    ) -> Result<Vec<String>> {
        let entry_ids: Vec<String> = entries.iter().map(|(id, _, _, _, _)| id.clone()).collect();
        self.execute_transaction(|tx| {
            for (entry_id, layer, topic, confidence, created_at) in &entries {
                tx.execute(
                    "INSERT INTO memory_index (entry_id, layer, topic, confidence, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![entry_id, layer, topic, confidence, created_at],
                )
                .map_err(|e| MemoryError::WriteFailed(e.to_string()))?;
            }
            Ok(entry_ids.clone())
        })
    }

    pub fn with_retries<F, T>(&self, max_retries: u32, f: F) -> Result<T>
    where
        F: Fn() -> Result<T>,
    {
        let mut last_error: Option<MemoryError> = None;
        for attempt in 0..=max_retries {
            match f() {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let is_retryable = matches!(
                        &e,
                        MemoryError::WriteFailed(_)
                            | MemoryError::TransactionConflict(_)
                            | MemoryError::DatabaseLocked(_)
                    );
                    if !is_retryable || attempt == max_retries {
                        return Err(e);
                    }
                    last_error = Some(e);
                    let backoff = Duration::from_millis(50 * 2u64.pow(attempt));
                    thread::sleep(backoff);
                }
            }
        }
        Err(last_error.unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_storage() -> SqliteStorage {
        let storage = SqliteStorage::open_in_memory().expect("failed to open in-memory db");
        storage.init_schema().expect("failed to initialize schema");
        storage
    }

    #[test]
    fn test_new_transaction_manager() {
        let storage = setup_storage();
        let _manager = TransactionManager::new(&storage);
    }

    #[test]
    fn test_execute_transaction_commit() {
        let storage = setup_storage();
        let manager = TransactionManager::new(&storage);

        let result: Result<i32> = manager.execute_transaction(|tx| {
            tx.execute(
                "INSERT INTO memory_index (entry_id, layer, topic, confidence, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params!["id-1", "RAW_LOG", "test", 0.9, 1000i64],
            )
            .map_err(|e| MemoryError::WriteFailed(e.to_string()))?;
            Ok(42)
        });

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let conn = storage.get_connection();
        let lock = conn.lock();
        let count: i64 = lock
            .query_row(
                "SELECT COUNT(*) FROM memory_index WHERE entry_id = 'id-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_execute_transaction_rollback() {
        let storage = setup_storage();
        let manager = TransactionManager::new(&storage);

        let result: Result<i32> = manager.execute_transaction(|tx| {
            tx.execute(
                "INSERT INTO memory_index (entry_id, layer, topic, confidence, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params!["id-rollback", "SNAPSHOT", "test", 0.5, 2000i64],
            )
            .map_err(|e| MemoryError::WriteFailed(e.to_string()))?;
            Err(MemoryError::InvalidQuery("forced error".to_string()))
        });

        assert!(result.is_err());

        let conn = storage.get_connection();
        let lock = conn.lock();
        let count: i64 = lock
            .query_row(
                "SELECT COUNT(*) FROM memory_index WHERE entry_id = 'id-rollback'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_batch_insert_memory_index() {
        let storage = setup_storage();
        let manager = TransactionManager::new(&storage);

        let entries = vec![
            (
                "id-a".to_string(),
                "RAW_LOG".to_string(),
                "topic-1".to_string(),
                0.8,
                1000i64,
            ),
            (
                "id-b".to_string(),
                "SNAPSHOT".to_string(),
                "topic-2".to_string(),
                0.6,
                2000i64,
            ),
            (
                "id-c".to_string(),
                "EXPERIENCE".to_string(),
                "topic-1".to_string(),
                0.95,
                3000i64,
            ),
        ];

        let result = manager.batch_insert_memory_index(entries);
        assert!(result.is_ok());

        let ids = result.unwrap();
        assert_eq!(ids, vec!["id-a", "id-b", "id-c"]);

        let conn = storage.get_connection();
        let lock = conn.lock();
        let count: i64 = lock
            .query_row("SELECT COUNT(*) FROM memory_index", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_batch_insert_empty() {
        let storage = setup_storage();
        let manager = TransactionManager::new(&storage);

        let result = manager.batch_insert_memory_index(vec![]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_with_retries_success_first_try() {
        let storage = setup_storage();
        let manager = TransactionManager::new(&storage);

        let result = manager.with_retries(3, || Ok::<_, MemoryError>(123));
        assert_eq!(result.unwrap(), 123);
    }

    #[test]
    fn test_with_retries_non_retryable_error() {
        let storage = setup_storage();
        let manager = TransactionManager::new(&storage);

        let result: Result<i32> = manager.with_retries(3, || {
            Err(MemoryError::InvalidQuery("not retryable".to_string()))
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            MemoryError::InvalidQuery(msg) => assert_eq!(msg, "not retryable"),
            other => panic!("expected InvalidQuery, got {:?}", other),
        }
    }

    #[test]
    fn test_with_retries_eventual_success() {
        let storage = setup_storage();
        let manager = TransactionManager::new(&storage);

        let attempt = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let attempt_clone = attempt.clone();

        let result = manager.with_retries(3, move || {
            let current = attempt_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if current < 2 {
                Err(MemoryError::WriteFailed("temporary failure".to_string()))
            } else {
                Ok("success")
            }
        });

        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempt.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[test]
    fn test_with_retries_exhausted() {
        let storage = setup_storage();
        let manager = TransactionManager::new(&storage);

        let attempt = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let attempt_clone = attempt.clone();

        let result: Result<&str> = manager.with_retries(2, move || {
            attempt_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(MemoryError::DatabaseLocked("still locked".to_string()))
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            MemoryError::DatabaseLocked(msg) => assert_eq!(msg, "still locked"),
            other => panic!("expected DatabaseLocked, got {:?}", other),
        }
        assert_eq!(attempt.load(std::sync::atomic::Ordering::SeqCst), 3);
    }
}
