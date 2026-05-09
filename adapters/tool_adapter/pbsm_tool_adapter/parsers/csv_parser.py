from __future__ import annotations

import csv
import io
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


class CsvParser:
    def __init__(
        self,
        delimiter: str = "auto",
        has_header: bool = True,
        quote_char: str = '"',
        escape_char: str = "\\",
        skip_empty_rows: bool = True,
        max_rows: int = 10000,
    ) -> None:
        self.format = FormatType.CSV
        self.version = "1.0.0"
        self.priority = 15
        self.delimiter = delimiter
        self.has_header = has_header
        self.quote_char = quote_char
        self.escape_char = escape_char
        self.skip_empty_rows = skip_empty_rows
        self.max_rows = max_rows
        self._nodes_processed = 0

    def can_parse(self, input: RawOutput) -> ParseabilityResult:
        content = input.content
        if isinstance(content, bytes):
            try:
                content = content.decode("utf-8")
            except UnicodeDecodeError:
                return ParseabilityResult(
                    can_parse=False,
                    confidence=0.0,
                    estimated_complexity="low",
                )

        lines = content.splitlines()
        non_empty_lines = [line for line in lines if line.strip()]
        if len(non_empty_lines) < 1:
            return ParseabilityResult(
                can_parse=False,
                confidence=0.0,
                estimated_complexity="low",
            )

        candidate_delimiters = [",", "\t", ";"]
        for delim in candidate_delimiters:
            counts = [line.count(delim) for line in non_empty_lines[:5]]
            if counts[0] > 0 and len(set(counts)) == 1:
                return ParseabilityResult(
                    can_parse=True,
                    confidence=0.85,
                    estimated_complexity="medium",
                    detected_features=[f"consistent_{repr(delim)}_delimiter"],
                )

        return ParseabilityResult(
            can_parse=False,
            confidence=0.0,
            estimated_complexity="low",
        )

    def parse(self, input: RawOutput, options: ParseOptions | None = None) -> ParseResult:
        start_time = time.monotonic()
        options = options or ParseOptions()

        content = input.content
        if isinstance(content, bytes):
            content = content.decode("utf-8")

        detected_delimiter = self._detect_delimiter(content)

        reader = csv.reader(
            io.StringIO(content),
            delimiter=detected_delimiter,
            quotechar=self.quote_char,
            escapechar=self.escape_char,
        )

        rows = list(reader)

        if not rows:
            elapsed = (time.monotonic() - start_time) * 1000
            return ParseResult(
                success=True,
                assertions=[],
                format=FormatType.CSV,
                format_confidence=0.85,
                parsing_duration_ms=elapsed,
                is_partial=False,
            )

        if self.has_header:
            headers = [h.strip() for h in rows[0]]
            data_rows = rows[1:]
        else:
            headers = [f"col_{i}" for i in range(len(rows[0]))]
            data_rows = rows

        assertions: list[StructuredAssertion] = []
        rows_processed = 0

        tool_id = (input.metadata or {}).get("tool_id", "csv_parser")
        tool_name = (input.metadata or {}).get("tool_name", "CsvParser")
        invocation_id = (input.metadata or {}).get("invocation_id", "")

        for row_idx, row in enumerate(data_rows):
            if rows_processed >= self.max_rows:
                break

            if self.skip_empty_rows and not any(cell.strip() for cell in row):
                continue

            entity_id = row[0].strip() if row and row[0].strip() else f"row_{row_idx}"

            for col_idx, cell in enumerate(row):
                if col_idx >= len(headers):
                    break

                predicate = headers[col_idx]
                value_type = self._infer_value_type(cell.strip())
                cell_ref = self._col_to_cell_ref(col_idx, row_idx + (2 if self.has_header else 1))

                assertion = create_assertion(
                    assertion_type=AssertionType.ENTITY_ATTRIBUTE,
                    subject_type="csv_row",
                    subject_id=entity_id,
                    predicate=predicate,
                    value=cell.strip(),
                    value_type=value_type,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.CELL_REFERENCE,
                    data_path=cell_ref,
                    confidence_score=0.85,
                    confidence_method=ConfidenceMethod.EXTRACTED,
                    original_format=FormatType.CSV,
                )
                assertions.append(assertion)
                self._nodes_processed += 1

            rows_processed += 1

        elapsed = (time.monotonic() - start_time) * 1000
        is_partial = rows_processed >= self.max_rows and rows_processed < len(data_rows)

        return ParseResult(
            success=True,
            assertions=assertions,
            format=FormatType.CSV,
            format_confidence=0.85,
            parsing_duration_ms=elapsed,
            is_partial=is_partial,
        )

    def validate(self, input: RawOutput) -> ValidationResult:
        content = input.content
        if isinstance(content, bytes):
            try:
                content = content.decode("utf-8")
            except UnicodeDecodeError as e:
                return ValidationResult(
                    is_valid=False,
                    errors=[f"Failed to decode content as UTF-8: {e}"],
                )

        try:
            reader = csv.reader(io.StringIO(content))
            for _ in reader:
                pass
        except csv.Error as e:
            return ValidationResult(
                is_valid=False,
                errors=[f"CSV parsing error: {e}"],
            )

        return ValidationResult(is_valid=True)

    def _detect_delimiter(self, content: str) -> str:
        if self.delimiter != "auto":
            return self.delimiter

        lines = content.splitlines()
        non_empty_lines = [line for line in lines if line.strip()][:5]

        if not non_empty_lines:
            return ","

        candidate_delimiters = [",", "\t", ";"]
        best_delimiter = ","
        best_score = 0

        for delim in candidate_delimiters:
            counts = [line.count(delim) for line in non_empty_lines]
            if not counts or counts[0] == 0:
                continue
            if len(set(counts)) == 1:
                score = counts[0]
                if score > best_score:
                    best_score = score
                    best_delimiter = delim

        return best_delimiter

    def _infer_value_type(self, value: str) -> ValueType:
        if not value:
            return ValueType.STRING

        try:
            int(value)
            return ValueType.NUMBER
        except ValueError:
            pass

        try:
            float(value)
            return ValueType.NUMBER
        except ValueError:
            pass

        lower = value.lower()
        if lower in ("true", "false"):
            return ValueType.BOOLEAN

        return ValueType.STRING

    def _col_to_cell_ref(self, col_idx: int, row_idx: int) -> str:
        result = ""
        n = col_idx
        while True:
            result = chr(ord("A") + n % 26) + result
            n = n // 26 - 1
            if n < 0:
                break
        return f"{result}{row_idx}"
