from __future__ import annotations

import re
import time
from typing import Any, Optional

from ..types import (
    FormatType,
    RawOutput,
    ParseOptions,
    ParseResult,
    ParseabilityResult,
    ValidationResult,
    StructuredAssertion,
    AssertionType,
    ValueType,
    ConfidenceMethod,
    DataLocationFormat,
)
from .base_parser import FormatParser, create_assertion


class TextParser(FormatParser):

    KEY_VALUE_PATTERNS = [
        re.compile(
            r'^([A-Za-z\u4e00-\u9fa5_][A-Za-z0-9\u4e00-\u9fa5_\s]*)\s*[:：]\s*(.+)$',
            re.MULTILINE,
        ),
        re.compile(
            r'^([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.+)$',
            re.MULTILINE,
        ),
    ]

    NUMERIC_PATTERNS = [
        re.compile(r'-?\d+\.?\d*%?'),
        re.compile(r'¥?\d+\.?\d*元?'),
        re.compile(r'\$?\d+\.?\d*'),
    ]

    DATE_PATTERNS = [
        re.compile(
            r'\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?)?'
        ),
        re.compile(r'\d{4}/\d{2}/\d{2}'),
        re.compile(r'\d{2}/\d{2}/\d{4}'),
    ]

    STATUS_WORDS = {
        '成功': 'SUCCESS',
        '失败': 'FAILED',
        '完成': 'COMPLETED',
        '待处理': 'PENDING',
        '进行中': 'IN_PROGRESS',
        '取消': 'CANCELLED',
        'active': 'ACTIVE',
        'inactive': 'INACTIVE',
        'enabled': 'ENABLED',
        'disabled': 'DISABLED',
        'success': 'SUCCESS',
        'failure': 'FAILED',
    }

    def __init__(
        self,
        key_value_patterns=None,
        numeric_patterns=None,
        date_patterns=None,
        confidence_base: float = 0.60,
        max_lines: int = 5000,
    ):
        self.format = FormatType.TEXT
        self.version = "1.0.0"
        self.priority = 30
        self._nodes_processed = 0
        self._key_value_patterns = key_value_patterns or self.KEY_VALUE_PATTERNS
        self._numeric_patterns = numeric_patterns or self.NUMERIC_PATTERNS
        self._date_patterns = date_patterns or self.DATE_PATTERNS
        self._confidence_base = confidence_base
        self._max_lines = max_lines

    def can_parse(self, input: RawOutput) -> ParseabilityResult:
        if isinstance(input.content, bytes):
            try:
                text = input.content.decode('utf-8')
            except UnicodeDecodeError:
                return ParseabilityResult(
                    can_parse=False,
                    confidence=0.0,
                    estimated_complexity="LOW",
                )
        else:
            text = input.content

        text = text.strip()

        if text.startswith('{') or text.startswith('<'):
            return ParseabilityResult(
                can_parse=False,
                confidence=0.0,
                estimated_complexity="LOW",
            )

        if len(text) > 10 and '\n' in text:
            return ParseabilityResult(
                can_parse=True,
                confidence=0.70,
                estimated_complexity="MEDIUM",
                detected_features=["multiline_text"],
            )

        return ParseabilityResult(
            can_parse=True,
            confidence=0.50,
            estimated_complexity="LOW",
            detected_features=["single_line_text"],
        )

    def parse(
        self, input: RawOutput, options: ParseOptions | None = None
    ) -> ParseResult:
        start = time.monotonic()

        if isinstance(input.content, bytes):
            text = input.content.decode('utf-8', errors='replace')
        else:
            text = input.content

        lines = text.split('\n')
        if len(lines) > self._max_lines:
            lines = lines[: self._max_lines]
        text = '\n'.join(lines)

        tool_id = ""
        tool_name = ""
        invocation_id = ""
        if input.metadata:
            tool_id = input.metadata.get("tool_id", "")
            tool_name = input.metadata.get("tool_name", "")
            invocation_id = input.metadata.get("invocation_id", "")

        assertions: list[StructuredAssertion] = []
        assertions.extend(self._extract_key_values(text, tool_id, tool_name, invocation_id))
        assertions.extend(self._extract_numerics(text, tool_id, tool_name, invocation_id))
        assertions.extend(self._extract_dates(text, tool_id, tool_name, invocation_id))
        assertions.extend(self._extract_status(text, tool_id, tool_name, invocation_id))

        self._nodes_processed += len(assertions)

        elapsed_ms = (time.monotonic() - start) * 1000

        return ParseResult(
            success=True,
            assertions=assertions,
            format=FormatType.TEXT,
            format_confidence=self._confidence_base,
            parsing_duration_ms=elapsed_ms,
        )

    def validate(self, input: RawOutput) -> ValidationResult:
        if isinstance(input.content, bytes):
            try:
                input.content.decode('utf-8')
            except UnicodeDecodeError:
                return ValidationResult(
                    is_valid=False,
                    errors=["Content is not valid UTF-8 text"],
                )

        return ValidationResult(is_valid=True)

    def _extract_key_values(
        self,
        text: str,
        tool_id: str,
        tool_name: str,
        invocation_id: str,
    ) -> list[StructuredAssertion]:
        assertions: list[StructuredAssertion] = []
        confidence = min(self._confidence_base + 0.15, 1.0)

        for pattern in self._key_value_patterns:
            for match in pattern.finditer(text):
                key = match.group(1).strip()
                value = match.group(2).strip()
                assertion = create_assertion(
                    assertion_type=AssertionType.ENTITY_ATTRIBUTE,
                    subject_type="text_field",
                    subject_id=self._hash_key(key),
                    predicate=key,
                    value=value,
                    value_type=ValueType.STRING,
                    confidence_score=confidence,
                    confidence_method=ConfidenceMethod.EXTRACTED,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.LINE_NUMBER,
                    data_path=str(match.start()),
                    original_format=self.format,
                )
                assertions.append(assertion)

        return assertions

    def _extract_numerics(
        self,
        text: str,
        tool_id: str,
        tool_name: str,
        invocation_id: str,
    ) -> list[StructuredAssertion]:
        assertions: list[StructuredAssertion] = []
        confidence = min(self._confidence_base + 0.05, 1.0)

        for pattern in self._numeric_patterns:
            for match in pattern.finditer(text):
                value = match.group(0)
                if len(value) < 2:
                    continue
                assertion = create_assertion(
                    assertion_type=AssertionType.ENTITY_ATTRIBUTE,
                    subject_type="numeric_value",
                    subject_id=self._hash_key(value),
                    predicate="numeric",
                    value=value,
                    value_type=ValueType.NUMBER,
                    confidence_score=confidence,
                    confidence_method=ConfidenceMethod.EXTRACTED,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.REGEX_MATCH,
                    data_path=str(match.start()),
                    original_format=self.format,
                )
                assertions.append(assertion)

        return assertions

    def _extract_dates(
        self,
        text: str,
        tool_id: str,
        tool_name: str,
        invocation_id: str,
    ) -> list[StructuredAssertion]:
        assertions: list[StructuredAssertion] = []
        confidence = min(self._confidence_base + 0.10, 1.0)

        for pattern in self._date_patterns:
            for match in pattern.finditer(text):
                value = match.group(0)
                assertion = create_assertion(
                    assertion_type=AssertionType.ENTITY_ATTRIBUTE,
                    subject_type="date_value",
                    subject_id=self._hash_key(value),
                    predicate="date",
                    value=value,
                    value_type=ValueType.DATE,
                    confidence_score=confidence,
                    confidence_method=ConfidenceMethod.EXTRACTED,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.REGEX_MATCH,
                    data_path=str(match.start()),
                    original_format=self.format,
                )
                assertions.append(assertion)

        return assertions

    def _extract_status(
        self,
        text: str,
        tool_id: str,
        tool_name: str,
        invocation_id: str,
    ) -> list[StructuredAssertion]:
        assertions: list[StructuredAssertion] = []
        confidence = min(self._confidence_base + 0.15, 1.0)

        for word, status in self.STATUS_WORDS.items():
            if word in text:
                assertion = create_assertion(
                    assertion_type=AssertionType.ENTITY_ATTRIBUTE,
                    subject_type="status",
                    subject_id=self._hash_key(word),
                    predicate="status",
                    value=status,
                    value_type=ValueType.STRING,
                    confidence_score=confidence,
                    confidence_method=ConfidenceMethod.INFERRED,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.REGEX_MATCH,
                    data_path=word,
                    original_format=self.format,
                )
                assertions.append(assertion)

        return assertions

    def _hash_key(self, key: str) -> str:
        return str(abs(hash(key)) % 10000)
