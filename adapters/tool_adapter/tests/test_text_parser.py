from __future__ import annotations

import pytest
from pbsm_tool_adapter import (
    TextParser,
    FormatType,
    RawOutput,
    ConfidenceMethod,
    DataLocationFormat,
)


class TestTextParserCanParse:
    def test_can_parse_text(self):
        parser = TextParser()
        result = parser.can_parse(
            RawOutput(content="Status: running\nCPU: 75%")
        )
        assert result.can_parse is True

    def test_can_parse_rejects_json(self):
        parser = TextParser()
        result = parser.can_parse(
            RawOutput(content='{"key": "value"}')
        )
        assert result.can_parse is False

    def test_can_parse_rejects_html(self):
        parser = TextParser()
        result = parser.can_parse(RawOutput(content="<html></html>"))
        assert result.can_parse is False


class TestTextParserParse:
    def test_parse_key_value_pairs(self):
        parser = TextParser()
        result = parser.parse(
            RawOutput(content="Name: Server1\nStatus: running")
        )
        assert result.success is True
        predicates = [a.predicate for a in result.assertions]
        assert "Name" in predicates
        assert "Status" in predicates

    def test_parse_numeric_values(self):
        parser = TextParser()
        result = parser.parse(
            RawOutput(content="CPU Usage: 75.5%\nMemory: 8.2 GB")
        )
        assert result.success is True
        values = [a.object.value for a in result.assertions]
        assert any("75.5" in str(v) for v in values)

    def test_parse_dates(self):
        parser = TextParser()
        result = parser.parse(
            RawOutput(content="Date: 2024-01-15\nUpdated: 2024/02/20")
        )
        assert result.success is True
        values = [a.object.value for a in result.assertions]
        assert any("2024" in str(v) for v in values)

    def test_parse_status_words(self):
        parser = TextParser()
        result = parser.parse(
            RawOutput(content="Status: active\nMode: enabled")
        )
        assert result.success is True
        values = [a.object.value for a in result.assertions]
        assert "ACTIVE" in values or "ENABLED" in values

    def test_parse_bytes_input(self):
        parser = TextParser()
        result = parser.parse(RawOutput(content=b"Status: running"))
        assert result.success is True


class TestTextParserMetadata:
    def test_data_location_line_number(self):
        parser = TextParser()
        result = parser.parse(
            RawOutput(content="Name: Server1\nStatus: running")
        )
        assert result.success is True
        assert any(
            a.source.data_location.format == DataLocationFormat.LINE_NUMBER
            for a in result.assertions
        )

    def test_confidence_method_inferred(self):
        parser = TextParser()
        result = parser.parse(
            RawOutput(content="Status: active\nMode: enabled")
        )
        assert result.success is True
        assert any(
            a.confidence.method == ConfidenceMethod.INFERRED
            for a in result.assertions
        )
