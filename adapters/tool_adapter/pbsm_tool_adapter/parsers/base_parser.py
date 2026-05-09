from __future__ import annotations

import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Optional, Protocol, runtime_checkable

from ..types import (
    AssertionMetadata,
    AssertionObject,
    AssertionSubject,
    AssertionType,
    ConfidenceFactor,
    ConfidenceInfo,
    ConfidenceMethod,
    DataLocation,
    DataLocationFormat,
    FormatIdentification,
    FormatType,
    ParseOptions,
    ParseResult,
    ParseabilityResult,
    ParseWarning,
    RawOutput,
    SourceInfo,
    StructuredAssertion,
    ValidationResult,
    ValueType,
)


@runtime_checkable
class FormatParser(Protocol):
    format: FormatType
    version: str
    priority: int

    def can_parse(self, input: RawOutput) -> ParseabilityResult: ...

    def parse(self, input: RawOutput, options: ParseOptions | None = None) -> ParseResult: ...

    def validate(self, input: RawOutput) -> ValidationResult: ...


class ParserRegistry:
    def __init__(self) -> None:
        self._parsers: dict[FormatType, list[FormatParser]] = {}
        self._effective_priorities: dict[int, int] = {}

    def register(self, parser: FormatParser, priority: int | None = None) -> None:
        effective_priority = priority if priority is not None else parser.priority
        self._effective_priorities[id(parser)] = effective_priority
        format_type = parser.format
        if format_type not in self._parsers:
            self._parsers[format_type] = []
        self._parsers[format_type].append(parser)
        self._parsers[format_type].sort(key=lambda p: self._effective_priorities[id(p)])

    def unregister(self, format_type: FormatType) -> bool:
        if format_type in self._parsers:
            for p in self._parsers[format_type]:
                self._effective_priorities.pop(id(p), None)
            del self._parsers[format_type]
            return True
        return False

    def get_parser(self, format_type: FormatType) -> FormatParser | None:
        parsers = self._parsers.get(format_type)
        if parsers:
            return parsers[0]
        return None

    def identify_format(self, raw_output: RawOutput) -> FormatIdentification:
        all_parsers: list[FormatParser] = []
        for parser_list in self._parsers.values():
            all_parsers.extend(parser_list)
        all_parsers.sort(key=lambda p: self._effective_priorities.get(id(p), p.priority))

        best_parser: FormatParser | None = None
        best_confidence: float = 0.0
        best_result: ParseabilityResult | None = None

        for parser in all_parsers:
            result = parser.can_parse(raw_output)
            if result.can_parse:
                if best_parser is None or result.confidence > best_confidence:
                    best_parser = parser
                    best_confidence = result.confidence
                    best_result = result

        if best_parser is not None and best_result is not None:
            return FormatIdentification(
                format=best_parser.format,
                confidence=best_confidence,
                detection_method="parser_detection",
                detected_features=best_result.detected_features,
            )

        return FormatIdentification(
            format=FormatType.UNKNOWN,
            confidence=0.0,
            detection_method="no_matching_parser",
        )

    def list_parsers(self) -> dict[FormatType, list[str]]:
        return {
            format_type: [parser.version for parser in parsers]
            for format_type, parsers in self._parsers.items()
        }


def create_assertion(
    assertion_type: AssertionType,
    subject_type: str,
    subject_id: str,
    predicate: str,
    value: Any,
    value_type: ValueType,
    tool_id: str,
    tool_name: str,
    invocation_id: str,
    data_location_format: DataLocationFormat,
    data_path: str,
    confidence_score: float,
    confidence_method: ConfidenceMethod,
    original_format: FormatType,
    is_partial: bool = False,
) -> StructuredAssertion:
    return StructuredAssertion(
        assertion_id=str(uuid.uuid4()),
        assertion_type=assertion_type,
        subject=AssertionSubject(
            entity_type=subject_type,
            entity_id=subject_id,
        ),
        predicate=predicate,
        object=AssertionObject(
            value=value,
            value_type=value_type,
        ),
        confidence=ConfidenceInfo(
            score=confidence_score,
            method=confidence_method,
        ),
        source=SourceInfo(
            tool_id=tool_id,
            tool_name=tool_name,
            invocation_id=invocation_id,
            data_location=DataLocation(
                format=data_location_format,
                path=data_path,
            ),
        ),
        metadata=AssertionMetadata(
            extracted_at=datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
            original_format=original_format,
            is_partial=is_partial,
        ),
    )
