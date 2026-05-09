from __future__ import annotations

import json
import os
import sys
from pathlib import Path

import pytest

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


@pytest.fixture
def adapter():
    return ToolAdapter()


@pytest.fixture
def deepseek_client():
    if not API_KEY:
        pytest.skip("No API_KEY in .env")
    return OpenAI(api_key=API_KEY, base_url=BASE_URL)


class TestPhase1Init:
    def test_adapter_native_mode(self, adapter):
        assert adapter.is_native_mode is True

    @pytest.mark.skipif(not _can_run_live(), reason=SKIP_REASON)
    def test_deepseek_client_connectivity(self, deepseek_client):
        models = deepseek_client.models.list()
        assert models is not None


class TestPhase2MultiSource:
    def test_parse_json_metrics(self, adapter):
        data = json.dumps({"server": "prod-01", "metrics": [{"name": "cpu", "value": 92.3}]})
        raw = RawOutput(content=data, content_type="application/json")
        result = adapter.parse_tool_output(raw_output=raw, tool_id="metrics_api")
        assert result.success
        assert result.format == FormatType.JSON
        assert len(result.assertions) > 0

    def test_parse_csv_logs(self, adapter):
        csv = "timestamp,service,level,message\n2026-01-01T00:00:00Z,svc,ERROR,test error"
        raw = RawOutput(content=csv, content_type="text/csv")
        result = adapter.parse_tool_output(raw_output=raw, tool_id="log_aggregator")
        assert result.success
        assert result.format == FormatType.CSV
        assert len(result.assertions) > 0

    def test_parse_error_output(self, adapter):
        error = json.dumps({"error": "ServiceUnavailable", "status_code": 503})
        raw = RawOutput(content=error, content_type="application/json", status_code=503)
        result = adapter.parse_tool_output(raw_output=raw, tool_id="health_checker")
        assert result.success

    def test_parse_text_deployment(self, adapter):
        text = "Deployment: auth-service v2.3.1\nDeployed: 2026-01-01\nStatus: ROLLED OUT"
        raw = RawOutput(content=text)
        result = adapter.parse_tool_output(raw_output=raw, tool_id="deploy_tracker")
        assert result.success
        assert result.format == FormatType.TEXT

    def test_multi_format_total_assertions(self, adapter):
        total = 0
        for content, ct in [
            (json.dumps({"server": "x", "metrics": [{"name": "cpu", "value": 90}]}), "application/json"),
            ("timestamp,service,level,message\n2026-01-01,svc,ERROR,err", "text/csv"),
            ("Deployment: svc v1\nStatus: OK", None),
        ]:
            raw = RawOutput(content=content, content_type=ct)
            r = adapter.parse_tool_output(raw_output=raw, tool_id="test")
            total += len(r.assertions)
        assert total >= 3

    def test_submit_multi_format_assertions(self, adapter):
        data = json.dumps({"server": "prod-01", "metrics": [{"name": "cpu", "value": 92}]})
        raw = RawOutput(content=data, content_type="application/json")
        r = adapter.parse_tool_output(raw_output=raw, tool_id="test")
        submit = adapter.submit_to_core(r.assertions)
        assert submit.get("status") in ("accepted", "simulated")


class TestPhase3LLMDiagnosis:
    @pytest.mark.skipif(not _can_run_live(), reason=SKIP_REASON)
    def test_llm_diagnosis_returns_structured_json(self, deepseek_client):
        response = deepseek_client.chat.completions.create(
            model=MODEL,
            messages=[
                {"role": "system", "content": "Return JSON: {\"root_cause\": string, \"confidence\": float, \"severity\": string}. ONLY JSON."},
                {"role": "user", "content": "CPU=92%, Error=12%. Diagnose."},
            ],
            temperature=0.1,
            max_tokens=256,
        )
        content = response.choices[0].message.content or ""
        start = content.find("{")
        end = content.rfind("}") + 1
        assert start >= 0
        parsed = json.loads(content[start:end])
        assert "root_cause" in parsed


class TestPhase4Remediation:
    def test_verify_prediction(self, adapter):
        result = adapter.verify_prediction("pred_test", [{"cpu": 45}])
        assert result.get("status") in ("verified", "simulated")

    def test_submit_remediation_assertions(self, adapter):
        data = json.dumps({"actions_executed": [{"action": "rollback", "result": "success"}], "services_recovered": ["auth-svc"]})
        raw = RawOutput(content=data, content_type="application/json")
        r = adapter.parse_tool_output(raw_output=raw, tool_id="remediation")
        assert r.success
        submit = adapter.submit_to_core(r.assertions)
        assert submit.get("status") in ("accepted", "simulated")


class TestPhase5SecondIncident:
    def test_second_metrics_submission(self, adapter):
        data = json.dumps({"server": "prod-payment-01", "metrics": [{"name": "latency", "value": 5200}]})
        raw = RawOutput(content=data, content_type="application/json")
        r = adapter.parse_tool_output(raw_output=raw, tool_id="metrics_api")
        submit = adapter.submit_to_core(r.assertions)
        assert submit.get("status") in ("accepted", "simulated")

    @pytest.mark.skipif(not _can_run_live(), reason=SKIP_REASON)
    def test_llm_identifies_related_incident(self, deepseek_client):
        response = deepseek_client.chat.completions.create(
            model=MODEL,
            messages=[
                {"role": "system", "content": "Return JSON: {\"is_related_to_first\": boolean, \"pattern_match\": string}. ONLY JSON."},
                {"role": "user", "content": "First: auth-service pool exhausted. Second: payment-db connection storm after auth restart. Related?"},
            ],
            temperature=0.1,
            max_tokens=256,
        )
        content = response.choices[0].message.content or ""
        start = content.find("{")
        end = content.rfind("}") + 1
        parsed = json.loads(content[start:end])
        assert parsed.get("is_related_to_first") is True


class TestPhase6ErrorCascade:
    def test_handle_high_severity(self, adapter):
        result = adapter.handle_pbsm_error("DB timeout", "high")
        assert "intervention_applied" in result

    def test_handle_medium_severity(self, adapter):
        result = adapter.handle_pbsm_error("Slow response", "medium")
        assert "anomaly_count" in result

    def test_handle_low_severity(self, adapter):
        result = adapter.handle_pbsm_error("Minor delay", "low")
        assert "intervention_applied" in result

    def test_multiple_errors_intervention(self, adapter):
        interventions = 0
        for desc, sev in [("err1", "high"), ("err2", "high"), ("err3", "medium")]:
            r = adapter.handle_pbsm_error(desc, sev)
            interventions += int(r.get("intervention_applied", False))
        assert interventions >= 1


class TestPhase7Diagnostics:
    def test_belief_graph_stats(self, adapter):
        stats = adapter.get_belief_graph_stats()
        assert "node_count" in stats
        assert "edge_count" in stats

    def test_consistency_check(self, adapter):
        if not adapter.is_native_mode:
            pytest.skip("Native mode required")
        bridge = adapter._pbsm_bridge
        orch = bridge.orchestrator
        cc = json.loads(orch.consistency_check())
        assert "is_consistent" in cc

    def test_memory_footprint(self, adapter):
        if not adapter.is_native_mode:
            pytest.skip("Native mode required")
        bridge = adapter._pbsm_bridge
        orch = bridge.orchestrator
        mf = json.loads(orch.memory_footprint())
        assert "belief_graph_nodes" in mf
        assert "event_bus_history" in mf


class TestEndToEnd:
    @pytest.mark.skipif(not _can_run_live(), reason=SKIP_REASON)
    def test_full_incident_response_pipeline(self, deepseek_client, adapter):
        data = json.dumps({"server": "e2e-01", "metrics": [{"name": "cpu", "value": 95}]})
        raw = RawOutput(content=data, content_type="application/json")
        r = adapter.parse_tool_output(raw_output=raw, tool_id="e2e")
        assert r.success
        submit = adapter.submit_to_core(r.assertions)
        assert submit.get("status") in ("accepted", "simulated")

        task = adapter.start_task("e2e incident")
        assert task is not None

        cycle = adapter.execute_cycle()
        assert "attention_mode" in cycle

        verify = adapter.verify_prediction("pred_e2e", [{"cpu": 50}])
        assert verify.get("status") in ("verified", "simulated")

        error = adapter.handle_pbsm_error("e2e error", "high")
        assert "intervention_applied" in error

        stats = adapter.get_belief_graph_stats()
        assert "node_count" in stats
