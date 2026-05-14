#![allow(
    clippy::useless_conversion,
    clippy::redundant_closure,
    clippy::useless_format
)]
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyType};
use serde_json::Value;

use pbsm_core::modules::belief_graph::operations::BeliefGraphOperations;
use pbsm_core::modules::belief_graph::types::{
    AttributeValue, BeliefNodeType, RelationEdgeType, SourceType,
};
use pbsm_core::modules::intention_stack::manager::IntentionStackManager;
use pbsm_core::modules::metacognition::types::AnomalySeverity;
use pbsm_core::orchestrator::{PbsmConfig, PbsmOrchestrator};

#[pyclass]
struct PyToolAdapterCore;

#[pymethods]
impl PyToolAdapterCore {
    #[new]
    fn new() -> Self {
        PyToolAdapterCore
    }

    fn submit_assertions(&self, assertions_json: &str) -> PyResult<String> {
        let assertions: Vec<Value> = serde_json::from_str(assertions_json)
            .map_err(|e| PyValueError::new_err(format!("Invalid assertions JSON: {}", e)))?;

        let required_fields = [
            ("assertion_id", "assertionId"),
            ("assertion_type", "assertionType"),
            ("predicate", "predicate"),
            ("confidence", "confidence"),
        ];

        let mut assertion_ids: Vec<String> = Vec::new();

        for (i, assertion) in assertions.iter().enumerate() {
            let obj = assertion.as_object().ok_or_else(|| {
                PyValueError::new_err(format!("Assertion at index {} is not an object", i))
            })?;

            for (snake, camel) in &required_fields {
                if !obj.contains_key(*snake) && !obj.contains_key(*camel) {
                    return Err(PyValueError::new_err(format!(
                        "Assertion at index {} missing required field: {} (or {})",
                        i, snake, camel
                    )));
                }
            }

            let has_subject = obj.contains_key("subject_type")
                || obj.contains_key("subjectType")
                || obj.contains_key("subject");
            if !has_subject {
                return Err(PyValueError::new_err(format!(
                    "Assertion at index {} missing required field: subject_type (or subject)",
                    i
                )));
            }

            let id = obj
                .get("assertion_id")
                .or_else(|| obj.get("assertionId"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    PyValueError::new_err(format!(
                        "Assertion at index {} has non-string assertion_id",
                        i
                    ))
                })?;

            assertion_ids.push(id.to_string());
        }

        let result = serde_json::json!({
            "status": "accepted",
            "count": assertion_ids.len(),
            "assertion_ids": assertion_ids,
        });

        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize result: {}", e)))
    }

    fn verify_prediction(&self, prediction_id: &str, observations_json: &str) -> PyResult<String> {
        let _observations: Value = serde_json::from_str(observations_json)
            .map_err(|e| PyValueError::new_err(format!("Invalid observations JSON: {}", e)))?;

        let result = serde_json::json!({
            "status": "verified",
            "prediction_id": prediction_id,
            "confidence": 0.5,
        });

        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize result: {}", e)))
    }

    fn query_beliefs(&self, query_spec_json: &str) -> PyResult<String> {
        let _query_spec: Value = serde_json::from_str(query_spec_json)
            .map_err(|e| PyValueError::new_err(format!("Invalid query spec JSON: {}", e)))?;

        let result = serde_json::json!({
            "status": "ok",
            "results": [],
            "total_count": 0,
        });

        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize result: {}", e)))
    }
}

#[pyclass]
#[derive(Clone)]
struct PyStructuredAssertion {
    #[pyo3(get, set)]
    assertion_id: String,
    #[pyo3(get, set)]
    assertion_type: String,
    #[pyo3(get, set)]
    subject_type: String,
    #[pyo3(get, set)]
    subject_id: String,
    #[pyo3(get, set)]
    predicate: String,
    #[pyo3(get, set)]
    value: String,
    #[pyo3(get, set)]
    value_type: String,
    #[pyo3(get, set)]
    confidence: f64,
    #[pyo3(get, set)]
    confidence_method: String,
    #[pyo3(get, set)]
    tool_id: String,
    #[pyo3(get, set)]
    tool_name: String,
    #[pyo3(get, set)]
    invocation_id: String,
    #[pyo3(get, set)]
    data_location_format: String,
    #[pyo3(get, set)]
    data_path: String,
}

#[pymethods]
impl PyStructuredAssertion {
    #[new]
    #[pyo3(signature = (
        assertion_id="".to_string(),
        assertion_type="".to_string(),
        subject_type="".to_string(),
        subject_id="".to_string(),
        predicate="".to_string(),
        value="".to_string(),
        value_type="".to_string(),
        confidence=0.0,
        confidence_method="".to_string(),
        tool_id="".to_string(),
        tool_name="".to_string(),
        invocation_id="".to_string(),
        data_location_format="".to_string(),
        data_path="".to_string(),
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        assertion_id: String,
        assertion_type: String,
        subject_type: String,
        subject_id: String,
        predicate: String,
        value: String,
        value_type: String,
        confidence: f64,
        confidence_method: String,
        tool_id: String,
        tool_name: String,
        invocation_id: String,
        data_location_format: String,
        data_path: String,
    ) -> Self {
        PyStructuredAssertion {
            assertion_id,
            assertion_type,
            subject_type,
            subject_id,
            predicate,
            value,
            value_type,
            confidence,
            confidence_method,
            tool_id,
            tool_name,
            invocation_id,
            data_location_format,
            data_path,
        }
    }

    #[classmethod]
    fn from_dict(_cls: &Bound<'_, PyType>, dict: &Bound<'_, PyDict>) -> PyResult<Self> {
        let get_string = |key: &str| -> PyResult<String> {
            match dict.get_item(key)? {
                Some(val) => val.extract::<String>(),
                None => Err(PyValueError::new_err(format!("Missing key: {}", key))),
            }
        };

        let get_f64 = |key: &str| -> PyResult<f64> {
            match dict.get_item(key)? {
                Some(val) => val.extract::<f64>(),
                None => Err(PyValueError::new_err(format!("Missing key: {}", key))),
            }
        };

        Ok(PyStructuredAssertion {
            assertion_id: get_string("assertion_id")?,
            assertion_type: get_string("assertion_type")?,
            subject_type: get_string("subject_type")?,
            subject_id: get_string("subject_id")?,
            predicate: get_string("predicate")?,
            value: get_string("value")?,
            value_type: get_string("value_type")?,
            confidence: get_f64("confidence")?,
            confidence_method: get_string("confidence_method")?,
            tool_id: get_string("tool_id")?,
            tool_name: get_string("tool_name")?,
            invocation_id: get_string("invocation_id")?,
            data_location_format: get_string("data_location_format")?,
            data_path: get_string("data_path")?,
        })
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<PyObject> {
        let dict = PyDict::new_bound(py);
        dict.set_item("assertion_id", &self.assertion_id)?;
        dict.set_item("assertion_type", &self.assertion_type)?;
        dict.set_item("subject_type", &self.subject_type)?;
        dict.set_item("subject_id", &self.subject_id)?;
        dict.set_item("predicate", &self.predicate)?;
        dict.set_item("value", &self.value)?;
        dict.set_item("value_type", &self.value_type)?;
        dict.set_item("confidence", self.confidence)?;
        dict.set_item("confidence_method", &self.confidence_method)?;
        dict.set_item("tool_id", &self.tool_id)?;
        dict.set_item("tool_name", &self.tool_name)?;
        dict.set_item("invocation_id", &self.invocation_id)?;
        dict.set_item("data_location_format", &self.data_location_format)?;
        dict.set_item("data_path", &self.data_path)?;
        Ok(dict.into_any().unbind())
    }
}

#[pyclass]
struct PyPbsmConfig {
    inner: PbsmConfig,
}

#[pymethods]
impl PyPbsmConfig {
    #[new]
    #[pyo3(signature = (config_json=None))]
    fn new(config_json: Option<&str>) -> PyResult<Self> {
        let config = if let Some(json) = config_json {
            let path = std::path::Path::new(json);
            if path.exists() {
                if path
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
                {
                    return Err(PyValueError::new_err("Path traversal (..) is not allowed"));
                }
                if json.ends_with(".toml") {
                    PbsmConfig::load_from_toml(path)
                        .map_err(|e| PyValueError::new_err(format!("{}", e)))?
                } else {
                    PbsmConfig::load_from_json(path)
                        .map_err(|e| PyValueError::new_err(format!("{}", e)))?
                }
            } else {
                PbsmConfig::from_json_str(json)
                    .map_err(|e| PyValueError::new_err(format!("{}", e)))?
            }
        } else {
            PbsmConfig::default()
        };
        Ok(PyPbsmConfig { inner: config })
    }

    fn validate(&self) -> PyResult<()> {
        self.inner
            .validate()
            .map_err(|e| PyValueError::new_err(format!("{}", e)))
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn save(&self, path: &str) -> PyResult<()> {
        let p = std::path::Path::new(path);
        if p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(PyValueError::new_err("Path traversal (..) is not allowed"));
        }
        if path.ends_with(".toml") {
            self.inner
                .save_to_toml(p)
                .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        } else {
            self.inner
                .save_to_json(p)
                .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))
        }
    }

    #[getter]
    fn graph_max_nodes(&self) -> usize {
        self.inner.graph.max_nodes
    }

    #[setter]
    fn set_graph_max_nodes(&mut self, value: usize) {
        self.inner.graph.max_nodes = value;
    }

    #[getter]
    fn graph_max_edges(&self) -> usize {
        self.inner.graph.max_edges
    }

    #[setter]
    fn set_graph_max_edges(&mut self, value: usize) {
        self.inner.graph.max_edges = value;
    }

    #[getter]
    fn intention_stack_max_depth(&self) -> usize {
        self.inner.intention_stack.max_stack_depth
    }

    #[setter]
    fn set_intention_stack_max_depth(&mut self, value: usize) {
        self.inner.intention_stack.max_stack_depth = value;
    }
}

#[pyclass]
struct PyPbsmOrchestrator {
    inner: PbsmOrchestrator,
}

#[pymethods]
impl PyPbsmOrchestrator {
    #[new]
    #[pyo3(signature = (config=None))]
    fn new(config: Option<&PyPbsmConfig>) -> PyResult<Self> {
        let cfg = config.map(|c| c.inner.clone()).unwrap_or_default();
        cfg.validate()
            .map_err(|e| PyValueError::new_err(format!("{}", e)))?;
        Ok(PyPbsmOrchestrator {
            inner: PbsmOrchestrator::new(cfg),
        })
    }

    fn start_task(&self, description: String) -> PyResult<String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;
        let result = rt
            .block_on(self.inner.start_task(description, None))
            .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;
        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn execute_cycle(&self) -> PyResult<String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;
        let result = rt
            .block_on(self.inner.execute_cycle())
            .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;
        let json_result = serde_json::json!({
            "attention_mode": result.attention_mode,
            "active_predictions": result.active_predictions,
            "pending_forget_count": result.pending_forget_count,
        });
        serde_json::to_string(&json_result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn handle_error(&self, error_description: String, severity: &str) -> PyResult<String> {
        let sev = match severity.to_lowercase().as_str() {
            "none" => AnomalySeverity::None,
            "low" => AnomalySeverity::Low,
            "medium" => AnomalySeverity::Medium,
            "high" => AnomalySeverity::High,
            _ => {
                return Err(PyValueError::new_err(format!(
                    "Unknown severity: {}",
                    severity
                )))
            }
        };
        let result = self
            .inner
            .handle_error(error_description, sev)
            .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;
        let json_result = serde_json::json!({
            "error_description": result.error_description,
            "anomaly_count": result.anomaly_count,
            "intervention_applied": result.intervention_applied,
        });
        serde_json::to_string(&json_result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn belief_graph_node_count(&self) -> usize {
        self.inner.belief_graph().node_count()
    }

    fn belief_graph_edge_count(&self) -> usize {
        self.inner.belief_graph().edge_count()
    }

    fn event_bus_receiver_count(&self) -> usize {
        self.inner.event_bus().receiver_count()
    }

    fn has_memory_store(&self) -> bool {
        self.inner.memory_store().is_some()
    }

    fn get_config_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self.inner.config())
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn memory_footprint(&self) -> PyResult<String> {
        let fp = self.inner.memory_footprint();
        let result = serde_json::json!({
            "belief_graph_nodes": fp.belief_graph_nodes,
            "belief_graph_edges": fp.belief_graph_edges,
            "event_bus_history": fp.event_bus_history,
            "event_bus_receivers": fp.event_bus_receivers,
            "has_memory_store": fp.has_memory_store,
        });
        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn consistency_check(&self) -> PyResult<String> {
        let report = self.inner.consistency_check();
        let issues: Vec<serde_json::Value> = report
            .issues
            .iter()
            .map(|i| {
                serde_json::json!({
                    "severity": match i.severity {
                        pbsm_core::orchestrator::IssueSeverity::Warning => "warning",
                        pbsm_core::orchestrator::IssueSeverity::Error => "error",
                    },
                    "component": i.component,
                    "description": i.description,
                })
            })
            .collect();
        let result = serde_json::json!({
            "is_consistent": report.is_consistent,
            "error_count": report.error_count,
            "warning_count": report.warning_count,
            "issues": issues,
        });
        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    #[pyo3(signature = (node_type, name, attributes_json=None, source="tool_adapter", source_type="ToolReturn", tags_json=None, initial_confidence=None))]
    fn create_belief(
        &self,
        node_type: &str,
        name: &str,
        attributes_json: Option<&str>,
        source: &str,
        source_type: &str,
        tags_json: Option<&str>,
        initial_confidence: Option<f64>,
    ) -> PyResult<String> {
        let nt = match node_type {
            "User" => BeliefNodeType::User,
            "File" => BeliefNodeType::File,
            "Tool" => BeliefNodeType::Tool,
            "Variable" => BeliefNodeType::Variable,
            "Concept" => BeliefNodeType::Concept,
            "Event" => BeliefNodeType::Event,
            "Agent" => BeliefNodeType::Agent,
            "Resource" => BeliefNodeType::Resource,
            "Process" => BeliefNodeType::Process,
            _ => return Err(PyValueError::new_err(format!("Unknown node_type: {}", node_type))),
        };

        let st = match source_type {
            "DirectObservation" => SourceType::DirectObservation,
            "ToolReturn" => SourceType::ToolReturn,
            "UserInput" => SourceType::UserInput,
            "Derived" => SourceType::Derived,
            "MemoryRestore" => SourceType::MemoryRestore,
            "AgentSync" => SourceType::AgentSync,
            _ => return Err(PyValueError::new_err(format!("Unknown source_type: {}", source_type))),
        };

        let mut attrs = std::collections::HashMap::new();
        if let Some(json) = attributes_json {
            let parsed: std::collections::HashMap<String, Value> = serde_json::from_str(json)
                .map_err(|e| PyValueError::new_err(format!("Invalid attributes JSON: {}", e)))?;
            for (k, v) in parsed {
                let attr_val = AttributeValue::new(v, st.default_confidence(), source.to_string(), st);
                attrs.insert(k, attr_val);
            }
        }

        let tags = tags_json
            .map(|j| serde_json::from_str::<Vec<String>>(j))
            .transpose()
            .map_err(|e| PyValueError::new_err(format!("Invalid tags JSON: {}", e)))?;

        let belief_id = BeliefGraphOperations::create_belief(
            self.inner.belief_graph(),
            nt,
            name.to_string(),
            attrs,
            source.to_string(),
            st,
            tags,
            initial_confidence,
        )
        .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;

        let result = serde_json::json!({
            "belief_id": belief_id.to_string(),
            "node_type": node_type,
            "name": name,
        });
        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn create_edge(
        &self,
        edge_type: &str,
        source_node_id: &str,
        target_node_id: &str,
        confidence: f64,
    ) -> PyResult<String> {
        let et = match edge_type {
            "Owns" => RelationEdgeType::Owns,
            "DependsOn" => RelationEdgeType::DependsOn,
            "Authorizes" => RelationEdgeType::Authorizes,
            "Calls" => RelationEdgeType::Calls,
            "Contains" => RelationEdgeType::Contains,
            "RelatedTo" => RelationEdgeType::RelatedTo,
            "PartOf" => RelationEdgeType::PartOf,
            "LocatedIn" => RelationEdgeType::LocatedIn,
            "Enables" => RelationEdgeType::Enables,
            "Blocks" => RelationEdgeType::Blocks,
            "Modifies" => RelationEdgeType::Modifies,
            "References" => RelationEdgeType::References,
            "Causes" => RelationEdgeType::Causes,
            "Implies" => RelationEdgeType::Implies,
            "TemporalBefore" => RelationEdgeType::TemporalBefore,
            "TemporalAfter" => RelationEdgeType::TemporalAfter,
            "DelegatesTo" => RelationEdgeType::DelegatesTo,
            "SynchronizesWith" => RelationEdgeType::SynchronizesWith,
            _ => return Err(PyValueError::new_err(format!("Unknown edge_type: {}", edge_type))),
        };

        let source_id = source_node_id
            .parse()
            .map_err(|e: uuid::Error| PyValueError::new_err(format!("Invalid source_node_id: {}", e)))?;
        let target_id = target_node_id
            .parse()
            .map_err(|e: uuid::Error| PyValueError::new_err(format!("Invalid target_node_id: {}", e)))?;

        let edge_id = BeliefGraphOperations::create_edge(
            self.inner.belief_graph(),
            et,
            source_id,
            target_id,
            confidence,
        )
        .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;

        let result = serde_json::json!({
            "edge_id": edge_id.to_string(),
            "edge_type": edge_type,
            "source": source_node_id,
            "target": target_node_id,
            "confidence": confidence,
        });
        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn query_beliefs(&self, query_json: &str) -> PyResult<String> {
        let query: Value = serde_json::from_str(query_json)
            .map_err(|e| PyValueError::new_err(format!("Invalid query JSON: {}", e)))?;

        let node_type_filter = query
            .get("node_type")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "User" => Some(BeliefNodeType::User),
                "File" => Some(BeliefNodeType::File),
                "Tool" => Some(BeliefNodeType::Tool),
                "Variable" => Some(BeliefNodeType::Variable),
                "Concept" => Some(BeliefNodeType::Concept),
                "Event" => Some(BeliefNodeType::Event),
                "Agent" => Some(BeliefNodeType::Agent),
                "Resource" => Some(BeliefNodeType::Resource),
                "Process" => Some(BeliefNodeType::Process),
                _ => None,
            });

        let name_contains = query
            .get("name_contains")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let tag_filter = query
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<String>>()
            });

        let min_confidence = query
            .get("min_confidence")
            .and_then(|v| v.as_f64());

        let limit = query
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as usize;

        let nodes = self.inner.belief_graph().nodes().read();
        let mut results: Vec<serde_json::Value> = Vec::new();
        let mut count = 0;

        for (id, node) in nodes.iter() {
            if count >= limit {
                break;
            }

            if let Some(ref nt) = node_type_filter {
                if &node.node_type != nt {
                    continue;
                }
            }

            if let Some(ref name_pat) = name_contains {
                if !node.name.contains(name_pat) {
                    continue;
                }
            }

            if let Some(ref tags) = tag_filter {
                if !tags.iter().all(|t| node.metadata.tags.contains(t)) {
                    continue;
                }
            }

            if let Some(min_conf) = min_confidence {
                let node_conf = node
                    .attributes
                    .get("_initial_confidence")
                    .map(|a| a.confidence)
                    .unwrap_or(0.0);
                if node_conf < min_conf {
                    continue;
                }
            }

            results.push(serde_json::json!({
                "belief_id": id.to_string(),
                "node_type": format!("{:?}", node.node_type),
                "name": node.name,
                "tags": node.metadata.tags,
                "attribute_count": node.attributes.len(),
                "outgoing_edges": node.outgoing_edges.len(),
                "incoming_edges": node.incoming_edges.len(),
            }));
            count += 1;
        }

        let result = serde_json::json!({
            "status": "ok",
            "results": results,
            "total_count": count,
        });
        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn get_belief(&self, belief_id: &str) -> PyResult<String> {
        let id: pbsm_core::modules::belief_graph::types::BeliefId = belief_id
            .parse()
            .map_err(|e: uuid::Error| PyValueError::new_err(format!("Invalid belief_id: {}", e)))?;

        let node = self
            .inner
            .belief_graph()
            .get_node(id)
            .ok_or_else(|| PyValueError::new_err(format!("Belief not found: {}", belief_id)))?;

        let attrs: std::collections::HashMap<String, serde_json::Value> = node
            .attributes
            .iter()
            .map(|(k, v)| (k.clone(), v.value.clone()))
            .collect();

        let result = serde_json::json!({
            "belief_id": node.node_id.to_string(),
            "node_type": format!("{:?}", node.node_type),
            "name": node.name,
            "tags": node.metadata.tags,
            "attributes": attrs,
            "outgoing_edges": node.outgoing_edges.len(),
            "incoming_edges": node.incoming_edges.len(),
            "created_at": node.metadata.created_at,
        });
        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn get_belief_graph_stats(&self) -> PyResult<String> {
        let stats = self.inner.belief_graph().get_statistics();
        let result = serde_json::json!({
            "node_count": stats.total_nodes,
            "edge_count": stats.total_edges,
            "average_confidence": stats.average_confidence,
            "high_confidence_count": stats.high_confidence_count,
            "low_confidence_count": stats.low_confidence_count,
            "version": self.inner.belief_graph().version(),
        });
        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    #[pyo3(signature = (description, priority=None))]
    fn push_intention(&self, description: &str, priority: Option<&str>) -> PyResult<String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;

        let prio = match priority.unwrap_or("Medium") {
            "Critical" => pbsm_core::modules::intention_stack::state::GoalPriority::Critical,
            "High" => pbsm_core::modules::intention_stack::state::GoalPriority::High,
            "Medium" => pbsm_core::modules::intention_stack::state::GoalPriority::Medium,
            "Low" => pbsm_core::modules::intention_stack::state::GoalPriority::Low,
            _ => pbsm_core::modules::intention_stack::state::GoalPriority::Medium,
        };

        let goal = pbsm_core::modules::intention_stack::types::GoalDefinition::simple(
            description.to_string(),
            prio,
        );
        let request = pbsm_core::modules::intention_stack::types::PushIntentRequest {
            goal,
            plan: None,
            parent_level: None,
            micro_prediction: None,
            attach_to_current: false,
        };

        let result = rt
            .block_on(self.inner.intention_stack().push_intention(request))
            .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;

        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn pop_intention(&self) -> PyResult<String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;

        let stack = rt
            .block_on(self.inner.intention_stack().get_stack_state())
            .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;

        let layer_index = if stack.layers.is_empty() {
            return Err(PyRuntimeError::new_err("Intention stack is empty"));
        } else {
            stack.layers.len() - 1
        };

        let request = pbsm_core::modules::intention_stack::types::PopIntentRequest {
            layer_index,
            reason: pbsm_core::modules::intention_stack::types::PopReason::Completed,
            final_state: None,
            completion_report: None,
            cascade: false,
        };

        let result = rt
            .block_on(self.inner.intention_stack().pop_intention(request))
            .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;

        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn get_intention_stack_state(&self) -> PyResult<String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;

        let stack = rt
            .block_on(self.inner.intention_stack().get_stack_state())
            .map_err(|e| PyRuntimeError::new_err(format!("{}", e)))?;

        let layers: Vec<serde_json::Value> = stack
            .layers
            .iter()
            .map(|l| {
                serde_json::json!({
                    "layer_index": l.level,
                    "description": l.goal.description,
                    "priority": format!("{:?}", l.goal.priority),
                    "state": format!("{:?}", l.execution_state),
                })
            })
            .collect();

        let result = serde_json::json!({
            "depth": stack.layers.len(),
            "layers": layers,
        });
        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    fn get_attention_status(&self) -> PyResult<String> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to create runtime: {}", e)))?;

        let status = rt.block_on(self.inner.metacognitive_controller().get_attention_status());
        serde_json::to_string(&status)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    #[pyo3(signature = (window_size=None))]
    fn detect_anomalies(&self, window_size: Option<usize>) -> PyResult<String> {
        let report = self
            .inner
            .metacognitive_controller()
            .detect_anomalies(window_size);
        let result = serde_json::json!({
            "has_anomalies": report.has_anomalies,
            "severity": format!("{:?}", report.severity),
            "anomaly_count": report.anomalies.len(),
            "anomalies": report.anomalies.iter().map(|a| serde_json::json!({
                "anomaly_type": format!("{:?}", a.anomaly_type),
                "severity": format!("{:?}", a.severity),
                "recommendation": a.recommendation,
            })).collect::<Vec<_>>(),
        });
        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }

    #[pyo3(signature = (limit=None))]
    fn get_event_history(&self, limit: Option<usize>) -> PyResult<String> {
        let history = self.inner.event_bus().history();
        let events: Vec<serde_json::Value> = history
            .iter()
            .rev()
            .take(limit.unwrap_or(100))
            .map(|e| {
                serde_json::json!({
                    "event_type": e.event_type_name(),
                    "source_module": e.source_module(),
                })
            })
            .collect();
        let result = serde_json::json!({
            "total_events": history.len(),
            "events": events,
        });
        serde_json::to_string(&result)
            .map_err(|e| PyRuntimeError::new_err(format!("Serialization failed: {}", e)))
    }
}

#[pymodule]
fn pbsm_python(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyToolAdapterCore>()?;
    m.add_class::<PyStructuredAssertion>()?;
    m.add_class::<PyPbsmConfig>()?;
    m.add_class::<PyPbsmOrchestrator>()?;
    Ok(())
}
