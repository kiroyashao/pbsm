from __future__ import annotations

import pytest
from pbsm_tool_adapter import (
    ErrorParser,
    FormatType,
    RawOutput,
    AssertionType,
    ConfidenceMethod,
)


class TestErrorParserCanParse:
    def test_can_parse_http_error(self):
        parser = ErrorParser()
        result = parser.can_parse(RawOutput(content="", status_code=500))
        assert result.can_parse is True
        assert result.confidence == 0.95

    def test_can_parse_json_error(self):
        parser = ErrorParser()
        result = parser.can_parse(
            RawOutput(
                content='{"error": {"code": "SVC_503", "message": "Unavailable"}}'
            )
        )
        assert result.can_parse is True

    def test_can_parse_text_error(self):
        parser = ErrorParser()
        result = parser.can_parse(
            RawOutput(content="Error: connection failed")
        )
        assert result.can_parse is True

    def test_can_parse_non_error(self):
        parser = ErrorParser()
        result = parser.can_parse(
            RawOutput(content='{"status": "ok"}', status_code=200)
        )
        assert result.can_parse is False


class TestErrorParserParse:
    def test_parse_json_error(self):
        parser = ErrorParser()
        result = parser.parse(
            RawOutput(
                content='{"error": {"code": "SVC_503", "message": "Unavailable"}}'
            )
        )
        assert result.success is True
        assert any(
            a.assertion_type == AssertionType.ERROR for a in result.assertions
        )
        assert any(a.confidence.score == 0.95 for a in result.assertions)

    def test_parse_http_error(self):
        parser = ErrorParser()
        result = parser.parse(
            RawOutput(content="Not Found", status_code=404)
        )
        assert result.success is True
        values = [a.object.value for a in result.assertions]
        assert any("client_error" in str(v) for v in values)

    def test_parse_http_server_error(self):
        parser = ErrorParser()
        result = parser.parse(
            RawOutput(content="Internal Server Error", status_code=500)
        )
        assert result.success is True
        values = [a.object.value for a in result.assertions]
        assert any("server_error" in str(v) for v in values)

    def test_parse_text_error(self):
        parser = ErrorParser()
        result = parser.parse(
            RawOutput(content="Error: connection failed")
        )
        assert result.success is True
        assert any(
            a.assertion_type == AssertionType.ERROR for a in result.assertions
        )

    def test_error_context_assertions(self):
        parser = ErrorParser()
        result = parser.parse(
            RawOutput(
                content='{"error": {"code": "SVC_503", "message": "Unavailable", "details": {"reason": "timeout"}}}'
            )
        )
        assert result.success is True
        assert any(
            a.assertion_type == AssertionType.ERROR_CONTEXT
            for a in result.assertions
        )


class TestErrorParserMetadata:
    def test_confidence_method_exact(self):
        parser = ErrorParser()
        result = parser.parse(
            RawOutput(
                content='{"error": {"code": "SVC_503", "message": "Unavailable"}}'
            )
        )
        assert result.success is True
        assert any(
            a.confidence.method == ConfidenceMethod.EXACT
            for a in result.assertions
        )

    def test_priority_highest(self):
        parser = ErrorParser()
        assert parser.priority == 5
