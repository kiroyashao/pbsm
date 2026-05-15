use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::config::ForgettingConfig;
use super::error::Result;
use super::events::{MetacognitiveEvent, MetacognitiveEventPublisher};
use super::types::{
    ArchiveResult, DeferredForget, ForceForgetRequest, ForceForgetResponse, ForgetCandidate,
    ForgetReason, ForgetStatistics, GetForgetStatusResponse, PendingForget, RecentForget,
};

pub struct ForgettingExecutor {
    config: ForgettingConfig,
    deferred_forgets: RwLock<HashMap<String, DeferredForgetEntry>>,
    recent_forgets: RwLock<Vec<RecentForget>>,
    recent_value_scores: RwLock<Vec<f64>>,
    total_forgotten_session: RwLock<usize>,
    belief_ages: RwLock<HashMap<String, usize>>,
    protected_beliefs: RwLock<Vec<String>>,
    belief_residual_association: RwLock<HashMap<String, f64>>,
    event_publisher: Arc<dyn MetacognitiveEventPublisher>,
}

#[derive(Debug, Clone)]
struct DeferredForgetEntry {
    node_id: String,
    defer_reason: String,
    defer_steps: usize,
    residual_association: f64,
}

impl ForgettingExecutor {
    pub fn new(
        config: ForgettingConfig,
        event_publisher: Arc<dyn MetacognitiveEventPublisher>,
    ) -> Self {
        Self {
            config,
            deferred_forgets: RwLock::new(HashMap::new()),
            recent_forgets: RwLock::new(Vec::new()),
            recent_value_scores: RwLock::new(Vec::new()),
            total_forgotten_session: RwLock::new(0),
            belief_ages: RwLock::new(HashMap::new()),
            protected_beliefs: RwLock::new(Vec::new()),
            belief_residual_association: RwLock::new(HashMap::new()),
            event_publisher,
        }
    }

    pub fn identify_forget_candidates(
        &self,
        node_ids: &[String],
        value_scores: &HashMap<String, f64>,
        reason: ForgetReason,
    ) -> Vec<ForgetCandidate> {
        let protected = self.protected_beliefs.read();
        let residual_assoc = self.belief_residual_association.read();
        let ages = self.belief_ages.read();

        let mut candidates: Vec<ForgetCandidate> = node_ids
            .iter()
            .filter_map(|id| {
                let score = value_scores.get(id).copied().unwrap_or(1.0);
                if score >= self.config.forget_threshold {
                    return None;
                }

                let age = ages.get(id).copied().unwrap_or(0);
                if age < self.config.min_survival_steps {
                    return None;
                }

                let is_protected = protected.contains(id);
                let residual = residual_assoc.get(id).copied().unwrap_or(0.0);
                let is_deferred = residual > self.config.residual_defer_threshold;

                Some(ForgetCandidate {
                    node_id: id.clone(),
                    value_score: score,
                    reason: format!("{:?}", reason),
                    is_protected,
                    is_deferred,
                    defer_reason: if is_deferred {
                        Some("RESIDUAL_ASSOCIATION".to_string())
                    } else {
                        None
                    },
                })
            })
            .collect();

        candidates.sort_by(|a, b| {
            a.value_score
                .partial_cmp(&b.value_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates
    }

    pub fn trigger_forget(
        &self,
        request: ForceForgetRequest,
        value_scores: &HashMap<String, f64>,
    ) -> Result<ForceForgetResponse> {
        let candidates =
            self.identify_forget_candidates(&request.node_ids, value_scores, request.reason);

        let mut forgotten_ids = Vec::new();
        let mut protected_ids = Vec::new();
        let mut deferred_ids = Vec::new();
        let mut archive_results = Vec::new();

        for candidate in candidates {
            if candidate.is_protected && !request.force_flag.unwrap_or(false) {
                protected_ids.push(candidate.node_id.clone());
                continue;
            }

            if candidate.is_deferred && !request.force_flag.unwrap_or(false) {
                let entry = DeferredForgetEntry {
                    node_id: candidate.node_id.clone(),
                    defer_reason: candidate.defer_reason.clone().unwrap_or_default(),
                    defer_steps: 0,
                    residual_association: self
                        .belief_residual_association
                        .read()
                        .get(&candidate.node_id)
                        .copied()
                        .unwrap_or(0.0),
                };
                self.deferred_forgets
                    .write()
                    .insert(candidate.node_id.clone(), entry);
                deferred_ids.push(candidate.node_id.clone());
                continue;
            }

            match self.archive_and_remove(&candidate.node_id, candidate.value_score) {
                Ok(location) => {
                    forgotten_ids.push(candidate.node_id.clone());
                    archive_results.push(ArchiveResult {
                        node_id: candidate.node_id,
                        success: true,
                        archive_location: Some(location),
                        error: None,
                    });
                }
                Err(e) => {
                    archive_results.push(ArchiveResult {
                        node_id: candidate.node_id,
                        success: false,
                        archive_location: None,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        let _ = self
            .event_publisher
            .publish(MetacognitiveEvent::ForgetCompleted {
                archived_count: forgotten_ids.len(),
                archived_ids: forgotten_ids.clone(),
                deferred_count: deferred_ids.len(),
                deferred_ids: deferred_ids.clone(),
                protection_violations: protected_ids.clone(),
                archive_location: "archive".to_string(),
            });

        Ok(ForceForgetResponse {
            forgotten_ids,
            protected_ids,
            deferred_ids,
            archive_results,
            success: true,
        })
    }

    fn archive_and_remove(&self, node_id: &str, value_score: f64) -> Result<String> {
        let archive_id = Uuid::new_v4().to_string();
        let location = format!("archive/{}", archive_id);

        self.recent_forgets.write().push(RecentForget {
            node_id: node_id.to_string(),
            archived_at: Utc::now(),
            reason: "LOW_VALUE".to_string(),
        });

        self.recent_value_scores.write().push(value_score);

        *self.total_forgotten_session.write() += 1;

        self.belief_ages.write().remove(node_id);
        self.belief_residual_association.write().remove(node_id);

        Ok(location)
    }

    pub fn process_deferred_forgetting(&self) -> Vec<String> {
        let mut deferred = self.deferred_forgets.write();
        let residual_assoc = self.belief_residual_association.read();
        let mut ready_to_forget = Vec::new();
        let mut to_remove = Vec::new();

        for (node_id, entry) in deferred.iter_mut() {
            entry.defer_steps += 1;

            let current_residual = residual_assoc
                .get(node_id)
                .copied()
                .unwrap_or(entry.residual_association);
            entry.residual_association = current_residual;

            if current_residual <= self.config.residual_defer_threshold
                || (entry.defer_steps >= self.config.max_defer_steps && current_residual < 0.9)
            {
                ready_to_forget.push(node_id.clone());
                to_remove.push(node_id.clone());
            }

            if entry.defer_steps >= self.config.max_defer_steps && current_residual >= 0.9 {
                let _ = self.event_publisher.publish(MetacognitiveEvent::DeferredForgetWarning {
                    node_id: node_id.clone(),
                    residual_association: current_residual,
                    defer_steps: entry.defer_steps,
                    max_defer_steps: self.config.max_defer_steps,
                });
            }
        }

        for id in &to_remove {
            deferred.remove(id);
        }

        ready_to_forget
    }

    pub fn get_forget_status(&self) -> GetForgetStatusResponse {
        let deferred = self.deferred_forgets.read();
        let recent = self.recent_forgets.read();
        let total_session = *self.total_forgotten_session.read();
        let value_scores = self.recent_value_scores.read();

        let deferred_forgets: Vec<DeferredForget> = deferred
            .values()
            .map(|e| DeferredForget {
                node_id: e.node_id.clone(),
                defer_reason: e.defer_reason.clone(),
                defer_steps: e.defer_steps,
            })
            .collect();

        let avg_score = if value_scores.is_empty() {
            0.0
        } else {
            value_scores.iter().sum::<f64>() / value_scores.len() as f64
        };

        let pending_forgets: Vec<PendingForget> = deferred
            .values()
            .filter(|e| (e.defer_steps as f64) >= self.config.max_defer_steps as f64 * 0.8)
            .map(|e| PendingForget {
                node_id: e.node_id.clone(),
                reason: e.defer_reason.clone(),
                queued_at: Utc::now(),
            })
            .collect();

        GetForgetStatusResponse {
            pending_forgets,
            deferred_forgets,
            recent_forgets: recent.clone(),
            statistics: ForgetStatistics {
                total_forgotten_this_session: total_session,
                total_forgotten_all_time: total_session,
                average_value_score: avg_score,
            },
        }
    }

    pub fn set_belief_age(&self, node_id: &str, age: usize) {
        self.belief_ages.write().insert(node_id.to_string(), age);
    }

    pub fn add_protected_belief(&self, node_id: &str) {
        let mut protected = self.protected_beliefs.write();
        if !protected.contains(&node_id.to_string()) {
            protected.push(node_id.to_string());
        }
    }

    pub fn set_belief_residual_association(&self, node_id: &str, association: f64) {
        self.belief_residual_association
            .write()
            .insert(node_id.to_string(), association);
    }

    pub fn is_protected(&self, node_id: &str) -> bool {
        self.protected_beliefs.read().contains(&node_id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_executor() -> ForgettingExecutor {
        ForgettingExecutor::new(
            ForgettingConfig::default(),
            Arc::new(super::super::events::NullMetacognitiveEventPublisher),
        )
    }

    #[test]
    fn test_identify_forget_candidates() {
        let executor = create_executor();
        executor.set_belief_age("n1", 20);
        executor.set_belief_age("n2", 20);

        let value_scores = vec![("n1".to_string(), 0.1), ("n2".to_string(), 0.5)]
            .into_iter()
            .collect();

        let candidates = executor.identify_forget_candidates(
            &["n1".to_string(), "n2".to_string()],
            &value_scores,
            ForgetReason::LowValue,
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].node_id, "n1");
    }

    #[test]
    fn test_protected_belief_excluded() {
        let executor = create_executor();
        executor.set_belief_age("n1", 20);
        executor.add_protected_belief("n1");

        let value_scores = vec![("n1".to_string(), 0.1)].into_iter().collect();

        let candidates = executor.identify_forget_candidates(
            &["n1".to_string()],
            &value_scores,
            ForgetReason::LowValue,
        );

        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].is_protected);
    }

    #[test]
    fn test_residual_association_deferred() {
        let executor = create_executor();
        executor.set_belief_age("n1", 20);
        executor.set_belief_residual_association("n1", 0.8);

        let value_scores = vec![("n1".to_string(), 0.1)].into_iter().collect();

        let candidates = executor.identify_forget_candidates(
            &["n1".to_string()],
            &value_scores,
            ForgetReason::LowValue,
        );

        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].is_deferred);
    }

    #[test]
    fn test_min_survival_steps() {
        let executor = create_executor();
        executor.set_belief_age("n1", 5);

        let value_scores = vec![("n1".to_string(), 0.1)].into_iter().collect();

        let candidates = executor.identify_forget_candidates(
            &["n1".to_string()],
            &value_scores,
            ForgetReason::LowValue,
        );

        assert!(candidates.is_empty());
    }

    #[test]
    fn test_trigger_forget() {
        let executor = create_executor();
        executor.set_belief_age("n1", 20);

        let value_scores = vec![("n1".to_string(), 0.1)].into_iter().collect();

        let result = executor
            .trigger_forget(
                ForceForgetRequest {
                    node_ids: vec!["n1".to_string()],
                    force_flag: None,
                    reason: ForgetReason::LowValue,
                },
                &value_scores,
            )
            .unwrap();

        assert!(result.forgotten_ids.contains(&"n1".to_string()));
    }

    #[test]
    fn test_trigger_forget_protected() {
        let executor = create_executor();
        executor.set_belief_age("n1", 20);
        executor.add_protected_belief("n1");

        let value_scores = vec![("n1".to_string(), 0.1)].into_iter().collect();

        let result = executor
            .trigger_forget(
                ForceForgetRequest {
                    node_ids: vec!["n1".to_string()],
                    force_flag: None,
                    reason: ForgetReason::LowValue,
                },
                &value_scores,
            )
            .unwrap();

        assert!(result.protected_ids.contains(&"n1".to_string()));
        assert!(!result.forgotten_ids.contains(&"n1".to_string()));
    }

    #[test]
    fn test_trigger_forget_force() {
        let executor = create_executor();
        executor.set_belief_age("n1", 20);
        executor.add_protected_belief("n1");

        let value_scores = vec![("n1".to_string(), 0.1)].into_iter().collect();

        let result = executor
            .trigger_forget(
                ForceForgetRequest {
                    node_ids: vec!["n1".to_string()],
                    force_flag: Some(true),
                    reason: ForgetReason::UserRequest,
                },
                &value_scores,
            )
            .unwrap();

        assert!(result.forgotten_ids.contains(&"n1".to_string()));
    }

    #[test]
    fn test_deferred_forgetting_processing() {
        let executor = create_executor();
        executor.set_belief_age("n1", 20);
        executor.set_belief_residual_association("n1", 0.8);

        let value_scores = vec![("n1".to_string(), 0.1)].into_iter().collect();

        let result = executor
            .trigger_forget(
                ForceForgetRequest {
                    node_ids: vec!["n1".to_string()],
                    force_flag: None,
                    reason: ForgetReason::LowValue,
                },
                &value_scores,
            )
            .unwrap();

        assert!(result.deferred_ids.contains(&"n1".to_string()));

        executor.set_belief_residual_association("n1", 0.3);
        let ready = executor.process_deferred_forgetting();
        assert!(ready.contains(&"n1".to_string()));
    }

    #[test]
    fn test_get_forget_status() {
        let executor = create_executor();
        let status = executor.get_forget_status();
        assert_eq!(status.statistics.total_forgotten_this_session, 0);
    }

    #[test]
    fn test_is_protected() {
        let executor = create_executor();
        assert!(!executor.is_protected("n1"));
        executor.add_protected_belief("n1");
        assert!(executor.is_protected("n1"));
    }
}
