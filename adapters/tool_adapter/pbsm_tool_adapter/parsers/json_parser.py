from __future__ import annotations

import json
import time
import uuid
from datetime import datetime
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
    ParseWarning,
)
from .base_parser import FormatParser, create_assertion


class JsonParser(FormatParser):

    def __init__(
        self,
        max_depth: int = 10,
        max_nodes: int = 1000,
        type_field: str = "type",
        id_fields: list[str] | None = None,
        ignore_fields: list[str] | None = None,
    ):
        self.max_depth = max_depth
        self.max_nodes = max_nodes
        self.type_field = type_field
        self.id_fields = id_fields or ["id", "identifier", "uuid", "name"]
        self.ignore_fields = ignore_fields or ["metadata", "_meta"]
        self.format = FormatType.JSON
        self.version = "1.0.0"
        self.priority = 10
        self._nodes_processed = 0
        self._depth_exceeded = False

    def can_parse(self, input: RawOutput) -> ParseabilityResult:
        content = input.content
        if isinstance(content, bytes):
            content = content.decode("utf-8", errors="replace")
        content = content.strip()
        if content.startswith("{") or content.startswith("["):
            try:
                json.loads(content)
                return ParseabilityResult(
                    can_parse=True,
                    confidence=0.95,
                    estimated_complexity="LOW",
                    detected_features=["structured_json"],
                )
            except (json.JSONDecodeError, ValueError):
                pass
        return ParseabilityResult(
            can_parse=False,
            confidence=0.0,
            estimated_complexity="UNKNOWN",
            detected_features=[],
        )

    def parse(self, input: RawOutput, options: ParseOptions | None = None) -> ParseResult:
        start_time = time.monotonic()
        self._nodes_processed = 0
        self._depth_exceeded = False
        content = input.content
        if isinstance(content, bytes):
            content = content.decode("utf-8", errors="replace")
        content = content.strip()
        try:
            data = json.loads(content)
        except (json.JSONDecodeError, ValueError) as e:
            return ParseResult(
                success=False,
                assertions=[],
                format=FormatType.JSON,
                format_confidence=0.95,
                errors=[f"JSON parse failed: {e}"],
            )
        metadata = input.metadata or {}
        tool_id = metadata.get("tool_id", "unknown")
        tool_name = metadata.get("tool_name", "unknown")
        invocation_id = metadata.get("invocation_id", str(uuid.uuid4()))
        assertions = self._extract_assertions(
            data=data,
            path="$",
            depth=0,
            tool_id=tool_id,
            tool_name=tool_name,
            invocation_id=invocation_id,
        )
        duration = (time.monotonic() - start_time) * 1000
        is_partial = self._nodes_processed >= self.max_nodes or self._depth_exceeded
        return ParseResult(
            success=True,
            assertions=assertions,
            format=FormatType.JSON,
            format_confidence=0.95,
            parsing_duration_ms=duration,
            is_partial=is_partial,
        )

    def validate(self, input: RawOutput) -> ValidationResult:
        content = input.content
        if isinstance(content, bytes):
            content = content.decode("utf-8", errors="replace")
        content = content.strip()
        try:
            json.loads(content)
            return ValidationResult(is_valid=True)
        except (json.JSONDecodeError, ValueError) as e:
            return ValidationResult(
                is_valid=False,
                errors=[f"Invalid JSON: {e}"],
            )

    def _extract_assertions(
        self,
        data: Any,
        path: str,
        depth: int,
        tool_id: str,
        tool_name: str,
        invocation_id: str,
        subject_entity_id: str | None = None,
    ) -> list[StructuredAssertion]:
        if self._nodes_processed >= self.max_nodes:
            return []
        if depth > self.max_depth:
            self._depth_exceeded = True
            return []
        assertions: list[StructuredAssertion] = []
        if isinstance(data, dict):
            self._nodes_processed += 1
            entity_type = data.get(self.type_field, "UnknownEntity")
            entity_id = self._extract_entity_id(data) or str(uuid.uuid4())
            for key, value in data.items():
                if key in self.ignore_fields:
                    continue
                child_path = f"{path}.{key}"
                if isinstance(value, dict):
                    assertions.extend(
                        self._extract_assertions(
                            data=value,
                            path=child_path,
                            depth=depth + 1,
                            tool_id=tool_id,
                            tool_name=tool_name,
                            invocation_id=invocation_id,
                            subject_entity_id=entity_id,
                        )
                    )
                elif isinstance(value, list):
                    for i, item in enumerate(value):
                        item_path = f"{child_path}[{i}]"
                        if isinstance(item, dict):
                            assertions.extend(
                                self._extract_assertions(
                                    data=item,
                                    path=item_path,
                                    depth=depth + 1,
                                    tool_id=tool_id,
                                    tool_name=tool_name,
                                    invocation_id=invocation_id,
                                    subject_entity_id=entity_id,
                                )
                            )
                else:
                    assertion = create_assertion(
                        assertion_type=AssertionType.ENTITY_ATTRIBUTE,
                        subject_type=entity_type,
                        subject_id=entity_id,
                        predicate=key,
                        value=value,
                        value_type=self._infer_value_type(value),
                        tool_id=tool_id,
                        tool_name=tool_name,
                        invocation_id=invocation_id,
                        data_location_format=DataLocationFormat.JSON_PATH,
                        data_path=child_path,
                        confidence_score=0.85,
                        confidence_method=ConfidenceMethod.EXTRACTED,
                        original_format=self.format,
                    )
                    assertions.append(assertion)
        elif isinstance(data, list):
            for i, item in enumerate(data):
                item_path = f"{path}[{i}]"
                if isinstance(item, dict):
                    assertions.extend(
                        self._extract_assertions(
                            data=item,
                            path=item_path,
                            depth=depth + 1,
                            tool_id=tool_id,
                            tool_name=tool_name,
                            invocation_id=invocation_id,
                            subject_entity_id=subject_entity_id,
                        )
                    )
        return assertions

    def _extract_entity_id(self, data: dict) -> str | None:
        for field in self.id_fields:
            if field in data:
                value = data[field]
                if isinstance(value, (str, int, float)):
                    return str(value)
        return None

    def _extract_value(self, data: dict, key: str) -> Any:
        if key in data:
            value = data[key]
            if not isinstance(value, (dict, list)):
                return value
        return None

    def _infer_value_type(self, value: Any) -> ValueType:
        if value is None:
            return ValueType.NULL
        if isinstance(value, bool):
            return ValueType.BOOLEAN
        if isinstance(value, int):
            return ValueType.NUMBER
        if isinstance(value, float):
            return ValueType.NUMBER
        if isinstance(value, str):
            return ValueType.STRING
        if isinstance(value, list):
            return ValueType.ARRAY
        if isinstance(value, dict):
            return ValueType.OBJECT
        return ValueType.STRING
