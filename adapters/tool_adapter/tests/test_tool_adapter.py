from __future__ import annotations

from pbsm_tool_adapter import (
    ToolAdapter,
    FormatType,
    RawOutput,
    ParseOptions,
    ToolSpecification,
    AssertionType,
)


def test_adapter_creation():
    adapter = ToolAdapter()
    assert adapter is not None


def test_adapter_native_mode():
    adapter = ToolAdapter()
    assert isinstance(adapter.is_native_mode, bool)


def test_parse_json(json_raw_output):
    adapter = ToolAdapter()
    result = adapter.parse_tool_output(json_raw_output)
    assert result.success
    assert len(result.assertions) > 0


def test_parse_html(html_raw_output):
    adapter = ToolAdapter()
    result = adapter.parse_tool_output(html_raw_output)
    assert result.success


def test_parse_text(text_raw_output):
    adapter = ToolAdapter()
    result = adapter.parse_tool_output(text_raw_output)
    assert result.success


def test_parse_csv(csv_raw_output):
    adapter = ToolAdapter()
    result = adapter.parse_tool_output(csv_raw_output)
    assert result.success
    assert len(result.assertions) > 0


def test_parse_error(error_raw_output):
    adapter = ToolAdapter()
    result = adapter.parse_tool_output(error_raw_output)
    assert result.success


def test_identify_format_json():
    adapter = ToolAdapter()
    result = adapter.identify_format(RawOutput(content='{"key":"val"}'))
    assert result.format == FormatType.JSON


def test_identify_format_html():
    adapter = ToolAdapter()
    result = adapter.identify_format(RawOutput(content="<html></html>"))
    assert result.format == FormatType.HTML


def test_force_format():
    adapter = ToolAdapter()
    json_content = RawOutput(content='{"key":"val"}')
    options = ParseOptions(force_format=FormatType.TEXT)
    result = adapter.parse_tool_output(json_content, options=options)
    assert result.format == FormatType.TEXT


def test_confidence_threshold(json_raw_output):
    adapter = ToolAdapter()
    options = ParseOptions(confidence_threshold=0.9)
    result = adapter.parse_tool_output(json_raw_output, options=options)
    for assertion in result.assertions:
        assert assertion.confidence.score >= 0.9


def test_register_tool(sample_tool_spec):
    adapter = ToolAdapter()
    success, tool_id, warnings = adapter.register_tool(sample_tool_spec)
    assert success
    assert tool_id == sample_tool_spec.tool_id


def test_parse_empty_input():
    adapter = ToolAdapter()
    result = adapter.parse_tool_output(RawOutput(content=""))
    assert result is not None


def test_submit_to_core():
    adapter = ToolAdapter()
    result = adapter.submit_to_core([])
    assert "status" in result


def test_verify_prediction():
    adapter = ToolAdapter()
    result = adapter.verify_prediction("pred-1", [])
    assert "status" in result
