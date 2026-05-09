from __future__ import annotations

import json
from typing import Any, Optional

from .types import StructuredAssertion, ToolAdapterError


class PyO3Bridge:
    def __init__(self, config_json: Optional[str] = None):
        self._native_core = None
        self._orchestrator = None
        self._config = None
        self._fallback_mode = True
        try:
            from pbsm_python import PyToolAdapterCore, PyPbsmConfig, PyPbsmOrchestrator

            if config_json:
                self._config = PyPbsmConfig(config_json)
            else:
                self._config = PyPbsmConfig()
            self._native_core = PyToolAdapterCore()
            self._orchestrator = PyPbsmOrchestrator(self._config)
            self._fallback_mode = False
        except ImportError:
            pass

    @property
    def is_native(self) -> bool:
        return not self._fallback_mode

    @property
    def orchestrator(self):
        return self._orchestrator

    @property
    def config(self):
        return self._config

    def submit_assertions(self, assertions: list[StructuredAssertion]) -> dict[str, Any]:
        if not self._fallback_mode:
            json_str = self._assertions_to_json(assertions)
            result = self._native_core.submit_assertions(json_str)
            return json.loads(result)
        return {
            "status": "simulated",
            "accepted": len(assertions),
            "assertion_ids": [a.assertion_id for a in assertions],
            "message": "Fallback mode: assertions not submitted to core",
        }

    def verify_prediction(
        self, prediction_id: str, observations: list[dict[str, Any]]
    ) -> dict[str, Any]:
        if not self._fallback_mode:
            result = self._native_core.verify_prediction(
                prediction_id, json.dumps(observations)
            )
            return json.loads(result)
        return {
            "status": "simulated",
            "prediction_id": prediction_id,
            "verified": True,
            "confidence": 0.5,
            "message": "Fallback mode: prediction verification simulated",
        }

    def query_beliefs(self, query_spec: dict[str, Any]) -> dict[str, Any]:
        if not self._fallback_mode:
            result = self._native_core.query_beliefs(json.dumps(query_spec))
            return json.loads(result)
        return {
            "status": "simulated",
            "results": [],
            "total_count": 0,
            "message": "Fallback mode: belief query simulated",
        }

    def start_task(self, description: str) -> dict[str, Any]:
        if not self._fallback_mode:
            result = self._orchestrator.start_task(description)
            return json.loads(result)
        return {
            "status": "simulated",
            "description": description,
            "message": "Fallback mode: task start simulated",
        }

    def execute_cycle(self) -> dict[str, Any]:
        if not self._fallback_mode:
            result = self._orchestrator.execute_cycle()
            return json.loads(result)
        return {
            "status": "simulated",
            "attention_mode": "UNKNOWN",
            "active_predictions": 0,
            "pending_forget_count": 0,
            "message": "Fallback mode: cycle execution simulated",
        }

    def handle_error(
        self, error_description: str, severity: str = "medium"
    ) -> dict[str, Any]:
        if not self._fallback_mode:
            result = self._orchestrator.handle_error(error_description, severity)
            return json.loads(result)
        return {
            "status": "simulated",
            "error_description": error_description,
            "anomaly_count": 0,
            "intervention_applied": False,
            "message": "Fallback mode: error handling simulated",
        }

    def get_belief_graph_stats(self) -> dict[str, Any]:
        if not self._fallback_mode:
            return {
                "node_count": self._orchestrator.belief_graph_node_count(),
                "edge_count": self._orchestrator.belief_graph_edge_count(),
                "has_memory_store": self._orchestrator.has_memory_store(),
            }
        return {
            "node_count": 0,
            "edge_count": 0,
            "has_memory_store": False,
            "message": "Fallback mode: graph stats simulated",
        }

    def get_config_json(self) -> str:
        if not self._fallback_mode:
            return self._orchestrator.get_config_json()
        return json.dumps({"status": "simulated"})

    def _assertions_to_json(self, assertions: list[StructuredAssertion]) -> str:
        return json.dumps([a.to_dict() for a in assertions])
