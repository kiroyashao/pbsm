from __future__ import annotations

import re
import time
import uuid
from datetime import datetime
from typing import Any, Optional

from bs4 import BeautifulSoup, Tag

from ..types import (
    AssertionType,
    ConfidenceMethod,
    DataLocationFormat,
    FormatType,
    ParseOptions,
    ParseResult,
    ParseabilityResult,
    RawOutput,
    StructuredAssertion,
    ValidationResult,
    ValueType,
)
from .base_parser import FormatParser, create_assertion


class HtmlParser(FormatParser):

    def __init__(
        self,
        extract_tables: bool = True,
        extract_lists: bool = True,
        max_text_length: int = 10000,
        clean_scripts: bool = True,
    ):
        self.format = FormatType.HTML
        self.version = "1.0.0"
        self.priority = 20
        self._nodes_processed = 0
        self.extract_tables = extract_tables
        self.extract_lists = extract_lists
        self.max_text_length = max_text_length
        self.clean_scripts = clean_scripts

    def can_parse(self, input: RawOutput) -> ParseabilityResult:
        try:
            content = self._decode_content(input.content)
            soup = BeautifulSoup(content, "html.parser")
            structural_tags = ["html", "body", "div", "article", "section"]
            found_tags = []
            for tag_name in structural_tags:
                if soup.find(tag_name):
                    found_tags.append(tag_name)
            if not found_tags:
                return ParseabilityResult(
                    can_parse=False,
                    confidence=0.0,
                    estimated_complexity="low",
                    detected_features=[],
                )
            has_table_elements = bool(
                soup.find("table") or soup.find("th") or soup.find("td")
            )
            confidence = 0.90 if has_table_elements else 0.75
            return ParseabilityResult(
                can_parse=True,
                confidence=confidence,
                estimated_complexity="medium" if has_table_elements else "low",
                detected_features=found_tags,
            )
        except Exception:
            return ParseabilityResult(
                can_parse=False,
                confidence=0.0,
                estimated_complexity="unknown",
                detected_features=[],
            )

    def parse(
        self, input: RawOutput, options: ParseOptions | None = None
    ) -> ParseResult:
        start_time = time.monotonic()
        assertions: list[StructuredAssertion] = []
        warnings: list[str] = []
        errors: list[str] = []
        try:
            content = self._decode_content(input.content)
            soup = BeautifulSoup(content, "html.parser")
            if self.clean_scripts:
                for tag in soup.find_all(["script", "style"]):
                    tag.decompose()
            self._nodes_processed = 0
            tool_id = (input.metadata or {}).get("tool_id", "unknown")
            tool_name = (input.metadata or {}).get("tool_name", "unknown")
            invocation_id = (input.metadata or {}).get("invocation_id", str(uuid.uuid4()))
            source_url = (input.metadata or {}).get("source_url", "")
            title = self._extract_title(soup)
            if title:
                assertions.append(
                    create_assertion(
                        assertion_type=AssertionType.METADATA,
                        subject_type="document",
                        subject_id=source_url or invocation_id,
                        predicate="has_title",
                        value=title,
                        value_type=ValueType.STRING,
                        tool_id=tool_id,
                        tool_name=tool_name,
                        invocation_id=invocation_id,
                        data_location_format=DataLocationFormat.CSS_SELECTOR,
                        data_path="title",
                        confidence_score=0.80,
                        confidence_method=ConfidenceMethod.EXTRACTED,
                        original_format=FormatType.HTML,
                    )
                )
            if self.extract_tables:
                for table_idx, table in enumerate(soup.find_all("table")):
                    table_assertions = self._parse_table(
                        table, table_idx, tool_id, tool_name, invocation_id, source_url
                    )
                    assertions.extend(table_assertions)
                    self._nodes_processed += len(table_assertions)
            if self.extract_lists:
                for list_idx, lst in enumerate(soup.find_all(["ul", "ol"])):
                    list_assertions = self._parse_list(
                        lst, list_idx, tool_id, tool_name, invocation_id, source_url
                    )
                    assertions.extend(list_assertions)
                    self._nodes_processed += len(list_assertions)
            for article_idx, article in enumerate(
                soup.find_all(["article", "section"])
            ):
                article_assertions = self._parse_article(
                    article,
                    article_idx,
                    tool_id,
                    tool_name,
                    invocation_id,
                    source_url,
                )
                assertions.extend(article_assertions)
                self._nodes_processed += len(article_assertions)
            elapsed_ms = (time.monotonic() - start_time) * 1000
            return ParseResult(
                success=True,
                assertions=assertions,
                format=FormatType.HTML,
                format_confidence=0.75,
                warnings=warnings,
                errors=errors,
                parsing_duration_ms=elapsed_ms,
                is_partial=False,
            )
        except Exception as exc:
            elapsed_ms = (time.monotonic() - start_time) * 1000
            errors.append(str(exc))
            return ParseResult(
                success=False,
                assertions=assertions,
                format=FormatType.HTML,
                format_confidence=0.75,
                warnings=warnings,
                errors=errors,
                parsing_duration_ms=elapsed_ms,
                is_partial=True,
            )

    def validate(self, input: RawOutput) -> ValidationResult:
        try:
            content = self._decode_content(input.content)
            BeautifulSoup(content, "html.parser")
            return ValidationResult(is_valid=True, errors=[], warnings=[])
        except Exception as exc:
            return ValidationResult(
                is_valid=False,
                errors=[str(exc)],
                warnings=[],
            )

    def _decode_content(self, content: str | bytes) -> str:
        if isinstance(content, bytes):
            return content.decode("utf-8", errors="replace")
        return content

    def _extract_title(self, soup: BeautifulSoup) -> str | None:
        title_tag = soup.find("title")
        if title_tag and title_tag.string:
            return title_tag.string.strip()
        h1_tag = soup.find("h1")
        if h1_tag:
            return h1_tag.get_text(strip=True)
        return None

    def _parse_table(
        self,
        table: Tag,
        table_idx: int,
        tool_id: str,
        tool_name: str,
        invocation_id: str,
        source_url: str,
    ) -> list[StructuredAssertion]:
        assertions: list[StructuredAssertion] = []
        rows = table.find_all("tr")
        if not rows:
            return assertions
        first_row = rows[0]
        headers: list[str] = []
        for cell in first_row.find_all(["th", "td"]):
            headers.append(cell.get_text(strip=True))
        for row_idx, row in enumerate(rows[1:], start=1):
            cells = row.find_all(["td", "th"])
            for col_idx, cell in enumerate(cells):
                cell_text = cell.get_text(strip=True)
                if not cell_text:
                    continue
                header = headers[col_idx] if col_idx < len(headers) else f"column_{col_idx}"
                predicate = re.sub(r"[^a-zA-Z0-9_]", "_", header.lower()).strip("_")
                if not predicate:
                    predicate = f"column_{col_idx}"
                css_selector = f"table:nth-of-type({table_idx + 1}) tr:nth-of-type({row_idx + 1}) td:nth-of-type({col_idx + 1})"
                assertions.append(
                    create_assertion(
                        assertion_type=AssertionType.ENTITY_ATTRIBUTE,
                        subject_type="table_row",
                        subject_id=f"{source_url or invocation_id}#table_{table_idx}_row_{row_idx}",
                        predicate=predicate,
                        value=cell_text,
                        value_type=ValueType.STRING,
                        tool_id=tool_id,
                        tool_name=tool_name,
                        invocation_id=invocation_id,
                        data_location_format=DataLocationFormat.CSS_SELECTOR,
                        data_path=css_selector,
                        confidence_score=0.70,
                        confidence_method=ConfidenceMethod.EXTRACTED,
                        original_format=FormatType.HTML,
                    )
                )
        return assertions

    def _parse_list(
        self,
        lst: Tag,
        list_idx: int,
        tool_id: str,
        tool_name: str,
        invocation_id: str,
        source_url: str,
    ) -> list[StructuredAssertion]:
        assertions: list[StructuredAssertion] = []
        items = lst.find_all("li", recursive=False)
        list_type = "ordered_list" if lst.name == "ol" else "unordered_list"
        for item_idx, item in enumerate(items):
            item_text = item.get_text(strip=True)
            if not item_text:
                continue
            css_selector = f"{lst.name}:nth-of-type({list_idx + 1}) li:nth-of-type({item_idx + 1})"
            assertions.append(
                create_assertion(
                    assertion_type=AssertionType.ENTITY_ATTRIBUTE,
                    subject_type=list_type,
                    subject_id=f"{source_url or invocation_id}#list_{list_idx}_item_{item_idx}",
                    predicate="list_item",
                    value=item_text,
                    value_type=ValueType.STRING,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.CSS_SELECTOR,
                    data_path=css_selector,
                    confidence_score=0.65,
                    confidence_method=ConfidenceMethod.EXTRACTED,
                    original_format=FormatType.HTML,
                )
            )
        return assertions

    def _parse_article(
        self,
        article: Tag,
        article_idx: int,
        tool_id: str,
        tool_name: str,
        invocation_id: str,
        source_url: str,
    ) -> list[StructuredAssertion]:
        assertions: list[StructuredAssertion] = []
        element_id = article.get("id", "")
        css_base = f"{article.name}:nth-of-type({article_idx + 1})"
        for heading in article.find_all(["h1", "h2", "h3", "h4", "h5", "h6"]):
            heading_text = heading.get_text(strip=True)
            if not heading_text:
                continue
            heading_level = heading.name
            css_selector = f"{css_base} {heading_level}"
            entity_id = (
                f"{source_url or invocation_id}#{element_id}"
                if element_id
                else f"{source_url or invocation_id}#article_{article_idx}"
            )
            assertions.append(
                create_assertion(
                    assertion_type=AssertionType.ENTITY_ATTRIBUTE,
                    subject_type="heading",
                    subject_id=entity_id,
                    predicate=f"has_{heading_level}",
                    value=heading_text,
                    value_type=ValueType.STRING,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.CSS_SELECTOR,
                    data_path=css_selector,
                    confidence_score=0.70,
                    confidence_method=ConfidenceMethod.EXTRACTED,
                    original_format=FormatType.HTML,
                )
            )
        for para in article.find_all("p"):
            para_text = para.get_text(strip=True)
            if not para_text:
                continue
            if len(para_text) > self.max_text_length:
                para_text = para_text[: self.max_text_length] + "..."
            css_selector = f"{css_base} p"
            entity_id = (
                f"{source_url or invocation_id}#{element_id}"
                if element_id
                else f"{source_url or invocation_id}#article_{article_idx}"
            )
            assertions.append(
                create_assertion(
                    assertion_type=AssertionType.ENTITY_ATTRIBUTE,
                    subject_type="paragraph",
                    subject_id=entity_id,
                    predicate="contains_text",
                    value=para_text,
                    value_type=ValueType.STRING,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.CSS_SELECTOR,
                    data_path=css_selector,
                    confidence_score=0.65,
                    confidence_method=ConfidenceMethod.EXTRACTED,
                    original_format=FormatType.HTML,
                )
            )
        return assertions
