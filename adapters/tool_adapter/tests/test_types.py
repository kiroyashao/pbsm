from __future__ import annotations

import pytest
from pbsm_tool_adapter import (
    FormatType,
    AssertionType,
    ValueType,
    ConfidenceMethod,
    DataLocationFormat,
    InvocationStatus,
    ToolStatus,
    AuthenticationType,
    ErrorSeverity,
    ConfidenceFactor,
    ConfidenceInfo,
    DataLocation,
    SourceInfo,
    AssertionMetadata,
    AssertionSubject,
    AssertionObject,
    StructuredAssertion,
    RawOutput,
    ParseOptions,
    ParseResult,
    ToolSpecification,
    InvocationResult,
    InvocationStatus,
    ToolAdapterError,
)


def test_format_type_values():
    expected = ["JSON", "HTML", "TEXT", "CSV", "ERROR", "UNKNOWN"]
    actual = [e.value for e in FormatType]
    assert actual == expected


def test_assertion_type_values():
    expected = ["ENTITY_ATTRIBUTE", "RELATION", "EVENT", "ERROR", "ERROR_CONTEXT", "METADATA"]
    actual = [e.value for e in AssertionType]
    assert actual == expected


def test_value_type_values():
    expected = ["STRING", "NUMBER", "BOOLEAN", "NULL", "ARRAY", "OBJECT", "DATE", "DURATION"]
    actual = [e.value for e in ValueType]
    assert actual == expected


def test_confidence_method_values():
    expected = ["EXACT", "EXTRACTED", "INFERRED", "ASSUMED"]
    actual = [e.value for e in ConfidenceMethod]
    assert actual == expected


def test_structured_assertion_creation():
    assertion = StructuredAssertion(
        assertion_id="assert-001",
        assertion_type=AssertionType.ENTITY_ATTRIBUTE,
        subject=AssertionSubject(
            entity_type="Server",
            entity_id="server-1",
            display_name="web-01",
        ),
        predicate="hasStatus",
        object=AssertionObject(
            value="running",
            value_type=ValueType.STRING,
            formatted_value="running",
        ),
        confidence=ConfidenceInfo(
            score=0.95,
            method=ConfidenceMethod.EXACT,
            factors=[ConfidenceFactor(factor="direct_extraction", contribution=0.95)],
        ),
        source=SourceInfo(
            tool_id="test-tool",
            tool_name="TestTool",
            invocation_id="inv-001",
            data_location=DataLocation(
                format=DataLocationFormat.JSON_PATH,
                path="$.status",
                line_number=1,
                column_number=45,
            ),
        ),
        metadata=AssertionMetadata(
            extracted_at="2026-01-01T00:00:00Z",
            parsing_duration_ms=12,
            parser_version="1.0.0",
            original_format=FormatType.JSON,
            is_partial=False,
            warnings=[],
            tags=["server", "status"],
        ),
    )

    assert assertion.assertion_id == "assert-001"
    assert assertion.assertion_type == AssertionType.ENTITY_ATTRIBUTE
    assert assertion.subject.entity_type == "Server"
    assert assertion.subject.entity_id == "server-1"
    assert assertion.subject.display_name == "web-01"
    assert assertion.predicate == "hasStatus"
    assert assertion.object.value == "running"
    assert assertion.object.value_type == ValueType.STRING
    assert assertion.object.formatted_value == "running"
    assert assertion.confidence.score == 0.95
    assert assertion.confidence.method == ConfidenceMethod.EXACT
    assert len(assertion.confidence.factors) == 1
    assert assertion.confidence.factors[0].factor == "direct_extraction"
    assert assertion.confidence.factors[0].contribution == 0.95
    assert assertion.source.tool_id == "test-tool"
    assert assertion.source.tool_name == "TestTool"
    assert assertion.source.invocation_id == "inv-001"
    assert assertion.source.data_location.format == DataLocationFormat.JSON_PATH
    assert assertion.source.data_location.path == "$.status"
    assert assertion.source.data_location.line_number == 1
    assert assertion.source.data_location.column_number == 45
    assert assertion.metadata.extracted_at == "2026-01-01T00:00:00Z"
    assert assertion.metadata.parsing_duration_ms == 12
    assert assertion.metadata.parser_version == "1.0.0"
    assert assertion.metadata.original_format == FormatType.JSON
    assert assertion.metadata.is_partial is False
    assert assertion.metadata.warnings == []
    assert assertion.metadata.tags == ["server", "status"]


def test_structured_assertion_to_dict():
    assertion = StructuredAssertion(
        assertion_id="assert-002",
        assertion_type=AssertionType.RELATION,
        subject=AssertionSubject(
            entity_type="Server",
            entity_id="server-1",
            display_name="web-01",
        ),
        predicate="runsOn",
        object=AssertionObject(
            value="host-01",
            value_type=ValueType.STRING,
            formatted_value="host-01",
        ),
        confidence=ConfidenceInfo(
            score=0.85,
            method=ConfidenceMethod.INFERRED,
            factors=[],
        ),
        source=SourceInfo(
            tool_id="test-tool",
            tool_name="TestTool",
            invocation_id="inv-002",
            data_location=DataLocation(
                format=DataLocationFormat.JSON_PATH,
                path="$.host",
            ),
        ),
        metadata=AssertionMetadata(
            extracted_at="2026-01-01T00:00:00Z",
            original_format=FormatType.JSON,
        ),
    )

    result = assertion.to_dict()

    assert isinstance(result, dict)
    assert result["assertionId"] == "assert-002"
    assert result["assertionType"] == "RELATION"
    assert result["subject"]["entityType"] == "Server"
    assert result["subject"]["entityId"] == "server-1"
    assert result["subject"]["displayName"] == "web-01"
    assert result["predicate"] == "runsOn"
    assert result["object"]["value"] == "host-01"
    assert result["object"]["valueType"] == "STRING"
    assert result["object"]["formattedValue"] == "host-01"
    assert result["confidence"]["score"] == 0.85
    assert result["confidence"]["method"] == "INFERRED"
    assert result["confidence"]["factors"] == []
    assert result["source"]["toolId"] == "test-tool"
    assert result["source"]["toolName"] == "TestTool"
    assert result["source"]["invocationId"] == "inv-002"
    assert result["source"]["dataLocation"]["format"] == "JSON_PATH"
    assert result["source"]["dataLocation"]["path"] == "$.host"
    assert result["metadata"]["extractedAt"] == "2026-01-01T00:00:00Z"
    assert result["metadata"]["originalFormat"] == "JSON"


def test_structured_assertion_from_dict():
    original = StructuredAssertion(
        assertion_id="assert-003",
        assertion_type=AssertionType.EVENT,
        subject=AssertionSubject(
            entity_type="Server",
            entity_id="server-1",
            display_name="web-01",
        ),
        predicate="statusChanged",
        object=AssertionObject(
            value="stopped",
            value_type=ValueType.STRING,
            formatted_value="stopped",
        ),
        confidence=ConfidenceInfo(
            score=0.90,
            method=ConfidenceMethod.EXTRACTED,
            factors=[ConfidenceFactor(factor="pattern_match", contribution=0.90)],
        ),
        source=SourceInfo(
            tool_id="test-tool",
            tool_name="TestTool",
            invocation_id="inv-003",
            data_location=DataLocation(
                format=DataLocationFormat.LINE_NUMBER,
                path="line",
                line_number=5,
                column_number=10,
            ),
        ),
        metadata=AssertionMetadata(
            extracted_at="2026-01-01T00:00:00Z",
            parsing_duration_ms=8,
            parser_version="1.0.0",
            original_format=FormatType.TEXT,
            is_partial=False,
            warnings=["low_confidence_field"],
            tags=["event", "status"],
        ),
    )

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
    assert restored.confidence.factors[0].factor == original.confidence.factors[0].factor
    assert restored.confidence.factors[0].contribution == original.confidence.factors[0].contribution
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


def test_raw_output_str():
    output = RawOutput(
        content='{"key": "value"}',
        content_type="application/json",
        metadata={"tool_id": "t1"},
        status_code=200,
    )
    assert output.content == '{"key": "value"}'
    assert output.content_type == "application/json"
    assert output.metadata == {"tool_id": "t1"}
    assert output.status_code == 200


def test_raw_output_bytes():
    output = RawOutput(
        content=b"binary data",
        content_type="application/octet-stream",
    )
    assert output.content == b"binary data"
    assert output.content_type == "application/octet-stream"
    assert output.metadata is None
    assert output.status_code is None


def test_parse_options_defaults():
    opts = ParseOptions()
    assert opts.force_format is None
    assert opts.max_depth == 10
    assert opts.max_nodes == 1000
    assert opts.timeout_ms == 5000
    assert opts.confidence_threshold == 0.0
    assert opts.custom_options is None


def test_parse_result_creation():
    assertion = StructuredAssertion(
        assertion_id="assert-pr-001",
        assertion_type=AssertionType.ENTITY_ATTRIBUTE,
        subject=AssertionSubject(entity_type="Host", entity_id="host-1"),
        predicate="hasCpu",
        object=AssertionObject(value=75.5, value_type=ValueType.NUMBER),
        confidence=ConfidenceInfo(score=0.9, method=ConfidenceMethod.EXACT),
        source=SourceInfo(
            tool_id="test-tool",
            tool_name="TestTool",
            invocation_id="inv-001",
            data_location=DataLocation(format=DataLocationFormat.JSON_PATH, path="$.cpu"),
        ),
        metadata=AssertionMetadata(
            extracted_at="2026-01-01T00:00:00Z",
            original_format=FormatType.JSON,
        ),
    )
    result = ParseResult(
        success=True,
        assertions=[assertion],
        format=FormatType.JSON,
        format_confidence=0.95,
        warnings=[],
        errors=[],
        parsing_duration_ms=5.0,
        is_partial=False,
    )
    assert result.success is True
    assert len(result.assertions) == 1
    assert result.assertions[0].assertion_id == "assert-pr-001"
    assert result.format == FormatType.JSON
    assert result.format_confidence == 0.95
    assert result.warnings == []
    assert result.errors == []
    assert result.parsing_duration_ms == 5.0
    assert result.is_partial is False


def test_tool_specification_creation():
    spec = ToolSpecification(
        tool_id="test-tool",
        tool_name="TestTool",
        endpoint="http://localhost:8080/api",
        supported_formats=[FormatType.JSON, FormatType.TEXT],
        description="A test tool",
        version="2.0.0",
        authentication={"type": "API_KEY"},
        parser_config={"max_depth": 20},
        max_response_size=1048576,
        timeout_ms=60000,
    )
    assert spec.tool_id == "test-tool"
    assert spec.tool_name == "TestTool"
    assert spec.endpoint == "http://localhost:8080/api"
    assert spec.supported_formats == [FormatType.JSON, FormatType.TEXT]
    assert spec.description == "A test tool"
    assert spec.version == "2.0.0"
    assert spec.authentication == {"type": "API_KEY"}
    assert spec.parser_config == {"max_depth": 20}
    assert spec.max_response_size == 1048576
    assert spec.timeout_ms == 60000


def test_invocation_result_creation():
    result = InvocationResult(
        success=True,
        call_id="call-001",
        tool_id="test-tool",
        status=InvocationStatus.SUCCESS,
        status_code=200,
        headers={"Content-Type": "application/json"},
        body='{"result": "ok"}',
        assertions=None,
        parse_result=None,
        error=None,
        timestamps={"started": "2026-01-01T00:00:00Z", "completed": "2026-01-01T00:00:01Z"},
    )
    assert result.success is True
    assert result.call_id == "call-001"
    assert result.tool_id == "test-tool"
    assert result.status == InvocationStatus.SUCCESS
    assert result.status_code == 200
    assert result.headers == {"Content-Type": "application/json"}
    assert result.body == '{"result": "ok"}'
    assert result.assertions is None
    assert result.parse_result is None
    assert result.error is None
    assert result.timestamps == {"started": "2026-01-01T00:00:00Z", "completed": "2026-01-01T00:00:01Z"}


def test_tool_adapter_error_creation():
    error = ToolAdapterError(
        code="PARSE_FAILED",
        message="Failed to parse output",
        details={"format": "JSON", "line": 42},
        recoverable=True,
        retryable=False,
        timestamp="2026-01-01T00:00:00Z",
        correlation_id="corr-001",
    )
    assert error.code == "PARSE_FAILED"
    assert error.message == "Failed to parse output"
    assert error.details == {"format": "JSON", "line": 42}
    assert error.recoverable is True
    assert error.retryable is False
    assert error.timestamp == "2026-01-01T00:00:00Z"
    assert error.correlation_id == "corr-001"
