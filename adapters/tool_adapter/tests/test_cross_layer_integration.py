from __future__ import annotations

from pbsm_tool_adapter import (
    PyO3Bridge,
    ToolAdapter,
    FormatType,
    FormatIdentification,
    RawOutput,
    StructuredAssertion,
    AssertionType,
    AssertionSubject,
    AssertionObject,
    AssertionMetadata,
    ConfidenceInfo,
    ConfidenceMethod,
    ConfidenceFactor,
    SourceInfo,
    DataLocation,
    DataLocationFormat,
    ValueType,
)


def _make_test_assertion() -> StructuredAssertion:
    return StructuredAssertion(
        assertion_id="assert-001",
        assertion_type=AssertionType.ENTITY_ATTRIBUTE,
        subject=AssertionSubject(
            entity_type="Server",
            entity_id="srv-1",
            display_name="web-01",
        ),
        predicate="has_status",
        object=AssertionObject(
            value="active",
            value_type=ValueType.STRING,
            formatted_value="active",
        ),
        confidence=ConfidenceInfo(
            score=0.95,
            method=ConfidenceMethod.EXTRACTED,
            factors=[
                ConfidenceFactor(factor="direct_extraction", contribution=0.8),
                ConfidenceFactor(factor="schema_match", contribution=0.15),
            ],
        ),
        source=SourceInfo(
            tool_id="test-tool",
            tool_name="TestTool",
            invocation_id="inv-001",
            data_location=DataLocation(
                format=DataLocationFormat.JSON_PATH,
                path="$.status",
                line_number=1,
                column_number=10,
            ),
        ),
        metadata=AssertionMetadata(
            extracted_at="2025-01-01T00:00:00Z",
            parsing_duration_ms=42,
            parser_version="1.0.0",
            original_format=FormatType.JSON,
            is_partial=False,
            warnings=["minor_warning"],
            tags=["integration", "test"],
        ),
    )

# PyO3Bridge 在无原生核心时返回 simulated 状态，submit_assertions 和 verify_prediction 均如此

def test_pyo3_bridge_fallback_mode():
    bridge = PyO3Bridge()
    assertion = _make_test_assertion()

    result = bridge.submit_assertions([assertion])
    assert "status" in result
    assert result["status"] in ("simulated", "accepted")
    if not bridge.is_native:
        assert result["status"] == "simulated"
        assert result["accepted"] == 1
        assert assertion.assertion_id in result["assertion_ids"]

    verify_result = bridge.verify_prediction("pred-001", [{"obs": "value"}])
    assert "status" in verify_result
    assert verify_result["status"] in ("simulated", "verified")
    if not bridge.is_native:
        assert verify_result["status"] == "simulated"

# is_native 属性为布尔值

def test_pyo3_bridge_is_native_property():
    bridge = PyO3Bridge()
    assert isinstance(bridge.is_native, bool)

# 构建完整 StructuredAssertion → to_dict() → from_dict() → 逐字段验证所有嵌套结构一致（含 ConfidenceFactor、DataLocation、AssertionMetadata 等）

def test_structured_assertion_roundtrip():
    original = _make_test_assertion()
    data = original.to_dict()

    restored = StructuredAssertion.from_dict(data)

    assert restored.assertion_id == original.assertion_id
    assert restored.assertion_type == original.assertion_type
    assert restored.subject.entity_type == original.subject.entity_type
    assert restored.subject.entity_id == original.subject.entity_id
    assert restored.subject.display_name == original.subject.display_name
    assert restored.predicate == original.predicate
    assert restored.object.value == original.object.value
    assert restored.object.value_type == original.object.value_type
    assert restored.object.formatted_value == original.object.formatted_value
    assert restored.confidence.score == original.confidence.score
    assert restored.confidence.method == original.confidence.method
    assert len(restored.confidence.factors) == len(original.confidence.factors)
    for rf, of in zip(restored.confidence.factors, original.confidence.factors):
        assert rf.factor == of.factor
        assert rf.contribution == of.contribution
    assert restored.source.tool_id == original.source.tool_id
    assert restored.source.tool_name == original.source.tool_name
    assert restored.source.invocation_id == original.source.invocation_id
    assert restored.source.data_location.format == original.source.data_location.format
    assert restored.source.data_location.path == original.source.data_location.path
    assert restored.source.data_location.line_number == original.source.data_location.line_number
    assert restored.source.data_location.column_number == original.source.data_location.column_number
    assert restored.metadata.extracted_at == original.metadata.extracted_at
    assert restored.metadata.parsing_duration_ms == original.metadata.parsing_duration_ms
    assert restored.metadata.parser_version == original.metadata.parser_version
    assert restored.metadata.original_format == original.metadata.original_format
    assert restored.metadata.is_partial == original.metadata.is_partial
    assert restored.metadata.warnings == original.metadata.warnings
    assert restored.metadata.tags == original.metadata.tags

# 解析 JSON → submit_to_core 返回正确格式 → verify_prediction 返回 prediction_id 和 status

def test_tool_adapter_with_pyo3_bridge_integration(json_raw_output):
    adapter = ToolAdapter()
    parse_result = adapter.parse_tool_output(json_raw_output)
    assert parse_result.success
    assert len(parse_result.assertions) > 0

    submit_result = adapter.submit_to_core(parse_result.assertions)
    assert "status" in submit_result
    assert submit_result["status"] in ("simulated", "accepted")
    assert "assertion_ids" in submit_result

    verify_result = adapter.verify_prediction(
        "pred-cross-001",
        [{"entity": "srv-1", "attribute": "status", "observed": "active"}],
    )
    assert "prediction_id" in verify_result
    assert "status" in verify_result
    assert verify_result["prediction_id"] == "pred-cross-001"

# 对 JSON/HTML/CSV 内容调用 identify_format → 返回 FormatIdentification 且 format 字段和 confidence 正确

def test_tool_adapter_identify_format():
    adapter = ToolAdapter()

    json_id = adapter.identify_format(RawOutput(content='{"type":"Server","id":"srv-1"}'))
    assert isinstance(json_id, FormatIdentification)
    assert json_id.format == FormatType.JSON
    assert json_id.confidence > 0

    html_id = adapter.identify_format(
        RawOutput(content="<html><body><table><tr><td>data</td></tr></table></body></html>")
    )
    assert isinstance(html_id, FormatIdentification)
    assert html_id.format == FormatType.HTML
    assert html_id.confidence > 0

    csv_id = adapter.identify_format(RawOutput(content="Name,Status,CPU\nweb-01,active,85"))
    assert isinstance(csv_id, FormatIdentification)
    assert csv_id.format == FormatType.CSV
    assert csv_id.confidence > 0
