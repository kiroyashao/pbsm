from __future__ import annotations

import pytest
from pbsm_tool_adapter import (
    JsonParser,
    FormatType,
    RawOutput,
    ParseOptions,
    ConfidenceMethod,
    DataLocationFormat,
    AssertionType,
)


class TestJsonParserCanParse:
    def test_can_parse_valid_json_object(self):
        parser = JsonParser()
        result = parser.can_parse(RawOutput(content='{"key": "value"}'))
        assert result.can_parse is True
        assert result.confidence > 0.8

    def test_can_parse_valid_json_array(self):
        parser = JsonParser()
        result = parser.can_parse(RawOutput(content="[1,2,3]"))
        assert result.can_parse is True

    def test_can_parse_invalid_json(self):
        parser = JsonParser()
        result = parser.can_parse(RawOutput(content="not json"))
        assert result.can_parse is False

    def test_can_parse_bytes_input(self):
        parser = JsonParser()
        result = parser.can_parse(RawOutput(content=b'{"key": "value"}'))
        assert result.can_parse is True


class TestJsonParserParse:
    def test_parse_simple_object(self):
        parser = JsonParser()
        result = parser.parse(RawOutput(content='{"name": "test", "value": 42}'))
        assert result.success is True
        assert len(result.assertions) > 0
        assert any(
            a.assertion_type == AssertionType.ENTITY_ATTRIBUTE
            for a in result.assertions
        )

    def test_parse_nested_object(self):
        parser = JsonParser()
        result = parser.parse(
            RawOutput(content='{"server": {"id": "s1", "cpu": 75}}')
        )
        assert result.success is True
        predicates = [a.predicate for a in result.assertions]
        assert "cpu" in predicates
        assert "id" in predicates

    def test_parse_array_of_objects(self):
        parser = JsonParser()
        result = parser.parse(
            RawOutput(
                content='[{"id": "1", "name": "a"}, {"id": "2", "name": "b"}]'
            )
        )
        assert result.success is True
        assert len(result.assertions) >= 4

    def test_parse_with_id_field(self):
        parser = JsonParser()
        result = parser.parse(
            RawOutput(content='{"id": "server-1", "status": "running"}')
        )
        assert result.success is True
        entity_ids = {a.subject.entity_id for a in result.assertions}
        assert "server-1" in entity_ids

    def test_parse_with_type_field(self):
        parser = JsonParser()
        result = parser.parse(
            RawOutput(content='{"type": "Server", "name": "web-01"}')
        )
        assert result.success is True
        entity_types = {a.subject.entity_type for a in result.assertions}
        assert "Server" in entity_types

    def test_parse_max_depth(self):
        parser = JsonParser(max_depth=2)
        data = '{"a": {"b": {"c": {"d": "deep"}}}}'
        result = parser.parse(RawOutput(content=data))
        assert result.is_partial is True

    def test_parse_max_nodes(self):
        parser = JsonParser(max_nodes=5)
        items = ", ".join(f'{{"id": "{i}", "v": {i}}}' for i in range(20))
        data = f"[{items}]"
        result = parser.parse(RawOutput(content=data))
        assert result.is_partial is True

    def test_parse_empty_object(self):
        parser = JsonParser()
        result = parser.parse(RawOutput(content="{}"))
        assert result.success is True
        assert len(result.assertions) <= 1


class TestJsonParserValidate:
    def test_validate_valid_json(self):
        parser = JsonParser()
        result = parser.validate(RawOutput(content='{"key": "value"}'))
        assert result.is_valid is True

    def test_validate_invalid_json(self):
        parser = JsonParser()
        result = parser.validate(RawOutput(content="{bad"))
        assert result.is_valid is False


class TestJsonParserMetadata:
    def test_confidence_method_extracted(self):
        parser = JsonParser()
        result = parser.parse(RawOutput(content='{"key": "value"}'))
        assert result.success is True
        assert any(
            a.confidence.method == ConfidenceMethod.EXTRACTED
            for a in result.assertions
        )

    def test_data_location_json_path(self):
        parser = JsonParser()
        result = parser.parse(RawOutput(content='{"key": "value"}'))
        assert result.success is True
        assert any(
            a.source.data_location.format == DataLocationFormat.JSON_PATH
            for a in result.assertions
        )
