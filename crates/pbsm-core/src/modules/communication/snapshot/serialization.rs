use serde::{Deserialize, Serialize};

use crate::modules::communication::error::CommunicationError;
use crate::modules::communication::types::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompressedSnapshot {
    pub data: Vec<u8>,
    pub algorithm: CompressionAlgorithm,
    pub original_size: usize,
    pub compressed_size: usize,
}

pub fn to_json_string(snapshot: &CommunicationSnapshot) -> Result<String, CommunicationError> {
    serde_json::to_string(snapshot).map_err(CommunicationError::from)
}

pub fn to_json_bytes(snapshot: &CommunicationSnapshot) -> Result<Vec<u8>, CommunicationError> {
    serde_json::to_vec(snapshot).map_err(CommunicationError::from)
}

pub fn from_json_str(json: &str) -> Result<CommunicationSnapshot, CommunicationError> {
    serde_json::from_str(json).map_err(CommunicationError::from)
}

pub fn from_json_slice(data: &[u8]) -> Result<CommunicationSnapshot, CommunicationError> {
    serde_json::from_slice(data).map_err(CommunicationError::from)
}

pub fn compress_snapshot(
    snapshot: &CommunicationSnapshot,
    algorithm: CompressionAlgorithm,
) -> Result<CompressedSnapshot, CommunicationError> {
    let json_bytes = to_json_bytes(snapshot)?;

    match algorithm {
        CompressionAlgorithm::None => Ok(CompressedSnapshot {
            data: json_bytes.clone(),
            algorithm,
            original_size: json_bytes.len(),
            compressed_size: json_bytes.len(),
        }),
        CompressionAlgorithm::Lz4 => {
            let compressed = lz4_flex::compress_prepend_size(&json_bytes);
            Ok(CompressedSnapshot {
                compressed_size: compressed.len(),
                data: compressed,
                algorithm,
                original_size: json_bytes.len(),
            })
        }
        CompressionAlgorithm::Zstd => {
            let compressed = zstd::encode_all(json_bytes.as_slice(), 0).map_err(|e| {
                CommunicationError::InternalError {
                    context: format!("Zstd compression failed: {}", e),
                }
            })?;
            Ok(CompressedSnapshot {
                compressed_size: compressed.len(),
                data: compressed,
                algorithm,
                original_size: json_bytes.len(),
            })
        }
    }
}

pub fn decompress_snapshot(
    compressed: &CompressedSnapshot,
) -> Result<CommunicationSnapshot, CommunicationError> {
    let json_bytes = match compressed.algorithm {
        CompressionAlgorithm::None => compressed.data.clone(),
        CompressionAlgorithm::Lz4 => lz4_flex::decompress_size_prepended(&compressed.data)
            .map_err(|e| CommunicationError::InternalError {
                context: format!("Lz4 decompression failed: {}", e),
            })?,
        CompressionAlgorithm::Zstd => {
            zstd::decode_all(compressed.data.as_slice()).map_err(|e| {
                CommunicationError::InternalError {
                    context: format!("Zstd decompression failed: {}", e),
                }
            })?
        }
    };

    from_json_slice(&json_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_snapshot() -> CommunicationSnapshot {
        CommunicationSnapshot {
            snapshot_id: "test-snapshot-001".to_string(),
            snapshot_metadata: SnapshotMetadata {
                version: "1.0".to_string(),
                timestamp: Utc::now(),
                source_agent: SourceAgent {
                    agent_id: "agent-test".to_string(),
                    agent_type: Some("coordinator".to_string()),
                    capabilities: vec!["query".to_string()],
                },
                scope: CommSnapshotScope::default(),
                purpose: SnapshotPurpose::Sync,
                priority: Priority::Normal,
                expires_at: None,
            },
            entity_beliefs: vec![EntityBelief {
                node_id: "node-1".to_string(),
                node_type: CommNodeType::User,
                name: Some("Alice".to_string()),
                key_attributes: Some({
                    let mut map = HashMap::new();
                    map.insert(
                        "role".to_string(),
                        CommAttributeValue {
                            value: serde_json::json!("admin"),
                            confidence: 0.95,
                            source: Some("direct".to_string()),
                            last_updated: Some(Utc::now()),
                        },
                    );
                    map
                }),
                tags: vec!["important".to_string()],
            }],
            relation_beliefs: vec![],
            intention_summary: None,
            prediction_residual_summary: vec![],
            delegation_context: None,
            security_metadata: None,
            compression_info: None,
        }
    }

    #[test]
    fn test_json_string_roundtrip() {
        let snapshot = create_test_snapshot();
        let json = to_json_string(&snapshot).unwrap();
        let restored = from_json_str(&json).unwrap();
        assert_eq!(restored.snapshot_id, snapshot.snapshot_id);
        assert_eq!(restored.entity_beliefs.len(), snapshot.entity_beliefs.len());
    }

    #[test]
    fn test_json_bytes_roundtrip() {
        let snapshot = create_test_snapshot();
        let bytes = to_json_bytes(&snapshot).unwrap();
        let restored = from_json_slice(&bytes).unwrap();
        assert_eq!(restored.snapshot_id, snapshot.snapshot_id);
    }

    #[test]
    fn test_compress_decompress_none() {
        let snapshot = create_test_snapshot();
        let compressed = compress_snapshot(&snapshot, CompressionAlgorithm::None).unwrap();
        assert_eq!(compressed.algorithm, CompressionAlgorithm::None);
        assert_eq!(compressed.original_size, compressed.compressed_size);

        let restored = decompress_snapshot(&compressed).unwrap();
        assert_eq!(restored.snapshot_id, snapshot.snapshot_id);
    }

    #[test]
    fn test_compress_decompress_lz4() {
        let snapshot = create_test_snapshot();
        let compressed = compress_snapshot(&snapshot, CompressionAlgorithm::Lz4).unwrap();
        assert_eq!(compressed.algorithm, CompressionAlgorithm::Lz4);
        assert!(compressed.compressed_size > 0);

        let restored = decompress_snapshot(&compressed).unwrap();
        assert_eq!(restored.snapshot_id, snapshot.snapshot_id);
        assert_eq!(restored.entity_beliefs.len(), snapshot.entity_beliefs.len());
    }

    #[test]
    fn test_compress_decompress_zstd() {
        let snapshot = create_test_snapshot();
        let compressed = compress_snapshot(&snapshot, CompressionAlgorithm::Zstd).unwrap();
        assert_eq!(compressed.algorithm, CompressionAlgorithm::Zstd);
        assert!(compressed.compressed_size > 0);

        let restored = decompress_snapshot(&compressed).unwrap();
        assert_eq!(restored.snapshot_id, snapshot.snapshot_id);
        assert_eq!(restored.entity_beliefs.len(), snapshot.entity_beliefs.len());
    }
}
