use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::event_bus::{
    IntentionStackEventAdapter, MemoryEventAdapter, MetacognitiveEventAdapter,
    PredictionEventAdapter, SystemEventBus,
};
use crate::modules::belief_graph::graph::BeliefGraph;
use crate::modules::belief_graph::types::GraphConfig;
use crate::modules::common::{BeliefGraphReader, EventPublisher};
use crate::modules::intention_stack::config::IntentionStackConfig;
use crate::modules::intention_stack::manager::IntentionStackManager;
use crate::modules::intention_stack::manager::IntentionStackManagerImpl;
use crate::modules::intention_stack::state::GoalPriority;
use crate::modules::intention_stack::types::{
    GoalDefinition, PushIntentRequest, PushIntentResponse, TargetState,
};
use crate::modules::memory::config::MemoryConfig;
use crate::modules::memory::store::ExternalMemoryStore;
use crate::modules::metacognition::config::MetacognitiveConfig;
use crate::modules::metacognition::controller::MetacognitiveController;
use crate::modules::metacognition::types::{
    AnomalySeverity, AnomalyType, GetAnomalyReportResponse, GetAttentionStatusResponse,
    GetForgetStatusResponse, TriggerInterventionRequest, TriggerInterventionResponse,
};
use crate::modules::prediction_engine::PredictionEngine;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PbsmConfig {
    pub graph: GraphConfig,
    pub metacognitive: MetacognitiveConfig,
    pub memory: MemoryConfig,
    pub intention_stack: IntentionStackConfig,
}

impl PbsmConfig {
    pub fn validate(&self) -> Result<(), String> {
        self.intention_stack
            .validate()
            .map_err(|e| format!("intention_stack config invalid: {:?}", e))?;
        self.metacognitive
            .validate()
            .map_err(|e| format!("metacognitive config invalid: {:?}", e))?;
        if self.graph.max_nodes == 0 {
            return Err("graph.max_nodes must be > 0".to_string());
        }
        if self.graph.max_edges == 0 {
            return Err("graph.max_edges must be > 0".to_string());
        }
        if !(0.0..=1.0).contains(&self.graph.default_confidence) {
            return Err("graph.default_confidence must be in [0, 1]".to_string());
        }
        Ok(())
    }

    pub fn load_from_toml(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file {:?}: {}", path, e))?;
        let config: Self =
            toml::from_str(&content).map_err(|e| format!("Failed to parse TOML config: {}", e))?;
        config.validate()?;
        Ok(config)
    }

    pub fn load_from_json(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file {:?}: {}", path, e))?;
        let config: Self = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse JSON config: {}", e))?;
        config.validate()?;
        Ok(config)
    }

    pub fn save_to_toml(&self, path: &Path) -> Result<(), String> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config to TOML: {}", e))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
        std::fs::write(path, content)
            .map_err(|e| format!("Failed to write config file {:?}: {}", path, e))?;
        Ok(())
    }

    pub fn save_to_json(&self, path: &Path) -> Result<(), String> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config to JSON: {}", e))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
        std::fs::write(path, content)
            .map_err(|e| format!("Failed to write config file {:?}: {}", path, e))?;
        Ok(())
    }

    pub fn from_toml_str(s: &str) -> Result<Self, String> {
        let config: Self =
            toml::from_str(s).map_err(|e| format!("Failed to parse TOML config: {}", e))?;
        config.validate()?;
        Ok(config)
    }

    pub fn from_json_str(s: &str) -> Result<Self, String> {
        let config: Self =
            serde_json::from_str(s).map_err(|e| format!("Failed to parse JSON config: {}", e))?;
        config.validate()?;
        Ok(config)
    }
}

pub struct PbsmOrchestrator {
    belief_graph: Arc<BeliefGraph>,
    prediction_engine: Arc<PredictionEngine>,
    metacognitive_controller: Arc<MetacognitiveController>,
    memory_store: Option<Arc<ExternalMemoryStore>>,
    intention_stack: Arc<IntentionStackManagerImpl>,
    event_bus: Arc<SystemEventBus>,
    config: PbsmConfig,
}

impl PbsmOrchestrator {
    pub fn new(config: PbsmConfig) -> Self {
        let event_bus = Arc::new(SystemEventBus::default());

        let belief_graph = Arc::new(BeliefGraph::new(config.graph.clone()));

        let prediction_event_adapter: Arc<dyn EventPublisher> =
            Arc::new(PredictionEventAdapter::new(event_bus.clone()));
        let prediction_engine = Arc::new(PredictionEngine::with_components(
            belief_graph.clone() as Arc<dyn BeliefGraphReader>,
            prediction_event_adapter,
        ));

        let metacognitive_event_adapter = MetacognitiveEventAdapter::new(event_bus.clone());
        let metacognitive_controller = Arc::new(MetacognitiveController::with_components(
            config.metacognitive.clone(),
            Arc::new(metacognitive_event_adapter),
        ));

        let intention_stack_event_adapter = IntentionStackEventAdapter::new(event_bus.clone());
        let intention_stack = Arc::new(IntentionStackManagerImpl::with_event_publisher(
            "main".to_string(),
            config.intention_stack.clone(),
            Arc::new(intention_stack_event_adapter),
        ));

        Self {
            belief_graph,
            prediction_engine,
            metacognitive_controller,
            memory_store: None,
            intention_stack,
            event_bus,
            config,
        }
    }

    pub async fn with_memory(mut self) -> Result<Self, String> {
        let memory_event_adapter = MemoryEventAdapter::new(self.event_bus.clone());
        let store = ExternalMemoryStore::open_with_publisher(
            self.config.memory.clone(),
            Arc::new(memory_event_adapter),
        )
        .await
        .map_err(|e| format!("Failed to open memory store: {:?}", e))?;
        self.memory_store = Some(Arc::new(store));
        Ok(self)
    }

    pub fn belief_graph(&self) -> &BeliefGraph {
        &self.belief_graph
    }

    pub fn prediction_engine(&self) -> &PredictionEngine {
        &self.prediction_engine
    }

    pub fn metacognitive_controller(&self) -> &MetacognitiveController {
        &self.metacognitive_controller
    }

    pub fn memory_store(&self) -> Option<&Arc<ExternalMemoryStore>> {
        self.memory_store.as_ref()
    }

    pub fn intention_stack(&self) -> &IntentionStackManagerImpl {
        &self.intention_stack
    }

    pub fn event_bus(&self) -> &SystemEventBus {
        &self.event_bus
    }

    pub fn config(&self) -> &PbsmConfig {
        &self.config
    }

    pub async fn start_task(
        &self,
        description: String,
        target_state: Option<TargetState>,
    ) -> Result<PushIntentResponse, String> {
        let goal = if let Some(ts) = target_state {
            let mut g = GoalDefinition::simple(description, GoalPriority::Medium);
            g.target_state = ts;
            g
        } else {
            GoalDefinition::simple(description, GoalPriority::Medium)
        };

        let request = PushIntentRequest {
            goal,
            plan: None,
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };

        self.intention_stack
            .push_intention(request)
            .await
            .map_err(|e| format!("Failed to start task: {:?}", e))
    }

    pub async fn execute_cycle(&self) -> Result<ExecuteCycleResult, String> {
        let mut result = ExecuteCycleResult::default();

        let attention_status: GetAttentionStatusResponse =
            self.metacognitive_controller().get_attention_status().await;
        result.attention_mode = Some(attention_status.current_mode.clone());

        let predictions = self.prediction_engine().get_active_predictions(None);
        result.active_predictions = predictions.total;

        let forget_status: GetForgetStatusResponse =
            self.metacognitive_controller().get_forget_status();
        result.pending_forget_count = forget_status.pending_forgets.len();

        Ok(result)
    }

    pub fn handle_error(
        &self,
        error_description: String,
        severity: AnomalySeverity,
    ) -> Result<HandleErrorResult, String> {
        let anomaly_report: GetAnomalyReportResponse =
            self.metacognitive_controller().detect_anomalies(None);

        let anomaly_type = match severity {
            AnomalySeverity::High => AnomalyType::ExcessiveFocus,
            AnomalySeverity::Medium => AnomalyType::Drift,
            _ => AnomalyType::Oscillation,
        };

        let intervention_result: TriggerInterventionResponse = self
            .metacognitive_controller()
            .trigger_intervention(TriggerInterventionRequest {
                anomaly_type: Some(anomaly_type),
                force_level: Some(severity),
            })
            .map_err(|e| format!("Failed to trigger intervention: {:?}", e))?;

        Ok(HandleErrorResult {
            error_description,
            anomaly_count: anomaly_report.anomalies.len(),
            intervention_applied: !intervention_result.interventions.is_empty(),
        })
    }

    pub fn memory_footprint(&self) -> MemoryFootprint {
        MemoryFootprint {
            belief_graph_nodes: self.belief_graph.node_count(),
            belief_graph_edges: self.belief_graph.edge_count(),
            event_bus_history: self.event_bus.history_len(),
            event_bus_receivers: self.event_bus.receiver_count(),
            has_memory_store: self.memory_store.is_some(),
        }
    }

    pub fn consistency_check(&self) -> ConsistencyReport {
        let mut issues: Vec<ConsistencyIssue> = Vec::new();

        let nodes = self.belief_graph.nodes().read();
        let edges = self.belief_graph.edges().read();
        let adjacency = self.belief_graph.adjacency_mut().read();

        for (edge_id, edge) in edges.iter() {
            if !nodes.contains_key(&edge.source_node) {
                issues.push(ConsistencyIssue {
                    severity: IssueSeverity::Error,
                    component: "belief_graph".to_string(),
                    description: format!(
                        "Edge {} references non-existent source node {}",
                        edge_id, edge.source_node
                    ),
                });
            }
            if !nodes.contains_key(&edge.target_node) {
                issues.push(ConsistencyIssue {
                    severity: IssueSeverity::Error,
                    component: "belief_graph".to_string(),
                    description: format!(
                        "Edge {} references non-existent target node {}",
                        edge_id, edge.target_node
                    ),
                });
            }
        }

        for (node_id, node) in nodes.iter() {
            for edge_id in &node.outgoing_edges {
                if !edges.contains_key(edge_id) {
                    issues.push(ConsistencyIssue {
                        severity: IssueSeverity::Warning,
                        component: "belief_graph".to_string(),
                        description: format!(
                            "Node {} references non-existent outgoing edge {}",
                            node_id, edge_id
                        ),
                    });
                }
            }
            for edge_id in &node.incoming_edges {
                if !edges.contains_key(edge_id) {
                    issues.push(ConsistencyIssue {
                        severity: IssueSeverity::Warning,
                        component: "belief_graph".to_string(),
                        description: format!(
                            "Node {} references non-existent incoming edge {}",
                            node_id, edge_id
                        ),
                    });
                }
            }
        }

        for (node_id, outgoing) in &adjacency.outgoing {
            if !nodes.contains_key(node_id) {
                issues.push(ConsistencyIssue {
                    severity: IssueSeverity::Warning,
                    component: "belief_graph".to_string(),
                    description: format!(
                        "Adjacency list contains non-existent source node {}",
                        node_id
                    ),
                });
            }
            for (edge_id, target_id) in outgoing {
                if !edges.contains_key(edge_id) {
                    issues.push(ConsistencyIssue {
                        severity: IssueSeverity::Warning,
                        component: "belief_graph".to_string(),
                        description: format!(
                            "Adjacency outgoing references non-existent edge {}",
                            edge_id
                        ),
                    });
                }
                if !nodes.contains_key(target_id) {
                    issues.push(ConsistencyIssue {
                        severity: IssueSeverity::Warning,
                        component: "belief_graph".to_string(),
                        description: format!(
                            "Adjacency outgoing references non-existent target node {}",
                            target_id
                        ),
                    });
                }
            }
        }

        if self.belief_graph.node_count() > self.config.graph.max_nodes {
            issues.push(ConsistencyIssue {
                severity: IssueSeverity::Error,
                component: "belief_graph".to_string(),
                description: format!(
                    "Node count {} exceeds max_nodes {}",
                    self.belief_graph.node_count(),
                    self.config.graph.max_nodes
                ),
            });
        }

        if self.belief_graph.edge_count() > self.config.graph.max_edges {
            issues.push(ConsistencyIssue {
                severity: IssueSeverity::Error,
                component: "belief_graph".to_string(),
                description: format!(
                    "Edge count {} exceeds max_edges {}",
                    self.belief_graph.edge_count(),
                    self.config.graph.max_edges
                ),
            });
        }

        let error_count = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Error)
            .count();
        let warning_count = issues
            .iter()
            .filter(|i| i.severity == IssueSeverity::Warning)
            .count();

        ConsistencyReport {
            is_consistent: error_count == 0,
            error_count,
            warning_count,
            issues,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueSeverity {
    Warning,
    Error,
}

#[derive(Debug)]
pub struct ConsistencyIssue {
    pub severity: IssueSeverity,
    pub component: String,
    pub description: String,
}

#[derive(Debug)]
pub struct ConsistencyReport {
    pub is_consistent: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub issues: Vec<ConsistencyIssue>,
}

#[derive(Debug)]
pub struct MemoryFootprint {
    pub belief_graph_nodes: usize,
    pub belief_graph_edges: usize,
    pub event_bus_history: usize,
    pub event_bus_receivers: usize,
    pub has_memory_store: bool,
}

#[derive(Debug, Default)]
pub struct ExecuteCycleResult {
    pub attention_mode: Option<String>,
    pub active_predictions: usize,
    pub pending_forget_count: usize,
}

#[derive(Debug)]
pub struct HandleErrorResult {
    pub error_description: String,
    pub anomaly_count: usize,
    pub intervention_applied: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_creation() {
        let config = PbsmConfig::default();
        let orchestrator = PbsmOrchestrator::new(config);
        assert!(orchestrator.belief_graph().node_count() == 0);
        assert!(orchestrator.memory_store().is_none());
    }

    #[test]
    fn test_orchestrator_with_event_bus() {
        let config = PbsmConfig::default();
        let orchestrator = PbsmOrchestrator::new(config);
        assert_eq!(orchestrator.event_bus().receiver_count(), 0);
    }

    #[tokio::test]
    async fn test_start_task() {
        let config = PbsmConfig::default();
        let orchestrator = PbsmOrchestrator::new(config);

        let result = orchestrator.start_task("Test task".to_string(), None).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(!response.layer_id.is_empty());
    }

    #[tokio::test]
    async fn test_execute_cycle() {
        let config = PbsmConfig::default();
        let orchestrator = PbsmOrchestrator::new(config);

        let result = orchestrator.execute_cycle().await;
        assert!(result.is_ok());

        let cycle_result = result.unwrap();
        assert_eq!(cycle_result.active_predictions, 0);
    }

    #[test]
    fn test_handle_error() {
        let config = PbsmConfig::default();
        let orchestrator = PbsmOrchestrator::new(config);

        let result = orchestrator.handle_error("Test error".to_string(), AnomalySeverity::Medium);

        assert!(result.is_ok());
        let handle_result = result.unwrap();
        assert_eq!(handle_result.error_description, "Test error");
    }

    #[test]
    fn test_config_default() {
        let config = PbsmConfig::default();
        assert_eq!(config.graph.max_nodes, GraphConfig::default().max_nodes);
    }

    #[test]
    fn test_config_validate_default() {
        let config = PbsmConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validate_invalid_graph() {
        let mut config = PbsmConfig::default();
        config.graph.max_nodes = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_toml_roundtrip() {
        let config = PbsmConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: PbsmConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.graph.max_nodes, parsed.graph.max_nodes);
        assert_eq!(config.graph.max_edges, parsed.graph.max_edges);
    }

    #[test]
    fn test_config_json_roundtrip() {
        let config = PbsmConfig::default();
        let json_str = serde_json::to_string_pretty(&config).unwrap();
        let parsed: PbsmConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(config.graph.max_nodes, parsed.graph.max_nodes);
        assert_eq!(
            config.intention_stack.max_stack_depth,
            parsed.intention_stack.max_stack_depth
        );
    }

    #[test]
    fn test_config_from_toml_str() {
        let toml_str = r#"
[graph]
maxNodes = 1000
maxEdges = 5000
defaultConfidence = 0.8

[intention_stack]
max_stack_depth = 30
max_stack_capacity = 800
max_revert_depth = 10
root_visibility_threshold = 0.7
default_step_timeout = 60000
default_max_retries = 5
max_checkpoints_per_layer = 30

[intention_stack.drift_threshold]
warning = 0.3
moderate = 0.5
severe = 0.7
critical = 0.9

[metacognitive]

[metacognitive.attention]
min_attention = 0.1
max_attention = 1.0
default_attention = 0.5
decay_rate = 0.05
boost_step = 0.4
time_decay_rate = 0.001
max_adjustment = 0.3
min_adjustment_interval_ms = 100

[metacognitive.value_evaluation]
recency_decay_lambda = 0.05
access_window_size = 50
max_access_threshold = 10

[metacognitive.value_evaluation.weights]
goal_relevance_weight = 0.35
access_frequency_weight = 0.25
recency_weight = 0.20
residual_weight = 0.20

[metacognitive.forgetting]
forget_threshold = 0.2
max_active_beliefs = 500
min_survival_steps = 10
forget_cooldown_steps = 20
max_defer_steps = 200
batch_forgive_interval = 50
residual_defer_threshold = 0.7

[metacognitive.anomaly_detection]
coverage_threshold = 0.3
oscillation_threshold = 5
drift_threshold = 0.2
lock_threshold = 100
anomaly_check_interval = 25
anomaly_history_size = 100

[memory]
storagePath = "/tmp/pbsm-test"
cacheSize = 1000
maxLogAgeDays = 30
compressionType = "NONE"
maxRecentSessions = 10
baseConfidenceThreshold = 0.3
cleanupAutoTriggerThreshold = 0.8
retrievalDefaultLimit = 50
importanceRetentionBonus = 0.1
archiveThresholdDays = 90
"#;
        let config = PbsmConfig::from_toml_str(toml_str).unwrap();
        assert_eq!(config.graph.max_nodes, 1000);
        assert_eq!(config.intention_stack.max_stack_depth, 30);
    }

    #[test]
    fn test_config_save_load_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let config = PbsmConfig::default();
        config.save_to_toml(&path).unwrap();
        let loaded = PbsmConfig::load_from_toml(&path).unwrap();
        assert_eq!(config.graph.max_nodes, loaded.graph.max_nodes);
    }

    #[test]
    fn test_config_save_load_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let config = PbsmConfig::default();
        config.save_to_json(&path).unwrap();
        let loaded = PbsmConfig::load_from_json(&path).unwrap();
        assert_eq!(config.graph.max_nodes, loaded.graph.max_nodes);
    }

    #[test]
    fn test_consistency_check_empty_graph() {
        let config = PbsmConfig::default();
        let orchestrator = PbsmOrchestrator::new(config);
        let report = orchestrator.consistency_check();
        assert!(report.is_consistent);
        assert_eq!(report.error_count, 0);
        assert_eq!(report.warning_count, 0);
    }

    #[test]
    fn test_memory_footprint() {
        let config = PbsmConfig::default();
        let orchestrator = PbsmOrchestrator::new(config);
        let fp = orchestrator.memory_footprint();
        assert_eq!(fp.belief_graph_nodes, 0);
        assert_eq!(fp.belief_graph_edges, 0);
        assert!(!fp.has_memory_store);
    }
}
