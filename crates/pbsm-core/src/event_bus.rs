use std::collections::VecDeque;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::broadcast;

use crate::modules::common::PredictionEvent;
use crate::modules::intention_stack::events::IntentionStackEvent;
use crate::modules::memory::events::MemoryEvent;
use crate::modules::metacognition::events::MetacognitiveEvent;

pub const DEFAULT_EVENT_BUS_CAPACITY: usize = 1024;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum SystemEvent {
    Prediction(PredictionEvent),
    Metacognitive(MetacognitiveEvent),
    Memory(MemoryEvent),
    IntentionStack(IntentionStackEvent),
}

impl SystemEvent {
    pub fn event_type_name(&self) -> &str {
        match self {
            SystemEvent::Prediction(e) => match e {
                PredictionEvent::PredictionCreated(_) => "prediction.created",
                PredictionEvent::PredictionVerified(_) => "prediction.verified",
                PredictionEvent::PredictionFalsified(_) => "prediction.falsified",
                PredictionEvent::ResidualComputed(_) => "prediction.residualComputed",
                PredictionEvent::WarningResidualDetected(_) => "prediction.warningResidual",
                PredictionEvent::ErrorResidualDetected(_) => "prediction.errorResidual",
                PredictionEvent::CriticalResidualDetected(_) => "prediction.criticalResidual",
            },
            SystemEvent::Metacognitive(e) => e.event_type_name(),
            SystemEvent::Memory(e) => &e.event_type,
            SystemEvent::IntentionStack(e) => e.event_type_name(),
        }
    }

    pub fn source_module(&self) -> &'static str {
        match self {
            SystemEvent::Prediction(_) => "M2",
            SystemEvent::Metacognitive(_) => "M3",
            SystemEvent::Memory(_) => "M4",
            SystemEvent::IntentionStack(_) => "M5",
        }
    }
}

impl From<PredictionEvent> for SystemEvent {
    fn from(event: PredictionEvent) -> Self {
        SystemEvent::Prediction(event)
    }
}

impl From<MetacognitiveEvent> for SystemEvent {
    fn from(event: MetacognitiveEvent) -> Self {
        SystemEvent::Metacognitive(event)
    }
}

impl From<MemoryEvent> for SystemEvent {
    fn from(event: MemoryEvent) -> Self {
        SystemEvent::Memory(event)
    }
}

impl From<IntentionStackEvent> for SystemEvent {
    fn from(event: IntentionStackEvent) -> Self {
        SystemEvent::IntentionStack(event)
    }
}

#[derive(Debug, Clone)]
pub struct SystemEventBus {
    sender: broadcast::Sender<SystemEvent>,
    history: Arc<Mutex<VecDeque<SystemEvent>>>,
    max_history: usize,
}

impl SystemEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            history: Arc::new(Mutex::new(VecDeque::new())),
            max_history: 1000,
        }
    }

    pub fn with_history_capacity(mut self, max_history: usize) -> Self {
        self.max_history = max_history;
        self
    }

    pub fn publish(&self, event: SystemEvent) -> Result<usize, String> {
        let receiver_count = self.sender.send(event.clone()).unwrap_or_default();

        let mut history = self.history.lock();
        if history.len() >= self.max_history {
            history.pop_front();
        }
        history.push_back(event);

        Ok(receiver_count)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SystemEvent> {
        self.sender.subscribe()
    }

    pub fn history(&self) -> Vec<SystemEvent> {
        self.history.lock().iter().cloned().collect()
    }

    pub fn clear_history(&self) {
        self.history.lock().clear();
    }

    pub fn history_len(&self) -> usize {
        self.history.lock().len()
    }

    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for SystemEventBus {
    fn default() -> Self {
        Self::new(DEFAULT_EVENT_BUS_CAPACITY)
    }
}

pub struct PredictionEventAdapter {
    bus: Arc<SystemEventBus>,
}

impl PredictionEventAdapter {
    pub fn new(bus: Arc<SystemEventBus>) -> Self {
        Self { bus }
    }
}

impl crate::modules::common::EventPublisher for PredictionEventAdapter {
    fn publish_event(
        &self,
        event: PredictionEvent,
    ) -> Result<(), crate::modules::common::EventPublishError> {
        self.bus
            .publish(SystemEvent::Prediction(event))
            .map_err(crate::modules::common::EventPublishError::PublishFailed)?;
        Ok(())
    }
}

pub struct MetacognitiveEventAdapter {
    bus: Arc<SystemEventBus>,
}

impl MetacognitiveEventAdapter {
    pub fn new(bus: Arc<SystemEventBus>) -> Self {
        Self { bus }
    }
}

impl crate::modules::metacognition::events::MetacognitiveEventPublisher
    for MetacognitiveEventAdapter
{
    fn publish(&self, event: MetacognitiveEvent) -> Result<(), String> {
        self.bus
            .publish(SystemEvent::Metacognitive(event))
            .map(|_| ())
            .map_err(|e| format!("Failed to publish metacognitive event: {}", e))
    }
}

pub struct MemoryEventAdapter {
    bus: Arc<SystemEventBus>,
}

impl MemoryEventAdapter {
    pub fn new(bus: Arc<SystemEventBus>) -> Self {
        Self { bus }
    }
}

impl crate::modules::memory::events::MemoryEventPublisher for MemoryEventAdapter {
    fn publish(
        &self,
        event: MemoryEvent,
    ) -> Result<(), crate::modules::memory::events::EventPublishError> {
        self.bus
            .publish(SystemEvent::Memory(event))
            .map(|_| ())
            .map_err(|e| {
                crate::modules::memory::events::EventPublishError::PublishFailed(format!(
                    "Failed to publish memory event: {}",
                    e
                ))
            })
    }
}

pub struct IntentionStackEventAdapter {
    bus: Arc<SystemEventBus>,
}

impl IntentionStackEventAdapter {
    pub fn new(bus: Arc<SystemEventBus>) -> Self {
        Self { bus }
    }
}

impl crate::modules::intention_stack::events::IntentionStackEventPublisher
    for IntentionStackEventAdapter
{
    fn publish(&self, event: IntentionStackEvent) -> Result<(), String> {
        self.bus
            .publish(SystemEvent::IntentionStack(event))
            .map(|_| ())
            .map_err(|e| format!("Failed to publish intention stack event: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::common::{EventPublisher, PredictionCreatedPayload, PredictionEvent};
    use crate::modules::intention_stack::events::IntentionStackEventPublisher;
    use crate::modules::memory::events::MemoryEventPublisher;
    use crate::modules::metacognition::events::MetacognitiveEventPublisher;
    use crate::types::prediction::ActionType;

    #[test]
    fn test_event_bus_creation() {
        let bus = SystemEventBus::new(256);
        assert_eq!(bus.receiver_count(), 0);
    }

    #[test]
    fn test_event_bus_default() {
        let bus = SystemEventBus::default();
        assert_eq!(bus.receiver_count(), 0);
    }

    #[test]
    fn test_publish_and_subscribe() {
        let bus = Arc::new(SystemEventBus::new(256));
        let mut rx = bus.subscribe();

        let event = PredictionEvent::PredictionCreated(PredictionCreatedPayload {
            prediction_id: uuid::Uuid::new_v4(),
            action_type: ActionType::ToolCall,
            target_node: None,
            expected_change_count: 1,
        });

        bus.publish(SystemEvent::Prediction(event.clone())).unwrap();

        let received = rx.try_recv().unwrap();
        match received {
            SystemEvent::Prediction(PredictionEvent::PredictionCreated(payload)) => {
                assert_eq!(payload.expected_change_count, 1);
            }
            _ => panic!("Expected PredictionCreated event"),
        }
    }

    #[test]
    fn test_event_history() {
        let bus = SystemEventBus::new(256);

        let event = PredictionEvent::PredictionCreated(PredictionCreatedPayload {
            prediction_id: uuid::Uuid::new_v4(),
            action_type: ActionType::ToolCall,
            target_node: None,
            expected_change_count: 1,
        });

        bus.publish(SystemEvent::Prediction(event)).unwrap();
        assert_eq!(bus.history().len(), 1);

        bus.clear_history();
        assert!(bus.history().is_empty());
    }

    #[test]
    fn test_prediction_event_adapter() {
        let bus = Arc::new(SystemEventBus::new(256));
        let adapter = PredictionEventAdapter::new(bus.clone());
        let mut rx = bus.subscribe();

        let event = PredictionEvent::PredictionCreated(PredictionCreatedPayload {
            prediction_id: uuid::Uuid::new_v4(),
            action_type: ActionType::ToolCall,
            target_node: None,
            expected_change_count: 2,
        });

        adapter.publish_event(event).unwrap();

        let received = rx.try_recv().unwrap();
        match received {
            SystemEvent::Prediction(PredictionEvent::PredictionCreated(p)) => {
                assert_eq!(p.expected_change_count, 2);
            }
            _ => panic!("Expected Prediction event"),
        }
    }

    #[test]
    fn test_metacognitive_event_adapter() {
        let bus = Arc::new(SystemEventBus::new(256));
        let adapter = MetacognitiveEventAdapter::new(bus.clone());
        let mut rx = bus.subscribe();

        let event = MetacognitiveEvent::ForgetTriggered {
            node_ids: vec!["n1".to_string()],
            reason: "low_value".to_string(),
            count: 1,
        };

        adapter.publish(event).unwrap();

        let received = rx.try_recv().unwrap();
        match received {
            SystemEvent::Metacognitive(MetacognitiveEvent::ForgetTriggered { count, .. }) => {
                assert_eq!(count, 1);
            }
            _ => panic!("Expected Metacognitive event"),
        }
    }

    #[test]
    fn test_system_event_source_module() {
        let pe = PredictionEvent::PredictionCreated(PredictionCreatedPayload {
            prediction_id: uuid::Uuid::new_v4(),
            action_type: ActionType::ToolCall,
            target_node: None,
            expected_change_count: 0,
        });
        assert_eq!(SystemEvent::Prediction(pe).source_module(), "M2");

        let me = MetacognitiveEvent::ForgetTriggered {
            node_ids: vec![],
            reason: String::new(),
            count: 0,
        };
        assert_eq!(SystemEvent::Metacognitive(me).source_module(), "M3");
    }

    #[test]
    fn test_history_capacity_limit() {
        let bus = SystemEventBus::new(256).with_history_capacity(3);

        for i in 0..5 {
            let event = PredictionEvent::PredictionCreated(PredictionCreatedPayload {
                prediction_id: uuid::Uuid::new_v4(),
                action_type: ActionType::ToolCall,
                target_node: None,
                expected_change_count: i,
            });
            bus.publish(SystemEvent::Prediction(event)).unwrap();
        }

        assert_eq!(bus.history().len(), 3);
    }
}
