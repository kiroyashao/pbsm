from __future__ import annotations

from pbsm_tool_adapter import (
    ToolAdapter,
    FormatType,
    RawOutput,
    ToolSpecification,
    AssertionType,
    ParsingStartedEvent,
    ParsingCompletedEvent,
    AssertionBatchCompletedEvent,
)

# 注册工具 → 解析 JSON → 断言类型为 ENTITY_ATTRIBUTE → 事件总线有事件发布

def test_tool_adapter_full_parse_flow(json_raw_output, sample_tool_spec):
    adapter = ToolAdapter()
    success, tool_id, warnings = adapter.register_tool(sample_tool_spec)
    assert success
    assert tool_id == sample_tool_spec.tool_id

    result = adapter.parse_tool_output(
        raw_output=json_raw_output,
        tool_id=sample_tool_spec.tool_id,
        tool_name=sample_tool_spec.tool_name,
    )
    assert result.success
    assert len(result.assertions) > 0
    for assertion in result.assertions:
        assert assertion.assertion_type == AssertionType.ENTITY_ATTRIBUTE
        assert assertion.assertion_id
        assert assertion.subject
        assert assertion.predicate
        assert assertion.object
        assert assertion.confidence.score > 0

    received_events = []
    adapter.event_bus.subscribe("", lambda e: received_events.append(e))
    adapter.parse_tool_output(
        raw_output=json_raw_output,
        tool_id=sample_tool_spec.tool_id,
    )
    assert len(received_events) > 0

# 解析输出 → 调用 submit_to_core → 返回 dict 包含 status（simulated 或 accepted）和 accepted 计数

def test_assertion_submission_to_core(json_raw_output):
    adapter = ToolAdapter()
    result = adapter.parse_tool_output(json_raw_output)
    assert result.success
    assert len(result.assertions) > 0

    submission = adapter.submit_to_core(result.assertions)
    assert "status" in submission
    assert submission["status"] in ("simulated", "accepted")
    if submission["status"] == "accepted":
        assert "assertion_ids" in submission
        assert submission["count"] == len(result.assertions)
    else:
        assert submission["accepted"] == len(result.assertions)

# 调用 verify_prediction → 返回 dict 包含 prediction_id 和 status

def test_prediction_verification_bridge():
    adapter = ToolAdapter()
    prediction_id = "pred-test-001"
    observations = [{"key": "cpu", "value": 85.0}]

    result = adapter.verify_prediction(prediction_id, observations)
    assert "prediction_id" in result
    assert "status" in result
    assert result["prediction_id"] == prediction_id
    assert result["status"] in ("simulated", "verified")

# 订阅事件总线 → 解析 JSON → 验证 ParsingStartedEvent、ParsingCompletedEvent、AssertionBatchCompletedEvent 均被触发，并检查事件字段

def test_event_bus_end_to_end(json_raw_output):
    adapter = ToolAdapter()
    received_events = []
    adapter.event_bus.subscribe("", lambda e: received_events.append(e))

    adapter.parse_tool_output(
        raw_output=json_raw_output,
        tool_id="test-tool",
    )

    event_types = [type(e) for e in received_events]
    assert ParsingStartedEvent in event_types
    assert ParsingCompletedEvent in event_types
    assert AssertionBatchCompletedEvent in event_types

    started_events = [e for e in received_events if isinstance(e, ParsingStartedEvent)]
    assert len(started_events) >= 1
    assert started_events[0].tool_id == "test-tool"
    assert started_events[0].raw_output_size > 0

    completed_events = [e for e in received_events if isinstance(e, ParsingCompletedEvent)]
    assert len(completed_events) >= 1
    assert completed_events[0].actual_format == FormatType.JSON
    assert completed_events[0].assertion_count > 0

    batch_events = [e for e in received_events if isinstance(e, AssertionBatchCompletedEvent)]
    assert len(batch_events) >= 1
    assert batch_events[0].total_assertions > 0

# 依次解析 JSON/HTML/CSV/ERROR/TEXT → 验证每种格式的 format 字段和断言类型正确（ERROR 格式产生 ERROR/ERROR_CONTEXT 断言）

def test_multi_format_cascade(
    json_raw_output,
    html_raw_output,
    csv_raw_output,
    error_raw_output,
    text_raw_output,
):
    adapter = ToolAdapter()

    json_result = adapter.parse_tool_output(json_raw_output)
    assert json_result.success
    assert json_result.format == FormatType.JSON
    assert len(json_result.assertions) > 0
    entity_assertions = [
        a for a in json_result.assertions
        if a.assertion_type == AssertionType.ENTITY_ATTRIBUTE
    ]
    assert len(entity_assertions) > 0

    html_result = adapter.parse_tool_output(html_raw_output)
    assert html_result.success
    assert html_result.format == FormatType.HTML
    assert len(html_result.assertions) > 0

    csv_result = adapter.parse_tool_output(csv_raw_output)
    assert csv_result.success
    assert csv_result.format == FormatType.CSV
    assert len(csv_result.assertions) > 0

    error_result = adapter.parse_tool_output(error_raw_output)
    assert error_result.success
    assert error_result.format == FormatType.ERROR
    error_assertions = [
        a for a in error_result.assertions
        if a.assertion_type in (AssertionType.ERROR, AssertionType.ERROR_CONTEXT)
    ]
    assert len(error_assertions) > 0

    text_result = adapter.parse_tool_output(text_raw_output)
    assert text_result.success
    assert text_result.format == FormatType.TEXT
    assert len(text_result.assertions) > 0
