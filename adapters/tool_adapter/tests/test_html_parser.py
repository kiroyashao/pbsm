from __future__ import annotations

import pytest
from pbsm_tool_adapter import (
    HtmlParser,
    FormatType,
    RawOutput,
    ConfidenceMethod,
    DataLocationFormat,
)


class TestHtmlParserCanParse:
    def test_can_parse_valid_html(self):
        parser = HtmlParser()
        result = parser.can_parse(
            RawOutput(content="<html><body>Hello</body></html>")
        )
        assert result.can_parse is True

    def test_can_parse_non_html(self):
        parser = HtmlParser()
        result = parser.can_parse(RawOutput(content="plain text"))
        assert result.can_parse is False


class TestHtmlParserParse:
    def test_parse_table(self):
        parser = HtmlParser()
        html = (
            "<table><tr><th>Name</th><th>Value</th></tr>"
            "<tr><td>CPU</td><td>75%</td></tr></table>"
        )
        result = parser.parse(RawOutput(content=html))
        assert result.success is True
        values = [a.object.value for a in result.assertions]
        assert "CPU" in values
        assert "75%" in values

    def test_parse_list(self):
        parser = HtmlParser()
        html = "<ul><li>Item 1</li><li>Item 2</li></ul>"
        result = parser.parse(RawOutput(content=html))
        assert result.success is True
        values = [a.object.value for a in result.assertions]
        assert "Item 1" in values
        assert "Item 2" in values

    def test_parse_article(self):
        parser = HtmlParser()
        html = "<article><h2>Title</h2><p>Content</p></article>"
        result = parser.parse(RawOutput(content=html))
        assert result.success is True
        values = [a.object.value for a in result.assertions]
        assert "Title" in values
        assert "Content" in values

    def test_clean_scripts(self):
        parser = HtmlParser(clean_scripts=True)
        html = (
            "<html><script>var x=1;</script>"
            "<body>Hello</body></html>"
        )
        result = parser.parse(RawOutput(content=html))
        assert result.success is True
        for assertion in result.assertions:
            assert "var x=1" not in str(assertion.object.value)

    def test_parse_title(self):
        parser = HtmlParser()
        html = (
            "<html><head><title>Test Page</title></head>"
            "<body></body></html>"
        )
        result = parser.parse(RawOutput(content=html))
        assert result.success is True
        values = [a.object.value for a in result.assertions]
        assert "Test Page" in values


class TestHtmlParserMetadata:
    def test_data_location_css_selector(self):
        parser = HtmlParser()
        html = (
            "<table><tr><th>Name</th><th>Value</th></tr>"
            "<tr><td>CPU</td><td>75%</td></tr></table>"
        )
        result = parser.parse(RawOutput(content=html))
        assert result.success is True
        assert any(
            a.source.data_location.format == DataLocationFormat.CSS_SELECTOR
            for a in result.assertions
        )

    def test_confidence_method_extracted(self):
        parser = HtmlParser()
        html = (
            "<table><tr><th>Name</th><th>Value</th></tr>"
            "<tr><td>CPU</td><td>75%</td></tr></table>"
        )
        result = parser.parse(RawOutput(content=html))
        assert result.success is True
        assert any(
            a.confidence.method == ConfidenceMethod.EXTRACTED
            for a in result.assertions
        )


class TestHtmlParserValidate:
    def test_validate_valid_html(self):
        parser = HtmlParser()
        result = parser.validate(RawOutput(content="<html></html>"))
        assert result.is_valid is True
