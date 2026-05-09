pub mod mock_event_bus;

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::Utc;
use parking_lot::Mutex;
use serde_json::Value;
use uuid::Uuid;

use pbsm_core::modules::common::{
    BeliefGraphError, BeliefGraphReader, BeliefGraphWriter, BeliefNode, BeliefQuerySpec,
    BeliefState, EventPublishError, EventPublisher, PredictionCreatedPayload, PredictionEvent,
    PredictionFalsifiedPayload, PredictionVerifiedPayload, RelationEdge,
};
use pbsm_core::modules::intention_stack::events::{
    IntentionStackEvent, IntentionStackEventPublisher,
};
use pbsm_core::modules::memory::config::MemoryConfig;
use pbsm_core::modules::memory::events::{MemoryEvent, MemoryEventPublisher};
use pbsm_core::modules::memory::types::CompressionType;
use pbsm_core::modules::metacognition::events::{MetacognitiveEvent, MetacognitiveEventPublisher};
use pbsm_core::types::prediction::{ActionRequest, ActionType, Observation};

pub struct MockBeliefGraphReader {
    nodes: Mutex<HashMap<String, BeliefNode>>,
    edges: Mutex<Vec<RelationEdge>>,
}

impl MockBeliefGraphReader {
    pub fn new() -> Self {
        Self {
            nodes: Mutex::new(HashMap::new()),
            edges: Mutex::new(Vec::new()),
        }
    }

    pub fn add_node(&self, node: BeliefNode) {
        self.nodes.lock().insert(node.node_id.clone(), node);
    }

    pub fn add_edge(&self, edge: RelationEdge) {
        self.edges.lock().push(edge);
    }

    pub fn set_nodes(&self, nodes: Vec<BeliefNode>) {
        let mut map = HashMap::new();
        for node in nodes {
            map.insert(node.node_id.clone(), node);
        }
        *self.nodes.lock() = map;
    }

    pub fn set_edges(&self, edges: Vec<RelationEdge>) {
        *self.edges.lock() = edges;
    }
}

impl Default for MockBeliefGraphReader {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BeliefGraphReader for MockBeliefGraphReader {
    async fn query_belief_by_id(
        &self,
        node_id: &str,
    ) -> Result<Option<BeliefNode>, BeliefGraphError> {
        Ok(self.nodes.lock().get(node_id).cloned())
    }

    async fn query_beliefs(
        &self,
        query_spec: BeliefQuerySpec,
    ) -> Result<Vec<BeliefNode>, BeliefGraphError> {
        let nodes = self.nodes.lock();
        let mut result: Vec<BeliefNode> = nodes
            .values()
            .filter(|n| {
                if let Some(ref node_type) = query_spec.node_type {
                    if n.node_type != *node_type {
                        return false;
                    }
                }
                if let Some(threshold) = query_spec.confidence_threshold {
                    if n.confidence < threshold {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();
        result.sort_by(|a, b| a.node_id.cmp(&b.node_id));
        Ok(result)
    }

    async fn get_belief_state(
        &self,
        belief_ids: &[String],
    ) -> Result<BeliefState, BeliefGraphError> {
        let nodes = self.nodes.lock();
        let edges = self.edges.lock();
        let filtered_nodes: Vec<BeliefNode> = belief_ids
            .iter()
            .filter_map(|id| nodes.get(id).cloned())
            .collect();
        let filtered_edges: Vec<RelationEdge> = edges
            .iter()
            .filter(|e| belief_ids.contains(&e.source_node) || belief_ids.contains(&e.target_node))
            .cloned()
            .collect();
        Ok(BeliefState {
            nodes: filtered_nodes,
            edges: filtered_edges,
            hash: format!("mock_{}", Uuid::new_v4()),
        })
    }

    async fn get_outgoing_edges(
        &self,
        node_id: &str,
    ) -> Result<Vec<RelationEdge>, BeliefGraphError> {
        let edges = self.edges.lock();
        Ok(edges
            .iter()
            .filter(|e| e.source_node == node_id)
            .cloned()
            .collect())
    }
}

pub struct MockBeliefGraphWriter {
    confidence_updates: Mutex<Vec<(String, String, f64)>>,
    revision_marks: Mutex<Vec<(String, String)>>,
}

impl MockBeliefGraphWriter {
    pub fn new() -> Self {
        Self {
            confidence_updates: Mutex::new(Vec::new()),
            revision_marks: Mutex::new(Vec::new()),
        }
    }

    pub fn get_confidence_updates(&self) -> Vec<(String, String, f64)> {
        self.confidence_updates.lock().clone()
    }

    pub fn get_revision_marks(&self) -> Vec<(String, String)> {
        self.revision_marks.lock().clone()
    }
}

impl Default for MockBeliefGraphWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BeliefGraphWriter for MockBeliefGraphWriter {
    async fn update_belief_confidence(
        &self,
        node_id: &str,
        attribute: &str,
        new_confidence: f64,
    ) -> Result<(), BeliefGraphError> {
        self.confidence_updates.lock().push((
            node_id.to_string(),
            attribute.to_string(),
            new_confidence,
        ));
        Ok(())
    }

    async fn mark_belief_for_revision(
        &self,
        belief_id: &str,
        reason: &str,
    ) -> Result<(), BeliefGraphError> {
        self.revision_marks
            .lock()
            .push((belief_id.to_string(), reason.to_string()));
        Ok(())
    }
}

pub trait ExtractEventPayload: Clone + 'static {
    fn extract_from(event: &PredictionEvent) -> Option<Self>;
}

impl ExtractEventPayload for PredictionCreatedPayload {
    fn extract_from(event: &PredictionEvent) -> Option<Self> {
        match event {
            PredictionEvent::PredictionCreated(p) => Some(p.clone()),
            _ => None,
        }
    }
}

impl ExtractEventPayload for PredictionVerifiedPayload {
    fn extract_from(event: &PredictionEvent) -> Option<Self> {
        match event {
            PredictionEvent::PredictionVerified(p) => Some(p.clone()),
            _ => None,
        }
    }
}

impl ExtractEventPayload for PredictionFalsifiedPayload {
    fn extract_from(event: &PredictionEvent) -> Option<Self> {
        match event {
            PredictionEvent::PredictionFalsified(p) => Some(p.clone()),
            _ => None,
        }
    }
}

pub struct CollectingEventPublisher {
    events: Mutex<Vec<PredictionEvent>>,
}

impl CollectingEventPublisher {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    pub fn events(&self) -> Vec<PredictionEvent> {
        self.events.lock().clone()
    }

    pub fn events_of_type<T: ExtractEventPayload>(&self) -> Vec<T> {
        self.events
            .lock()
            .iter()
            .filter_map(|e| T::extract_from(e))
            .collect()
    }

    pub fn clear(&self) {
        self.events.lock().clear();
    }

    pub fn count(&self) -> usize {
        self.events.lock().len()
    }
}

impl Default for CollectingEventPublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl EventPublisher for CollectingEventPublisher {
    fn publish_event(&self, event: PredictionEvent) -> Result<(), EventPublishError> {
        self.events.lock().push(event);
        Ok(())
    }
}

pub struct CollectingMetacognitiveEventPublisher {
    events: Mutex<Vec<MetacognitiveEvent>>,
}

impl CollectingMetacognitiveEventPublisher {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    pub fn events(&self) -> Vec<MetacognitiveEvent> {
        self.events.lock().clone()
    }

    pub fn clear(&self) {
        self.events.lock().clear();
    }

    pub fn count(&self) -> usize {
        self.events.lock().len()
    }
}

impl Default for CollectingMetacognitiveEventPublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl MetacognitiveEventPublisher for CollectingMetacognitiveEventPublisher {
    fn publish(&self, event: MetacognitiveEvent) -> Result<(), String> {
        self.events.lock().push(event);
        Ok(())
    }
}

pub struct CollectingIntentionStackEventPublisher {
    events: Mutex<Vec<IntentionStackEvent>>,
}

impl CollectingIntentionStackEventPublisher {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    pub fn events(&self) -> Vec<IntentionStackEvent> {
        self.events.lock().clone()
    }

    pub fn clear(&self) {
        self.events.lock().clear();
    }

    pub fn count(&self) -> usize {
        self.events.lock().len()
    }
}

impl Default for CollectingIntentionStackEventPublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl IntentionStackEventPublisher for CollectingIntentionStackEventPublisher {
    fn publish(&self, event: IntentionStackEvent) -> Result<(), String> {
        self.events.lock().push(event);
        Ok(())
    }
}

pub struct CollectingMemoryEventPublisher {
    events: Mutex<Vec<MemoryEvent>>,
}

impl CollectingMemoryEventPublisher {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    pub fn events(&self) -> Vec<MemoryEvent> {
        self.events.lock().clone()
    }

    pub fn clear(&self) {
        self.events.lock().clear();
    }

    pub fn count(&self) -> usize {
        self.events.lock().len()
    }
}

impl Default for CollectingMemoryEventPublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryEventPublisher for CollectingMemoryEventPublisher {
    fn publish(
        &self,
        event: MemoryEvent,
    ) -> Result<(), pbsm_core::modules::memory::events::EventPublishError> {
        self.events.lock().push(event);
        Ok(())
    }
}

pub fn make_test_belief_node(id: &str, node_type: &str, confidence: f64) -> BeliefNode {
    let now = Utc::now();
    BeliefNode {
        node_id: id.to_string(),
        node_type: node_type.to_string(),
        attributes: serde_json::json!({}),
        confidence,
        created_at: now,
        updated_at: now,
    }
}

pub fn make_test_relation_edge(
    id: Uuid,
    source: &str,
    target: &str,
    edge_type: &str,
    confidence: f64,
) -> RelationEdge {
    RelationEdge {
        edge_id: id,
        source_node: source.to_string(),
        target_node: target.to_string(),
        edge_type: edge_type.to_string(),
        confidence,
    }
}

pub fn make_test_action_request(
    action_type: ActionType,
    action_name: &str,
    target_id: Option<String>,
) -> ActionRequest {
    ActionRequest {
        action_type,
        action_name: action_name.to_string(),
        parameters: serde_json::json!({}),
        target_id,
    }
}

pub fn make_test_observation(format: &str, data: Value, source: &str) -> Observation {
    Observation {
        format: format.to_string(),
        data,
        timestamp: Utc::now(),
        source: source.to_string(),
    }
}

pub fn temp_memory_config() -> MemoryConfig {
    let temp_dir = std::env::temp_dir().join(format!("pbsm_test_{}", Uuid::new_v4()));
    MemoryConfig {
        storage_path: temp_dir,
        cache_size: 10,
        max_log_age_days: 1,
        compression_type: CompressionType::None,
        max_recent_sessions: 5,
        base_confidence_threshold: 0.5,
        cleanup_auto_trigger_threshold: 0.9,
        retrieval_default_limit: 10,
        importance_retention_bonus: 1.0,
        archive_threshold_days: 1,
    }
}
