from __future__ import annotations

import json
import os
import sys
from pathlib import Path

import pytest

import pbsm_python

PROJECT_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(PROJECT_ROOT / "adapters" / "tool_adapter"))

from dotenv import load_dotenv
from openai import OpenAI

from pbsm_tool_adapter import (
    ToolAdapter,
    RawOutput,
    AssertionType,
    FormatType,
)

load_dotenv(PROJECT_ROOT / ".env")

API_KEY = os.getenv("API_KEY", "")
BASE_URL = os.getenv("BASE_URL", "https://api.deepseek.com")
MODEL = os.getenv("MODEL", "deepseek-v4-flash")

SKIP_REASON = "Set LIVE=1 to run DeepSeek live tests"


def _can_run_live():
    return os.getenv("LIVE") == "1" and bool(API_KEY)


def _j(raw):
    if isinstance(raw, str):
        try:
            return json.loads(raw)
        except (json.JSONDecodeError, TypeError):
            return raw
    return raw


@pytest.fixture
def orch():
    return pbsm_python.PyPbsmOrchestrator()


@pytest.fixture
def adapter():
    return ToolAdapter()


@pytest.fixture
def deepseek_client():
    if not API_KEY:
        pytest.skip("No API_KEY in .env")
    return OpenAI(api_key=API_KEY, base_url=BASE_URL)


# ─── M1: BeliefGraph ─────────────────────────────────────────────────────

class TestBeliefGraphCreate:
    def test_create_belief_minimal(self, orch):
        r = _j(orch.create_belief("File", "test.py"))
        assert "belief_id" in r
        assert r["node_type"] == "File"
        assert r["name"] == "test.py"

    def test_create_belief_with_attributes(self, orch):
        r = _j(orch.create_belief(
            "Agent", "deploy-bot",
            '{"role": "deployment"}',
            "tool_adapter", "ToolReturn",
            '["ci-cd"]', 0.85,
        ))
        assert "belief_id" in r
        assert r["node_type"] == "Agent"

    def test_create_belief_all_node_types(self, orch):
        types = ["User", "File", "Tool", "Variable", "Concept", "Event", "Agent", "Resource", "Process"]
        for nt in types:
            r = _j(orch.create_belief(nt, f"test-{nt}"))
            assert r["node_type"] == nt, f"Failed for node_type={nt}"

    def test_create_belief_invalid_type(self, orch):
        with pytest.raises(Exception):
            orch.create_belief("InvalidType", "test")

    def test_create_belief_with_tags(self, orch):
        r = _j(orch.create_belief(
            "Process", "auth-service", None,
            "tool_adapter", "ToolReturn",
            '["microservice", "auth"]', 0.90,
        ))
        assert "belief_id" in r


class TestBeliefGraphEdge:
    def test_create_edge(self, orch):
        b1 = _j(orch.create_belief("User", "alice"))
        b2 = _j(orch.create_belief("File", "config.yaml"))
        r = _j(orch.create_edge("Owns", b1["belief_id"], b2["belief_id"], 0.9))
        assert "edge_id" in r
        assert r["edge_type"] == "Owns"

    def test_create_edge_all_types(self, orch):
        b1 = _j(orch.create_belief("User", "src"))
        b2 = _j(orch.create_belief("File", "tgt"))
        edge_types = [
            "Owns", "DependsOn", "Authorizes", "Calls", "Contains",
            "RelatedTo", "PartOf", "LocatedIn", "Enables", "Blocks", "Modifies", "References",
            "Causes", "Implies", "TemporalBefore", "TemporalAfter", "DelegatesTo", "SynchronizesWith",
        ]
        for et in edge_types:
            r = _j(orch.create_edge(et, b1["belief_id"], b2["belief_id"], 0.8))
            assert r["edge_type"] == et, f"Failed for edge_type={et}"

    def test_create_edge_invalid_type(self, orch):
        b1 = _j(orch.create_belief("User", "a"))
        b2 = _j(orch.create_belief("File", "b"))
        with pytest.raises(Exception):
            orch.create_edge("InvalidEdge", b1["belief_id"], b2["belief_id"], 0.5)

    def test_create_edge_invalid_uuid(self, orch):
        with pytest.raises(Exception):
            orch.create_edge("Owns", "not-a-uuid", "also-not-uuid", 0.5)


class TestBeliefGraphQuery:
    def test_query_by_node_type(self, orch):
        orch.create_belief("Process", "svc-1")
        orch.create_belief("Process", "svc-2")
        orch.create_belief("File", "config.yaml")
        r = _j(orch.query_beliefs('{"node_type": "Process"}'))
        assert r["total_count"] >= 2

    def test_query_by_name_contains(self, orch):
        orch.create_belief("Process", "payment-service-v2")
        r = _j(orch.query_beliefs('{"name_contains": "payment"}'))
        assert r["total_count"] >= 1

    def test_query_by_tags(self, orch):
        orch.create_belief("Resource", "prod-db", None, "direct_observation", "DirectObservation", '["production"]', 0.95)
        r = _j(orch.query_beliefs('{"tags": ["production"]}'))
        assert r["total_count"] >= 1

    def test_query_by_min_confidence(self, orch):
        orch.create_belief("Concept", "high-conf", None, "tool_adapter", "ToolReturn", None, 0.99)
        orch.create_belief("Concept", "low-conf", None, "tool_adapter", "ToolReturn", None, 0.3)
        r = _j(orch.query_beliefs('{"min_confidence": 0.9}'))
        assert r["total_count"] >= 1

    def test_query_no_filter(self, orch):
        r = _j(orch.query_beliefs('{}'))
        assert r["total_count"] >= 0
        assert r["status"] == "ok"


class TestBeliefGraphGet:
    def test_get_belief(self, orch):
        created = _j(orch.create_belief("File", "test.py"))
        bid = created["belief_id"]
        r = _j(orch.get_belief(bid))
        assert r["belief_id"] == bid
        assert r["name"] == "test.py"
        assert r["node_type"] == "File"

    def test_get_belief_not_found(self, orch):
        with pytest.raises(Exception):
            orch.get_belief("00000000-0000-0000-0000-000000000000")


class TestBeliefGraphStats:
    def test_stats_structure(self, orch):
        orch.create_belief("File", "stats-test.py")
        r = _j(orch.get_belief_graph_stats())
        assert "node_count" in r
        assert "edge_count" in r
        assert "average_confidence" in r
        assert r["node_count"] >= 1

    def test_stats_after_operations(self, orch):
        b1 = _j(orch.create_belief("User", "stats-user"))
        b2 = _j(orch.create_belief("File", "stats-file"))
        orch.create_edge("Owns", b1["belief_id"], b2["belief_id"], 0.9)
        r = _j(orch.get_belief_graph_stats())
        assert r["node_count"] >= 2
        assert r["edge_count"] >= 1


# ─── M2: PredictionEngine ────────────────────────────────────────────────

class TestPrediction:
    def test_verify_prediction(self, adapter):
        r = adapter.verify_prediction("pred_test", [{"cpu": 45}])
        assert r.get("status") in ("verified", "simulated")

    def test_execute_cycle(self, orch):
        r = _j(orch.execute_cycle())
        assert "attention_mode" in r
        assert "active_predictions" in r
        assert "pending_forget_count" in r


# ─── M3: ToolAdapter ─────────────────────────────────────────────────────

class TestToolAdapter:
    def test_native_mode(self, adapter):
        assert adapter.is_native_mode is True

    def test_parse_json(self, adapter):
        data = json.dumps({"server": "x", "metrics": [{"name": "cpu", "value": 90}]})
        raw = RawOutput(content=data, content_type="application/json")
        r = adapter.parse_tool_output(raw_output=raw, tool_id="test")
        assert r.success
        assert r.format == FormatType.JSON

    def test_parse_csv(self, adapter):
        csv = "timestamp,service,level,message\n2026-01-01,svc,ERROR,err"
        raw = RawOutput(content=csv, content_type="text/csv")
        r = adapter.parse_tool_output(raw_output=raw, tool_id="test")
        assert r.success
        assert r.format == FormatType.CSV

    def test_parse_error(self, adapter):
        error = json.dumps({"error": "ServiceUnavailable", "status_code": 503})
        raw = RawOutput(content=error, content_type="application/json", status_code=503)
        r = adapter.parse_tool_output(raw_output=raw, tool_id="test")
        assert r.success

    def test_parse_text(self, adapter):
        text = "Deployment: svc v1\nStatus: OK"
        raw = RawOutput(content=text)
        r = adapter.parse_tool_output(raw_output=raw, tool_id="test")
        assert r.success
        assert r.format == FormatType.TEXT

    def test_submit_assertions(self, adapter):
        data = json.dumps({"server": "x", "metrics": [{"name": "cpu", "value": 90}]})
        raw = RawOutput(content=data, content_type="application/json")
        r = adapter.parse_tool_output(raw_output=raw, tool_id="test")
        submit = adapter.submit_to_core(r.assertions)
        assert submit.get("status") in ("accepted", "simulated")

    def test_structured_assertion(self):
        sa = pbsm_python.PyStructuredAssertion(
            assertion_id="test-001",
            assertion_type="metric",
            subject_type="service",
            subject_id="test-svc",
            predicate="has_cpu",
            value="90",
            value_type="percent",
            confidence=0.95,
            confidence_method="direct",
            tool_id="test",
            tool_name="TestTool",
            invocation_id="inv-001",
            data_location_format="inline",
            data_path="",
        )
        assert sa.assertion_id == "test-001"
        assert sa.confidence == 0.95


# ─── M4: MetacognitiveController ─────────────────────────────────────────

class TestMetacognition:
    def test_get_attention_status(self, orch):
        r = _j(orch.get_attention_status())
        assert "current_mode" in r
        assert "attention_parameter" in r

    def test_detect_anomalies(self, orch):
        r = _j(orch.detect_anomalies())
        assert "has_anomalies" in r
        assert "anomaly_count" in r
        assert "severity" in r

    def test_handle_error_high(self, orch):
        r = _j(orch.handle_error("DB timeout", "high"))
        assert "anomaly_count" in r
        assert "intervention_applied" in r

    def test_handle_error_medium(self, orch):
        r = _j(orch.handle_error("Slow response", "medium"))
        assert "anomaly_count" in r

    def test_handle_error_low(self, orch):
        r = _j(orch.handle_error("Minor delay", "low"))
        assert "intervention_applied" in r

    def test_handle_error_invalid_severity(self, orch):
        with pytest.raises(Exception):
            orch.handle_error("test", "critical")

    def test_multiple_errors_trigger_intervention(self, orch):
        interventions = 0
        for desc, sev in [("err1", "high"), ("err2", "high"), ("err3", "medium")]:
            r = _j(orch.handle_error(desc, sev))
            interventions += int(r.get("intervention_applied", False))
        assert interventions >= 1


# ─── M5: IntentionStack ──────────────────────────────────────────────────

class TestIntentionStack:
    def test_start_task(self, orch):
        r = _j(orch.start_task("test task"))
        assert r.get("success") is True

    def test_push_intention(self, orch):
        orch.start_task("test")
        r = _j(orch.push_intention("sub-task", "High"))
        assert r.get("success") is True

    def test_push_intention_with_priority(self, orch):
        orch.start_task("test")
        for prio in ["Critical", "High", "Medium", "Low"]:
            r = _j(orch.push_intention(f"task-{prio}", prio))
            assert r.get("success") is True

    def test_pop_intention(self, orch):
        orch.start_task("test")
        orch.push_intention("sub-task", "Medium")
        r = _j(orch.pop_intention())
        assert r.get("success") is True

    def test_get_intention_stack_state(self, orch):
        orch.start_task("test")
        orch.push_intention("sub-task", "High")
        r = _j(orch.get_intention_stack_state())
        assert "depth" in r
        assert r["depth"] >= 1
        assert len(r["layers"]) >= 1

    def test_stack_depth_increases_with_push(self, orch):
        orch.start_task("test")
        s1 = _j(orch.get_intention_stack_state())
        orch.push_intention("task-1", "Medium")
        s2 = _j(orch.get_intention_stack_state())
        assert s2["depth"] == s1["depth"] + 1

    def test_stack_depth_decreases_with_pop(self, orch):
        orch.start_task("test")
        orch.push_intention("task-1", "Medium")
        s1 = _j(orch.get_intention_stack_state())
        orch.pop_intention()
        s2 = _j(orch.get_intention_stack_state())
        assert s2["depth"] == s1["depth"] - 1


# ─── M6: ExternalMemory ──────────────────────────────────────────────────

class TestExternalMemory:
    def test_has_memory_store(self, orch):
        r = _j(orch.memory_footprint())
        assert "has_memory_store" in r
        assert isinstance(r["has_memory_store"], bool)


# ─── M7: EventBus ────────────────────────────────────────────────────────

class TestEventBus:
    def test_get_event_history(self, orch):
        orch.start_task("test")
        r = _j(orch.get_event_history())
        assert "total_events" in r
        assert "events" in r
        assert r["total_events"] >= 1

    def test_get_event_history_with_limit(self, orch):
        orch.start_task("test")
        r = _j(orch.get_event_history(5))
        assert len(r.get("events", [])) <= 5

    def test_events_have_type_and_source(self, orch):
        orch.start_task("test")
        r = _j(orch.get_event_history(10))
        for event in r.get("events", []):
            assert "event_type" in event
            assert "source_module" in event


# ─── Orchestrator Core ───────────────────────────────────────────────────

class TestOrchestratorCore:
    def test_consistency_check(self, orch):
        r = _j(orch.consistency_check())
        assert "is_consistent" in r
        assert "error_count" in r
        assert "warning_count" in r

    def test_memory_footprint(self, orch):
        r = _j(orch.memory_footprint())
        assert "belief_graph_nodes" in r
        assert "belief_graph_edges" in r
        assert "event_bus_history" in r

    def test_config_json(self, orch):
        r = _j(orch.get_config_json())
        assert isinstance(r, dict)

    def test_belief_graph_node_count(self, orch):
        assert orch.belief_graph_node_count() >= 0

    def test_belief_graph_edge_count(self, orch):
        assert orch.belief_graph_edge_count() >= 0

    def test_event_bus_receiver_count(self, orch):
        assert orch.event_bus_receiver_count() >= 0


# ─── Config ──────────────────────────────────────────────────────────────

class TestConfig:
    def test_default_config(self):
        cfg = pbsm_python.PyPbsmConfig()
        cfg.validate()
        assert cfg.graph_max_nodes > 0
        assert cfg.graph_max_edges > 0
        assert cfg.intention_stack_max_depth > 0

    def test_config_setters(self):
        cfg = pbsm_python.PyPbsmConfig()
        cfg.graph_max_nodes = 1000
        assert cfg.graph_max_nodes == 1000
        cfg.graph_max_edges = 5000
        assert cfg.graph_max_edges == 5000

    def test_config_to_json(self):
        cfg = pbsm_python.PyPbsmConfig()
        j = cfg.to_json()
        assert isinstance(j, str)
        parsed = json.loads(j)
        assert "graph" in parsed


# ─── LLM Integration ────────────────────────────────────────────────────

class TestLLMIntegration:
    @pytest.mark.skipif(not _can_run_live(), reason=SKIP_REASON)
    def test_deepseek_connectivity(self, deepseek_client):
        models = deepseek_client.models.list()
        assert models is not None

    @pytest.mark.skipif(not _can_run_live(), reason=SKIP_REASON)
    def test_llm_structured_output(self, deepseek_client):
        response = deepseek_client.chat.completions.create(
            model=MODEL,
            messages=[
                {"role": "system", "content": "Return JSON: {\"root_cause\": string, \"confidence\": float}. ONLY JSON."},
                {"role": "user", "content": "CPU=92%, Error=12%. Diagnose."},
            ],
            temperature=0.1,
            max_tokens=256,
        )
        content = response.choices[0].message.content or ""
        start = content.find("{")
        end = content.rfind("}") + 1
        if start < 0 or end <= start:
            pytest.skip("LLM did not return parseable JSON")
        parsed = json.loads(content[start:end])
        assert "root_cause" in parsed

    @pytest.mark.skipif(not _can_run_live(), reason=SKIP_REASON)
    def test_llm_analysis_creates_belief(self, deepseek_client, orch):
        response = deepseek_client.chat.completions.create(
            model=MODEL,
            messages=[
                {"role": "system", "content": "Return a JSON object with keys: analysis (string), confidence (float between 0 and 1). Return ONLY the JSON, no other text."},
                {"role": "user", "content": "Analyze: high CPU usage on production server."},
            ],
            temperature=0.1,
            max_tokens=256,
        )
        content = response.choices[0].message.content or ""
        start = content.find("{")
        end = content.rfind("}") + 1
        if start < 0 or end <= start:
            pytest.skip("LLM did not return parseable JSON")
        analysis = json.loads(content[start:end])

        r = _j(orch.create_belief(
            "Concept", "llm-analysis",
            json.dumps(analysis),
            "tool_adapter", "ToolReturn",
            '["llm-output"]', analysis.get("confidence", 0.8),
        ))
        assert "belief_id" in r


# ─── End-to-End ──────────────────────────────────────────────────────────

class TestEndToEnd:
    def test_full_pipeline(self, orch, adapter):
        orch.start_task("e2e test")

        b1 = _j(orch.create_belief("Process", "auth-service", '{"status": "degraded"}', "tool_adapter", "ToolReturn", '["microservice"]', 0.82))
        b2 = _j(orch.create_belief("Resource", "prod-db", '{"engine": "postgresql"}', "direct_observation", "DirectObservation", '["database"]', 0.95))
        orch.create_edge("DependsOn", b1["belief_id"], b2["belief_id"], 0.95)

        orch.push_intention("Investigate auth-service", "High")

        data = json.dumps({"server": "e2e-01", "metrics": [{"name": "cpu", "value": 95}]})
        raw = RawOutput(content=data, content_type="application/json")
        r = adapter.parse_tool_output(raw_output=raw, tool_id="e2e")
        assert r.success
        submit = adapter.submit_to_core(r.assertions)
        assert submit.get("status") in ("accepted", "simulated")

        cycle = _j(orch.execute_cycle())
        assert "attention_mode" in cycle

        verify = adapter.verify_prediction("pred_e2e", [{"cpu": 50}])
        assert verify.get("status") in ("verified", "simulated")

        error = _j(orch.handle_error("e2e error", "high"))
        assert "intervention_applied" in error

        stats = _j(orch.get_belief_graph_stats())
        assert stats["node_count"] >= 2
        assert stats["edge_count"] >= 1

        cc = _j(orch.consistency_check())
        assert "is_consistent" in cc

        orch.pop_intention()

        events = _j(orch.get_event_history(10))
        assert events["total_events"] >= 1

    @pytest.mark.skipif(not _can_run_live(), reason=SKIP_REASON)
    def test_llm_pbsm_loop(self, deepseek_client, orch):
        b1 = _j(orch.create_belief("Process", "api-gateway", '{"status": "degraded"}', "tool_adapter", "ToolReturn", '["microservice"]', 0.80))
        b2 = _j(orch.create_belief("Concept", "cascade-risk", '{"severity": "high"}', "derived", "Derived", '["risk"]', 0.65))
        orch.create_edge("References", b2["belief_id"], b1["belief_id"], 0.75)

        stats = _j(orch.get_belief_graph_stats())
        attn = _j(orch.get_attention_status())

        response = deepseek_client.chat.completions.create(
            model=MODEL,
            messages=[
                {"role": "system", "content": "Return a JSON object with keys: health (string), risk (string), action (string). Return ONLY the JSON, no other text."},
                {"role": "user", "content": f"System: nodes={stats['node_count']}, attention={attn['current_mode']}, api-gateway degraded. Analyze."},
            ],
            temperature=0.1,
            max_tokens=256,
        )
        content = response.choices[0].message.content or ""
        start = content.find("{")
        end = content.rfind("}") + 1
        if start < 0 or end <= start:
            pytest.skip("LLM did not return parseable JSON")
        analysis = json.loads(content[start:end])

        r = _j(orch.create_belief(
            "Concept", "llm-result",
            json.dumps(analysis),
            "tool_adapter", "ToolReturn",
            '["llm-output"]', 0.85,
        ))
        assert "belief_id" in r

        final_stats = _j(orch.get_belief_graph_stats())
        assert final_stats["node_count"] > stats["node_count"]
