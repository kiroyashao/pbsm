use std::sync::Arc;

use uuid::Uuid;

use crate::modules::memory::error::Result;
use crate::modules::memory::layers::experience::ExperienceLayer;
use crate::modules::memory::types::*;

pub struct ProblemRetriever {
    experience_layer: Arc<ExperienceLayer>,
}

impl ProblemRetriever {
    pub fn new(experience: Arc<ExperienceLayer>) -> Self {
        Self {
            experience_layer: experience,
        }
    }

    pub async fn retrieve(
        &self,
        problem_description: &str,
        problem_type: Option<ProblemType>,
        similar_solution_limit: usize,
    ) -> Result<ProblemRetrievalResult> {
        let inferred_type =
            problem_type.unwrap_or_else(|| Self::infer_problem_type(problem_description));

        let domain = Self::problem_type_to_domain(&inferred_type);
        let pattern = Self::problem_type_to_pattern(&inferred_type);

        let mut experiences = self.experience_layer.query_by_domain(&domain, None).await?;

        if let Ok(pattern_experiences) = self.experience_layer.query_by_pattern_type(&pattern).await
        {
            for exp in pattern_experiences {
                if !experiences.iter().any(|e| e.entry_id == exp.entry_id) {
                    experiences.push(exp);
                }
            }
        }

        let mut similar_problems: Vec<SimilarProblemCase> = experiences
            .iter()
            .take(similar_solution_limit)
            .map(|exp| {
                let similarity = Self::compute_similarity(problem_description, &exp.summary);
                let resolution_steps = Self::extract_resolution_steps(&exp.content);
                let outcome = Self::infer_outcome(exp);

                SimilarProblemCase {
                    problem_id: exp.entry_id.clone(),
                    problem_description: exp.summary.clone(),
                    similarity_score: similarity,
                    resolution_steps,
                    outcome,
                    resolution_context: exp.memory_type.clone(),
                }
            })
            .collect();

        similar_problems.sort_by(|a, b| {
            b.similarity_score
                .partial_cmp(&a.similarity_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        similar_problems.truncate(similar_solution_limit);

        let recommended_steps = Self::generate_solution_steps(&similar_problems);
        let adaptation_notes = Self::generate_adaptation_notes(&inferred_type, &similar_problems);
        let confidence = Self::estimate_confidence(&similar_problems);

        Ok(ProblemRetrievalResult {
            request_id: Uuid::new_v4().to_string(),
            original_problem: problem_description.to_string(),
            inferred_problem_type: inferred_type,
            similar_problems,
            recommended_steps,
            adaptation_notes,
            confidence,
        })
    }

    fn infer_problem_type(description: &str) -> ProblemType {
        let lower = description.to_lowercase();
        if lower.contains("tool") && (lower.contains("fail") || lower.contains("error")) {
            ProblemType::ToolExecutionFailure
        } else if lower.contains("predict") && lower.contains("mismatch") {
            ProblemType::PredictionMismatch
        } else if lower.contains("belief")
            && (lower.contains("conflict") || lower.contains("contradict"))
        {
            ProblemType::BeliefConflict
        } else if lower.contains("goal")
            && (lower.contains("ambiguous") || lower.contains("unclear"))
        {
            ProblemType::GoalAmbiguity
        } else if lower.contains("resource")
            && (lower.contains("limit") || lower.contains("constraint"))
        {
            ProblemType::ResourceConstraint
        } else {
            ProblemType::Unknown
        }
    }

    fn problem_type_to_domain(problem_type: &ProblemType) -> String {
        match problem_type {
            ProblemType::ToolExecutionFailure => "error_handling".to_string(),
            ProblemType::PredictionMismatch => "prediction_correction".to_string(),
            ProblemType::BeliefConflict => "belief_reconciliation".to_string(),
            ProblemType::GoalAmbiguity => "goal_clarification".to_string(),
            ProblemType::ResourceConstraint => "resource_management".to_string(),
            ProblemType::Unknown => "general".to_string(),
        }
    }

    fn problem_type_to_pattern(problem_type: &ProblemType) -> String {
        match problem_type {
            ProblemType::ToolExecutionFailure => "ERROR_HANDLING".to_string(),
            ProblemType::PredictionMismatch => "BELIEF_CORRECTION".to_string(),
            ProblemType::BeliefConflict => "BELIEF_CORRECTION".to_string(),
            ProblemType::GoalAmbiguity => "GOAL_DECOMPOSITION".to_string(),
            ProblemType::ResourceConstraint => "TASK_PATTERN".to_string(),
            ProblemType::Unknown => "TASK_PATTERN".to_string(),
        }
    }

    fn compute_similarity(description: &str, experience_summary: &str) -> f64 {
        let desc_lower = description.to_lowercase();
        let summary_lower = experience_summary.to_lowercase();
        let desc_words: Vec<&str> = desc_lower.split_whitespace().collect();
        let summary_words: Vec<&str> = summary_lower.split_whitespace().collect();

        if desc_words.is_empty() || summary_words.is_empty() {
            return 0.0;
        }

        let common_count = desc_words
            .iter()
            .filter(|w| summary_words.contains(w))
            .count();

        (common_count as f64 / desc_words.len().max(summary_words.len()) as f64).min(1.0)
    }

    fn extract_resolution_steps(content: &serde_json::Value) -> Vec<String> {
        match content {
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            serde_json::Value::Object(obj) => obj
                .get("steps")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            _ => vec![],
        }
    }

    fn infer_outcome(entry: &MemoryEntry) -> ProblemOutcome {
        if entry.confidence >= 0.7 {
            ProblemOutcome::Success
        } else if entry.confidence >= 0.4 {
            ProblemOutcome::Partial
        } else {
            ProblemOutcome::Failed
        }
    }

    fn generate_solution_steps(similar_problems: &[SimilarProblemCase]) -> Vec<SolutionStep> {
        similar_problems
            .iter()
            .flat_map(|case| {
                case.resolution_steps
                    .iter()
                    .enumerate()
                    .map(|(i, step)| SolutionStep {
                        step_number: (i + 1) as u32,
                        action_description: step.clone(),
                        expected_outcome: "Resolve the identified issue".to_string(),
                        adaptation_guidance: format!(
                            "Adapt based on {} context",
                            case.resolution_context
                        ),
                        confidence: case.similarity_score * 0.8,
                    })
            })
            .take(5)
            .collect()
    }

    fn generate_adaptation_notes(
        problem_type: &ProblemType,
        similar_problems: &[SimilarProblemCase],
    ) -> Vec<String> {
        let mut notes = vec![format!(
            "Problem type {:?} may require specific handling",
            problem_type
        )];

        if similar_problems.is_empty() {
            notes.push("No similar problems found in experience store".to_string());
        } else {
            let avg_similarity: f64 = similar_problems
                .iter()
                .map(|p| p.similarity_score)
                .sum::<f64>()
                / similar_problems.len() as f64;
            notes.push(format!(
                "Average similarity of matched problems: {:.2}",
                avg_similarity
            ));

            let partial_count = similar_problems
                .iter()
                .filter(|p| p.outcome == ProblemOutcome::Partial)
                .count();
            if partial_count > 0 {
                notes.push(format!(
                    "{} similar problems had partial resolution",
                    partial_count
                ));
            }
        }

        notes
    }

    fn estimate_confidence(similar_problems: &[SimilarProblemCase]) -> f64 {
        if similar_problems.is_empty() {
            return 0.1;
        }

        let success_weight = similar_problems
            .iter()
            .map(|p| match p.outcome {
                ProblemOutcome::Success => 1.0,
                ProblemOutcome::Partial => 0.5,
                ProblemOutcome::Failed => 0.0,
            })
            .sum::<f64>()
            / similar_problems.len() as f64;

        let avg_similarity: f64 = similar_problems
            .iter()
            .map(|p| p.similarity_score)
            .sum::<f64>()
            / similar_problems.len() as f64;

        (success_weight * 0.6 + avg_similarity * 0.4).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_problem_type_tool_failure() {
        assert_eq!(
            ProblemRetriever::infer_problem_type("tool execution failed with error"),
            ProblemType::ToolExecutionFailure
        );
    }

    #[test]
    fn test_infer_problem_type_prediction_mismatch() {
        assert_eq!(
            ProblemRetriever::infer_problem_type("prediction mismatch detected"),
            ProblemType::PredictionMismatch
        );
    }

    #[test]
    fn test_infer_problem_type_belief_conflict() {
        assert_eq!(
            ProblemRetriever::infer_problem_type("belief conflict in reasoning"),
            ProblemType::BeliefConflict
        );
    }

    #[test]
    fn test_infer_problem_type_goal_ambiguity() {
        assert_eq!(
            ProblemRetriever::infer_problem_type("goal is ambiguous and unclear"),
            ProblemType::GoalAmbiguity
        );
    }

    #[test]
    fn test_infer_problem_type_resource_constraint() {
        assert_eq!(
            ProblemRetriever::infer_problem_type("resource limit constraint reached"),
            ProblemType::ResourceConstraint
        );
    }

    #[test]
    fn test_infer_problem_type_unknown() {
        assert_eq!(
            ProblemRetriever::infer_problem_type("something unexpected happened"),
            ProblemType::Unknown
        );
    }

    #[test]
    fn test_problem_type_to_domain() {
        assert_eq!(
            ProblemRetriever::problem_type_to_domain(&ProblemType::ToolExecutionFailure),
            "error_handling"
        );
        assert_eq!(
            ProblemRetriever::problem_type_to_domain(&ProblemType::Unknown),
            "general"
        );
    }

    #[test]
    fn test_compute_similarity_identical() {
        let score =
            ProblemRetriever::compute_similarity("tool execution failed", "tool execution failed");
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_similarity_partial_overlap() {
        let score = ProblemRetriever::compute_similarity(
            "tool execution failed",
            "execution timeout error",
        );
        assert!(score > 0.0 && score < 1.0);
    }

    #[test]
    fn test_compute_similarity_no_overlap() {
        let score =
            ProblemRetriever::compute_similarity("tool execution failed", "unrelated topic here");
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_similarity_empty() {
        assert_eq!(ProblemRetriever::compute_similarity("", "some text"), 0.0);
        assert_eq!(ProblemRetriever::compute_similarity("some text", ""), 0.0);
    }

    #[test]
    fn test_extract_resolution_steps_from_array() {
        let outcomes = serde_json::json!(["step 1", "step 2", "step 3"]);
        let steps = ProblemRetriever::extract_resolution_steps(&outcomes);
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn test_extract_resolution_steps_from_object() {
        let outcomes = serde_json::json!({"steps": ["retry", "fallback"]});
        let steps = ProblemRetriever::extract_resolution_steps(&outcomes);
        assert_eq!(steps.len(), 2);
    }

    #[test]
    fn test_estimate_confidence_empty() {
        let confidence = ProblemRetriever::estimate_confidence(&[]);
        assert!((confidence - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_estimate_confidence_with_successes() {
        let cases = vec![SimilarProblemCase {
            problem_id: "1".to_string(),
            problem_description: "test".to_string(),
            similarity_score: 0.9,
            resolution_steps: vec![],
            outcome: ProblemOutcome::Success,
            resolution_context: "test".to_string(),
        }];
        let confidence = ProblemRetriever::estimate_confidence(&cases);
        assert!(confidence > 0.5);
    }

    #[test]
    fn test_generate_adaptation_notes_empty() {
        let notes = ProblemRetriever::generate_adaptation_notes(&ProblemType::Unknown, &[]);
        assert!(notes.iter().any(|n| n.contains("No similar problems")));
    }

    #[test]
    fn test_problem_retrieval_result_construction() {
        let result = ProblemRetrievalResult {
            request_id: Uuid::new_v4().to_string(),
            original_problem: "test problem".to_string(),
            inferred_problem_type: ProblemType::ToolExecutionFailure,
            similar_problems: vec![],
            recommended_steps: vec![],
            adaptation_notes: vec!["note".to_string()],
            confidence: 0.5,
        };
        assert_eq!(
            result.inferred_problem_type,
            ProblemType::ToolExecutionFailure
        );
        assert_eq!(result.adaptation_notes.len(), 1);
    }
}
