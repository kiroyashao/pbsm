use std::sync::Arc;

use super::{
    CollectingEventPublisher, CollectingIntentionStackEventPublisher,
    CollectingMemoryEventPublisher, CollectingMetacognitiveEventPublisher,
};
use pbsm_core::modules::common::PredictionEvent;
use pbsm_core::modules::intention_stack::events::IntentionStackEvent;
use pbsm_core::modules::memory::events::MemoryEvent;
use pbsm_core::modules::metacognition::events::MetacognitiveEvent;

pub struct CrossModuleEventBus {
    prediction: Arc<CollectingEventPublisher>,
    metacognitive: Arc<CollectingMetacognitiveEventPublisher>,
    intention_stack: Arc<CollectingIntentionStackEventPublisher>,
    memory: Arc<CollectingMemoryEventPublisher>,
}

impl CrossModuleEventBus {
    pub fn new() -> Self {
        Self {
            prediction: Arc::new(CollectingEventPublisher::new()),
            metacognitive: Arc::new(CollectingMetacognitiveEventPublisher::new()),
            intention_stack: Arc::new(CollectingIntentionStackEventPublisher::new()),
            memory: Arc::new(CollectingMemoryEventPublisher::new()),
        }
    }

    pub fn prediction_publisher(&self) -> Arc<CollectingEventPublisher> {
        Arc::clone(&self.prediction)
    }

    pub fn metacognitive_publisher(&self) -> Arc<CollectingMetacognitiveEventPublisher> {
        Arc::clone(&self.metacognitive)
    }

    pub fn intention_stack_publisher(&self) -> Arc<CollectingIntentionStackEventPublisher> {
        Arc::clone(&self.intention_stack)
    }

    pub fn memory_publisher(&self) -> Arc<CollectingMemoryEventPublisher> {
        Arc::clone(&self.memory)
    }

    pub fn total_event_count(&self) -> usize {
        self.prediction.count()
            + self.metacognitive.count()
            + self.intention_stack.count()
            + self.memory.count()
    }

    pub fn clear_all(&self) {
        self.prediction.clear();
        self.metacognitive.clear();
        self.intention_stack.clear();
        self.memory.clear();
    }

    pub fn has_prediction_event(&self, event_type: &str) -> bool {
        self.prediction.events().iter().any(|e| match e {
            PredictionEvent::PredictionCreated(_) => event_type == "PredictionCreated",
            PredictionEvent::PredictionVerified(_) => event_type == "PredictionVerified",
            PredictionEvent::PredictionFalsified(_) => event_type == "PredictionFalsified",
            PredictionEvent::ResidualComputed(_) => event_type == "ResidualComputed",
            PredictionEvent::WarningResidualDetected(_) => event_type == "WarningResidualDetected",
            PredictionEvent::ErrorResidualDetected(_) => event_type == "ErrorResidualDetected",
            PredictionEvent::CriticalResidualDetected(_) => {
                event_type == "CriticalResidualDetected"
            }
        })
    }

    pub fn has_metacognitive_event(&self, event_type: &str) -> bool {
        self.metacognitive
            .events()
            .iter()
            .any(|e| e.event_type_name() == event_type)
    }

    pub fn has_intention_stack_event(&self, event_type: &str) -> bool {
        self.intention_stack
            .events()
            .iter()
            .any(|e| e.event_type_name() == event_type)
    }

    pub fn has_memory_event(&self, event_type: &str) -> bool {
        self.memory
            .events()
            .iter()
            .any(|e| e.event_type == event_type)
    }

    pub fn verify_chain(
        &self,
        from_module: &str,
        from_event: &str,
        to_module: &str,
        to_event: &str,
    ) -> bool {
        let from_has = match from_module {
            "prediction" => self.has_prediction_event(from_event),
            "metacognitive" => self.has_metacognitive_event(from_event),
            "intention_stack" => self.has_intention_stack_event(from_event),
            "memory" => self.has_memory_event(from_event),
            _ => false,
        };
        let to_has = match to_module {
            "prediction" => self.has_prediction_event(to_event),
            "metacognitive" => self.has_metacognitive_event(to_event),
            "intention_stack" => self.has_intention_stack_event(to_event),
            "memory" => self.has_memory_event(to_event),
            _ => false,
        };
        from_has && to_has
    }

    pub fn prediction_events(&self) -> Vec<PredictionEvent> {
        self.prediction.events()
    }

    pub fn metacognitive_events(&self) -> Vec<MetacognitiveEvent> {
        self.metacognitive.events()
    }

    pub fn intention_stack_events(&self) -> Vec<IntentionStackEvent> {
        self.intention_stack.events()
    }

    pub fn memory_events(&self) -> Vec<MemoryEvent> {
        self.memory.events()
    }
}

impl Default for CrossModuleEventBus {
    fn default() -> Self {
        Self::new()
    }
}
