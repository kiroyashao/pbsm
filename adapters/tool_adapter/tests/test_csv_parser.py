from __future__ import annotations

import pytest
from pbsm_tool_adapter import (
    CsvParser,
    FormatType,
    RawOutput,
    ConfidenceMethod,
    DataLocationFormat,
)


class TestCsvParserCanParse:
    def test_can_parse_csv(self):
        parser = CsvParser()
        result = parser.can_parse(
            RawOutput(content="Name,Value\nCPU,75%\nMemory,8GB")
        )
        assert result.can_parse is True

    def test_can_parse_tsv(self):
        parser = CsvParser()
        result = parser.can_parse(
            RawOutput(content="Name\tValue\nCPU\t75%")
        )
        assert result.can_parse is True

    def test_can_parse_non_csv(self):
        parser = CsvParser()
        result = parser.can_parse(
            RawOutput(content="just plain text")
        )
        assert result.can_parse is False


class TestCsvParserParse:
    def test_parse_csv_with_header(self):
        parser = CsvParser()
        result = parser.parse(
            RawOutput(content="Name,Type,Status\nserver-1,Server,running")
        )
        assert result.success is True
        predicates = [a.predicate for a in result.assertions]
        assert "Name" in predicates
        assert "Type" in predicates
        assert "Status" in predicates

    def test_parse_csv_without_header(self):
        parser = CsvParser(has_header=False)
        result = parser.parse(
            RawOutput(content="a,b,c\n1,2,3")
        )
        assert result.success is True
        predicates = [a.predicate for a in result.assertions]
        assert "col_0" in predicates
        assert "col_1" in predicates
        assert "col_2" in predicates

    def test_parse_csv_quoted(self):
        parser = CsvParser()
        result = parser.parse(
            RawOutput(content='Name,Value\n"Server 1","75%"')
        )
        assert result.success is True
        values = [a.object.value for a in result.assertions]
        assert "Server 1" in values
        assert "75%" in values


class TestCsvParserMetadata:
    def test_data_location_cell_reference(self):
        parser = CsvParser()
        result = parser.parse(
            RawOutput(content="Name,Value\nCPU,75%")
        )
        assert result.success is True
        assert any(
            a.source.data_location.format == DataLocationFormat.CELL_REFERENCE
            for a in result.assertions
        )

    def test_confidence_method_extracted(self):
        parser = CsvParser()
        result = parser.parse(
            RawOutput(content="Name,Value\nCPU,75%")
        )
        assert result.success is True
        assert any(
            a.confidence.method == ConfidenceMethod.EXTRACTED
            for a in result.assertions
        )


class TestCsvParserDelimiter:
    def test_detect_delimiter_comma(self):
        parser = CsvParser()
        content = "Name,Value\nCPU,75%"
        assert parser._detect_delimiter(content) == ","

    def test_detect_delimiter_tab(self):
        parser = CsvParser()
        content = "Name\tValue\nCPU\t75%"
        assert parser._detect_delimiter(content) == "\t"
