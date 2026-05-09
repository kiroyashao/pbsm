use std::collections::HashMap;

use chrono::Utc;
use uuid::Uuid;

use super::error::{IntentionStackError, Result};
use super::events::{
    IntentionStackEvent, IntentionStackEventPublisher, NullIntentionStackEventPublisher,
};
use super::types::{Checkpoint, IntentionLayer, RestoreResult};
use std::sync::Arc;

pub struct CheckpointManager {
    checkpoints: HashMap<String, Checkpoint>,
    layer_checkpoints: HashMap<String, Vec<String>>,
    max_per_layer: usize,
    event_publisher: Arc<dyn IntentionStackEventPublisher>,
}

impl CheckpointManager {
    pub fn new(max_per_layer: usize) -> Self {
        Self {
            checkpoints: HashMap::new(),
            layer_checkpoints: HashMap::new(),
            max_per_layer,
            event_publisher: Arc::new(NullIntentionStackEventPublisher),
        }
    }

    pub fn with_event_publisher(
        max_per_layer: usize,
        event_publisher: Arc<dyn IntentionStackEventPublisher>,
    ) -> Self {
        Self {
            checkpoints: HashMap::new(),
            layer_checkpoints: HashMap::new(),
            max_per_layer,
            event_publisher,
        }
    }

    pub fn create_checkpoint(
        &mut self,
        layer: &IntentionLayer,
        label: Option<String>,
    ) -> Result<Checkpoint> {
        let layer_id = layer.layer_id.clone();

        let count = self
            .layer_checkpoints
            .get(&layer_id)
            .map(|v| v.len())
            .unwrap_or(0);

        if count >= self.max_per_layer {
            if let Some(ids) = self.layer_checkpoints.get_mut(&layer_id) {
                if let Some(oldest_id) = ids.first().cloned() {
                    self.checkpoints.remove(&oldest_id);
                    ids.remove(0);
                }
            }
        }

        let checkpoint = Checkpoint {
            checkpoint_id: Uuid::new_v4().to_string(),
            layer_id: layer_id.clone(),
            created_at: Utc::now(),
            label,
            state_snapshot: layer.clone(),
        };

        let checkpoint_id = checkpoint.checkpoint_id.clone();
        self.checkpoints
            .insert(checkpoint_id.clone(), checkpoint.clone());

        self.layer_checkpoints
            .entry(layer_id.clone())
            .or_default()
            .push(checkpoint_id.clone());

        let _ = self
            .event_publisher
            .publish(IntentionStackEvent::CheckpointCreated(
                super::events::CheckpointCreatedPayload {
                    checkpoint_id,
                    layer_id,
                    label: checkpoint.label.clone(),
                },
            ));

        Ok(checkpoint)
    }

    pub fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<RestoreResult> {
        let checkpoint = self.checkpoints.get(checkpoint_id).ok_or_else(|| {
            IntentionStackError::CheckpointNotFound {
                checkpoint_id: checkpoint_id.to_string(),
            }
        })?;

        let _ = self
            .event_publisher
            .publish(IntentionStackEvent::CheckpointRestored(
                super::events::CheckpointRestoredPayload {
                    layer_id: checkpoint.layer_id.clone(),
                    layer_index: 0,
                    checkpoint_id: checkpoint_id.to_string(),
                    reverted_layers: 1,
                },
            ));

        Ok(RestoreResult {
            success: true,
            restored_checkpoint_id: checkpoint_id.to_string(),
            layers_restored: 1,
        })
    }

    pub fn get_checkpoint(&self, checkpoint_id: &str) -> Option<&Checkpoint> {
        self.checkpoints.get(checkpoint_id)
    }

    pub fn list_checkpoints(&self, layer_id: &str) -> Vec<&Checkpoint> {
        match self.layer_checkpoints.get(layer_id) {
            Some(ids) => ids
                .iter()
                .filter_map(|id| self.checkpoints.get(id))
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn delete_checkpoint(&mut self, checkpoint_id: &str) -> Result<bool> {
        let checkpoint = self.checkpoints.remove(checkpoint_id).ok_or_else(|| {
            IntentionStackError::CheckpointNotFound {
                checkpoint_id: checkpoint_id.to_string(),
            }
        })?;

        if let Some(ids) = self.layer_checkpoints.get_mut(&checkpoint.layer_id) {
            ids.retain(|id| id != checkpoint_id);
        }

        Ok(true)
    }

    pub fn get_latest_checkpoint(&self, layer_id: &str) -> Option<&Checkpoint> {
        self.layer_checkpoints
            .get(layer_id)
            .and_then(|ids| ids.last())
            .and_then(|id| self.checkpoints.get(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::intention_stack::state::GoalPriority;
    use crate::modules::intention_stack::types::GoalDefinition;

    fn create_test_layer() -> IntentionLayer {
        let goal = GoalDefinition::simple("Test goal".to_string(), GoalPriority::Medium);
        IntentionLayer::new(goal, 0, None)
    }

    #[test]
    fn test_create_checkpoint() {
        let mut manager = CheckpointManager::new(20);
        let layer = create_test_layer();

        let result = manager.create_checkpoint(&layer, Some("test".to_string()));
        assert!(result.is_ok());
        let checkpoint = result.unwrap();
        assert_eq!(checkpoint.layer_id, layer.layer_id);
        assert_eq!(checkpoint.label, Some("test".to_string()));
    }

    #[test]
    fn test_restore_checkpoint() {
        let mut manager = CheckpointManager::new(20);
        let layer = create_test_layer();

        let checkpoint = manager.create_checkpoint(&layer, None).unwrap();
        let result = manager.restore_checkpoint(&checkpoint.checkpoint_id);
        assert!(result.is_ok());
        assert!(result.unwrap().success);
    }

    #[test]
    fn test_restore_nonexistent_checkpoint() {
        let manager = CheckpointManager::new(20);
        let result = manager.restore_checkpoint("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_checkpoints() {
        let mut manager = CheckpointManager::new(20);
        let layer = create_test_layer();

        manager
            .create_checkpoint(&layer, Some("cp1".to_string()))
            .unwrap();
        manager
            .create_checkpoint(&layer, Some("cp2".to_string()))
            .unwrap();

        let list = manager.list_checkpoints(&layer.layer_id);
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_delete_checkpoint() {
        let mut manager = CheckpointManager::new(20);
        let layer = create_test_layer();

        let checkpoint = manager.create_checkpoint(&layer, None).unwrap();
        let result = manager.delete_checkpoint(&checkpoint.checkpoint_id);
        assert!(result.is_ok());
        assert!(result.unwrap());

        assert!(manager.get_checkpoint(&checkpoint.checkpoint_id).is_none());
    }

    #[test]
    fn test_max_checkpoints_per_layer() {
        let mut manager = CheckpointManager::new(2);
        let layer = create_test_layer();

        let cp1 = manager
            .create_checkpoint(&layer, Some("1".to_string()))
            .unwrap();
        let _cp2 = manager
            .create_checkpoint(&layer, Some("2".to_string()))
            .unwrap();
        let _cp3 = manager
            .create_checkpoint(&layer, Some("3".to_string()))
            .unwrap();

        assert!(manager.get_checkpoint(&cp1.checkpoint_id).is_none());
        assert_eq!(manager.list_checkpoints(&layer.layer_id).len(), 2);
    }

    #[test]
    fn test_get_latest_checkpoint() {
        let mut manager = CheckpointManager::new(20);
        let layer = create_test_layer();

        manager
            .create_checkpoint(&layer, Some("first".to_string()))
            .unwrap();
        let latest = manager
            .create_checkpoint(&layer, Some("latest".to_string()))
            .unwrap();

        let found = manager.get_latest_checkpoint(&layer.layer_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().checkpoint_id, latest.checkpoint_id);
    }
}
