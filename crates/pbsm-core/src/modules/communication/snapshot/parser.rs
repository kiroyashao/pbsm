use chrono::Utc;
use uuid::Uuid;

use crate::modules::communication::error::CommunicationError;
use crate::modules::communication::types::*;

use super::serialization;

pub struct SnapshotParser {
    supported_versions: Vec<String>,
}

impl SnapshotParser {
    pub fn new() -> Self {
        Self {
            supported_versions: vec!["1.0".to_string()],
        }
    }

    pub fn parse_snapshot(
        &self,
        raw_data: &[u8],
        metadata: Option<ParseMetadata>,
    ) -> Result<ParsedSnapshot, CommunicationError> {
        self.receive_snapshot(raw_data, metadata)
    }

    pub fn receive_snapshot(
        &self,
        raw_data: &[u8],
        metadata: Option<ParseMetadata>,
    ) -> Result<ParsedSnapshot, CommunicationError> {
        let meta = metadata.unwrap_or_default();

        let json_data = if let Some(compression) = meta.compression {
            match compression {
                CompressionAlgorithm::None => raw_data.to_vec(),
                CompressionAlgorithm::Lz4 => lz4_flex::decompress_size_prepended(raw_data)
                    .map_err(|e| {
                        CommunicationError::SnapshotParsingFailed(format!(
                            "Lz4 decompression failed: {}",
                            e
                        ))
                    })?,
                CompressionAlgorithm::Zstd => zstd::decode_all(raw_data).map_err(|e| {
                    CommunicationError::SnapshotParsingFailed(format!(
                        "Zstd decompression failed: {}",
                        e
                    ))
                })?,
            }
        } else {
            raw_data.to_vec()
        };

        let snapshot: CommunicationSnapshot = serialization::from_json_slice(&json_data)?;

        let verification_result = self.verify_snapshot(&snapshot, &meta)?;

        let mut warnings = Vec::new();
        if verification_result.result != VerificationOutcome::Passed
            && verification_result.result == VerificationOutcome::PartialPass
        {
            warnings.push("Snapshot verification partially passed".to_string());
        }

        Ok(ParsedSnapshot {
            snapshot,
            verification_result,
            warnings,
        })
    }

    fn verify_snapshot(
        &self,
        snapshot: &CommunicationSnapshot,
        metadata: &ParseMetadata,
    ) -> Result<VerificationResult, CommunicationError> {
        let mut checks = VerificationChecks::default();
        let mut all_passed = true;

        checks.format_validation = Some(FormatValidation {
            passed: true,
            errors: vec![],
        });

        let version_check_passed = if let Some(ref expected_version) = metadata.expected_version {
            let is_compatible = self
                .supported_versions
                .iter()
                .any(|v| snapshot.snapshot_metadata.version.starts_with(v))
                && snapshot
                    .snapshot_metadata
                    .version
                    .starts_with(expected_version);
            if !is_compatible {
                all_passed = false;
            }
            is_compatible
        } else {
            self.supported_versions
                .iter()
                .any(|v| snapshot.snapshot_metadata.version.starts_with(v))
        };

        if !version_check_passed {
            all_passed = false;
        }

        checks.version_check = Some(VersionCheck {
            passed: version_check_passed,
            snapshot_version: snapshot.snapshot_metadata.version.clone(),
            supported_versions: self.supported_versions.clone(),
        });

        let now = Utc::now();
        let age = now
            .signed_duration_since(snapshot.snapshot_metadata.timestamp)
            .num_seconds();
        let age_seconds = if age >= 0 { age as u64 } else { 0 };
        let max_age_seconds: u64 = 3600;

        let timestamp_passed = if let Some(expires_at) = snapshot.snapshot_metadata.expires_at {
            let not_expired = now < expires_at;
            if !not_expired {
                all_passed = false;
            }
            not_expired
        } else {
            age_seconds <= max_age_seconds
        };

        if !timestamp_passed {
            all_passed = false;
        }

        checks.timestamp_validation = Some(TimestampValidation {
            passed: timestamp_passed,
            age_seconds,
            max_age_seconds,
        });

        let outcome = if all_passed {
            VerificationOutcome::Passed
        } else if !timestamp_passed && snapshot.snapshot_metadata.expires_at.is_some() {
            VerificationOutcome::Expired
        } else {
            VerificationOutcome::Failed
        };

        Ok(VerificationResult {
            verification_id: Uuid::new_v4().to_string(),
            snapshot_id: snapshot.snapshot_id.clone(),
            result: outcome,
            checks,
            warnings: vec![],
            timestamp: Utc::now(),
        })
    }
}

impl Default for SnapshotParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use std::collections::HashMap;

    fn create_test_snapshot() -> CommunicationSnapshot {
        CommunicationSnapshot {
            snapshot_id: "test-parse-001".to_string(),
            snapshot_metadata: SnapshotMetadata {
                version: "1.0".to_string(),
                timestamp: Utc::now(),
                source_agent: SourceAgent {
                    agent_id: "agent-test".to_string(),
                    agent_type: None,
                    capabilities: vec![],
                },
                scope: CommSnapshotScope::default(),
                purpose: SnapshotPurpose::Sync,
                priority: Priority::Normal,
                expires_at: None,
            },
            entity_beliefs: vec![],
            relation_beliefs: vec![],
            intention_summary: None,
            prediction_residual_summary: vec![],
            delegation_context: None,
            security_metadata: None,
            compression_info: None,
        }
    }

    #[test]
    fn test_parse_valid_snapshot() {
        let snapshot = create_test_snapshot();
        let json = serde_json::to_vec(&snapshot).unwrap();
        let parser = SnapshotParser::new();

        let result = parser.parse_snapshot(&json, None);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.snapshot.snapshot_id, "test-parse-001");
        assert_eq!(
            parsed.verification_result.result,
            VerificationOutcome::Passed
        );
    }

    #[test]
    fn test_version_mismatch() {
        let mut snapshot = create_test_snapshot();
        snapshot.snapshot_metadata.version = "2.0".to_string();
        let json = serde_json::to_vec(&snapshot).unwrap();
        let parser = SnapshotParser::new();

        let meta = ParseMetadata {
            expected_version: Some("1.0".to_string()),
            ..ParseMetadata::default()
        };

        let result = parser.parse_snapshot(&json, Some(meta));
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(
            parsed.verification_result.result,
            VerificationOutcome::Failed
        );
        assert!(
            !parsed
                .verification_result
                .checks
                .version_check
                .unwrap()
                .passed
        );
    }

    #[test]
    fn test_expired_snapshot() {
        let mut snapshot = create_test_snapshot();
        snapshot.snapshot_metadata.expires_at = Some(Utc::now() - Duration::seconds(3600));
        let json = serde_json::to_vec(&snapshot).unwrap();
        let parser = SnapshotParser::new();

        let result = parser.parse_snapshot(&json, None);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(
            parsed.verification_result.result,
            VerificationOutcome::Expired
        );
    }

    #[test]
    fn test_roundtrip_with_constructor() {
        use super::super::constructor::SnapshotConstructor;
        use crate::modules::belief_graph::graph::BeliefGraph;
        use crate::modules::belief_graph::operations::BeliefGraphOperations;
        use crate::modules::belief_graph::types::*;
        use std::sync::Arc;

        let graph = Arc::new(BeliefGraph::with_default_config());
        BeliefGraphOperations::create_belief(
            &graph,
            BeliefNodeType::User,
            "Alice".to_string(),
            HashMap::new(),
            "test".to_string(),
            SourceType::UserInput,
            None,
            None,
        )
        .unwrap();

        let constructor = SnapshotConstructor::new(
            graph,
            "agent-001".to_string(),
            "coordinator".to_string(),
            vec![],
        );

        let constructed = constructor
            .construct_snapshot(CommSnapshotScope::default(), SnapshotPurpose::Sync, None)
            .unwrap();

        let json = serde_json::to_vec(&constructed.snapshot).unwrap();
        let parser = SnapshotParser::new();
        let parsed = parser.parse_snapshot(&json, None).unwrap();

        assert_eq!(
            parsed.snapshot.snapshot_id,
            constructed.snapshot.snapshot_id
        );
        assert_eq!(
            parsed.verification_result.result,
            VerificationOutcome::Passed
        );
    }
}
