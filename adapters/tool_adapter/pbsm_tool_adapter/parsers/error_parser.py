from __future__ import annotations

import json
import re
import time
from typing import Any, Optional

from ..types import (
    AssertionType,
    ConfidenceMethod,
    DataLocationFormat,
    ErrorSeverity,
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


class ErrorParser:
    ERROR_INDICATORS = [
        "error",
        "exception",
        "failed",
        "failure",
        "timeout",
        "denied",
        "invalid",
        "not found",
        "unauthorized",
        "forbidden",
    ]

    ERROR_CODE_MAPPING: dict[str, str] = {
        "AUTH_": "authentication",
        "PARAM_": "parameter",
        "PERM_": "permission",
        "RES_": "resource",
        "SVC_": "service",
        "NET_": "network",
        "TIMEOUT_": "timeout",
        "PARSE_": "parse",
    }

    def __init__(self, include_stack_trace: bool = False, max_context_size: int = 1000) -> None:
        self.format = FormatType.ERROR
        self.version = "1.0.0"
        self.priority = 5
        self._include_stack_trace = include_stack_trace
        self._max_context_size = max_context_size
        self._nodes_processed = 0

    def can_parse(self, input: RawOutput) -> ParseabilityResult:
        if input.status_code is not None and input.status_code >= 400:
            return ParseabilityResult(
                can_parse=True,
                confidence=0.95,
                estimated_complexity="low",
                detected_features=["http_error_status"],
            )

        content = self._decode_content(input.content)

        try:
            data = json.loads(content)
            if isinstance(data, dict):
                if "error" in data or "errors" in data:
                    return ParseabilityResult(
                        can_parse=True,
                        confidence=0.90,
                        estimated_complexity="low",
                        detected_features=["json_error_field"],
                    )
        except (json.JSONDecodeError, TypeError, ValueError):
            pass

        content_lower = content.lower()
        matched_indicators = [ind for ind in self.ERROR_INDICATORS if ind in content_lower]
        if matched_indicators:
            return ParseabilityResult(
                can_parse=True,
                confidence=0.70,
                estimated_complexity="medium",
                detected_features=[f"text_indicator:{ind}" for ind in matched_indicators],
            )

        return ParseabilityResult(
            can_parse=False,
            confidence=0.0,
            estimated_complexity="unknown",
            detected_features=[],
        )

    def parse(self, input: RawOutput, options: ParseOptions | None = None) -> ParseResult:
        start_time = time.monotonic()
        options = options or ParseOptions()

        metadata = input.metadata or {}
        tool_id = metadata.get("tool_id", "unknown")
        tool_name = metadata.get("tool_name", "unknown")
        invocation_id = metadata.get("invocation_id", "unknown")

        assertions: list[StructuredAssertion] = []
        content = self._decode_content(input.content)

        json_assertions = self._parse_json_error(content, tool_id, tool_name, invocation_id)
        if json_assertions:
            assertions.extend(json_assertions)
        else:
            http_assertions = self._parse_http_error(input, tool_id, tool_name, invocation_id)
            if http_assertions:
                assertions.extend(http_assertions)
            else:
                text_assertions = self._parse_text_error(content, tool_id, tool_name, invocation_id)
                assertions.extend(text_assertions)

        self._nodes_processed += len(assertions)

        elapsed_ms = (time.monotonic() - start_time) * 1000

        return ParseResult(
            success=True,
            assertions=assertions,
            format=FormatType.ERROR,
            format_confidence=0.95,
            parsing_duration_ms=elapsed_ms,
            is_partial=False,
        )

    def validate(self, input: RawOutput) -> ValidationResult:
        errors: list[str] = []
        warnings: list[str] = []

        content = self._decode_content(input.content)
        content_lower = content.lower()

        has_indicator = any(indicator in content_lower for indicator in self.ERROR_INDICATORS)
        has_status = input.status_code is not None and input.status_code >= 400

        if not has_indicator and not has_status:
            errors.append("No error indicators found in content or status code")

        if not content.strip():
            errors.append("Empty content")

        return ValidationResult(
            is_valid=len(errors) == 0,
            errors=errors,
            warnings=warnings,
        )

    def _parse_json_error(
        self,
        content: str,
        tool_id: str,
        tool_name: str,
        invocation_id: str,
    ) -> list[StructuredAssertion]:
        assertions: list[StructuredAssertion] = []

        try:
            data = json.loads(content)
        except (json.JSONDecodeError, TypeError, ValueError):
            return assertions

        if not isinstance(data, dict):
            return assertions

        error_data = data.get("error") or data.get("errors")
        if error_data is None:
            return assertions

        if isinstance(error_data, list):
            for idx, err in enumerate(error_data):
                if isinstance(err, dict):
                    assertions.extend(
                        self._extract_json_error_fields(err, tool_id, tool_name, invocation_id, index=idx)
                    )
            return assertions

        if isinstance(error_data, dict):
            assertions.extend(
                self._extract_json_error_fields(error_data, tool_id, tool_name, invocation_id)
            )
            return assertions

        assertions.append(
            create_assertion(
                assertion_type=AssertionType.ERROR,
                subject_type="error",
                subject_id=f"{tool_id}:error",
                predicate="hasErrorMessage",
                value=str(error_data),
                value_type=ValueType.STRING,
                tool_id=tool_id,
                tool_name=tool_name,
                invocation_id=invocation_id,
                data_location_format=DataLocationFormat.JSON_PATH,
                data_path="$.error",
                confidence_score=0.95,
                confidence_method=ConfidenceMethod.EXACT,
                original_format=FormatType.ERROR,
            )
        )

        return assertions

    def _extract_json_error_fields(
        self,
        error_data: dict[str, Any],
        tool_id: str,
        tool_name: str,
        invocation_id: str,
        index: int | None = None,
    ) -> list[StructuredAssertion]:
        assertions: list[StructuredAssertion] = []
        index_suffix = f"[{index}]" if index is not None else ""
        path_prefix = f"$.error{index_suffix}"

        error_code = error_data.get("code", "")
        error_message = error_data.get("message", "")
        error_details = error_data.get("details")
        request_id = error_data.get("requestId", "")

        category = self._classify_error(str(error_code)) if error_code else "unknown"
        subject_id = f"{tool_id}:error{index_suffix}"

        if error_code:
            assertions.append(
                create_assertion(
                    assertion_type=AssertionType.ERROR,
                    subject_type="error",
                    subject_id=subject_id,
                    predicate="hasErrorCode",
                    value=str(error_code),
                    value_type=ValueType.STRING,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.JSON_PATH,
                    data_path=f"{path_prefix}.code",
                    confidence_score=0.95,
                    confidence_method=ConfidenceMethod.EXACT,
                    original_format=FormatType.ERROR,
                )
            )

        if error_message:
            assertions.append(
                create_assertion(
                    assertion_type=AssertionType.ERROR,
                    subject_type="error",
                    subject_id=subject_id,
                    predicate="hasErrorMessage",
                    value=str(error_message),
                    value_type=ValueType.STRING,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.JSON_PATH,
                    data_path=f"{path_prefix}.message",
                    confidence_score=0.95,
                    confidence_method=ConfidenceMethod.EXACT,
                    original_format=FormatType.ERROR,
                )
            )

        if error_details is not None:
            detail_str = json.dumps(error_details) if not isinstance(error_details, str) else error_details
            if len(detail_str) > self._max_context_size:
                detail_str = detail_str[: self._max_context_size]
            assertions.append(
                create_assertion(
                    assertion_type=AssertionType.ERROR_CONTEXT,
                    subject_type="error",
                    subject_id=subject_id,
                    predicate="hasErrorDetails",
                    value=detail_str,
                    value_type=ValueType.STRING,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.JSON_PATH,
                    data_path=f"{path_prefix}.details",
                    confidence_score=0.95,
                    confidence_method=ConfidenceMethod.EXACT,
                    original_format=FormatType.ERROR,
                )
            )

        if request_id:
            assertions.append(
                create_assertion(
                    assertion_type=AssertionType.ERROR_CONTEXT,
                    subject_type="error",
                    subject_id=subject_id,
                    predicate="hasRequestId",
                    value=str(request_id),
                    value_type=ValueType.STRING,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.JSON_PATH,
                    data_path=f"{path_prefix}.requestId",
                    confidence_score=0.95,
                    confidence_method=ConfidenceMethod.EXACT,
                    original_format=FormatType.ERROR,
                )
            )

        return assertions

    def _parse_http_error(
        self,
        input: RawOutput,
        tool_id: str,
        tool_name: str,
        invocation_id: str,
    ) -> list[StructuredAssertion]:
        assertions: list[StructuredAssertion] = []

        if input.status_code is None or input.status_code < 400:
            return assertions

        status_code = input.status_code

        if 400 <= status_code < 500:
            error_category = "client_error"
            code_prefix = "PARAM_"
        else:
            error_category = "server_error"
            code_prefix = "SVC_"

        error_code = f"{code_prefix}{status_code}"
        subject_id = f"{tool_id}:error"

        assertions.append(
            create_assertion(
                assertion_type=AssertionType.ERROR,
                subject_type="error",
                subject_id=subject_id,
                predicate="hasErrorCode",
                value=error_code,
                value_type=ValueType.STRING,
                tool_id=tool_id,
                tool_name=tool_name,
                invocation_id=invocation_id,
                data_location_format=DataLocationFormat.JSON_PATH,
                data_path="$.statusCode",
                confidence_score=0.95,
                confidence_method=ConfidenceMethod.EXACT,
                original_format=FormatType.ERROR,
            )
        )

        assertions.append(
            create_assertion(
                assertion_type=AssertionType.ERROR,
                subject_type="error",
                subject_id=subject_id,
                predicate="hasErrorMessage",
                value=f"HTTP {status_code} {error_category}",
                value_type=ValueType.STRING,
                tool_id=tool_id,
                tool_name=tool_name,
                invocation_id=invocation_id,
                data_location_format=DataLocationFormat.JSON_PATH,
                data_path="$.statusCode",
                confidence_score=0.95,
                confidence_method=ConfidenceMethod.EXACT,
                original_format=FormatType.ERROR,
            )
        )

        content = self._decode_content(input.content)
        if content.strip():
            context_value = content[: self._max_context_size]
            assertions.append(
                create_assertion(
                    assertion_type=AssertionType.ERROR_CONTEXT,
                    subject_type="error",
                    subject_id=subject_id,
                    predicate="hasResponseBody",
                    value=context_value,
                    value_type=ValueType.STRING,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.JSON_PATH,
                    data_path="$.body",
                    confidence_score=0.90,
                    confidence_method=ConfidenceMethod.EXTRACTED,
                    original_format=FormatType.ERROR,
                )
            )

        return assertions

    def _parse_text_error(
        self,
        content: str,
        tool_id: str,
        tool_name: str,
        invocation_id: str,
    ) -> list[StructuredAssertion]:
        assertions: list[StructuredAssertion] = []

        content_lower = content.lower()
        matched_indicators = [ind for ind in self.ERROR_INDICATORS if ind in content_lower]

        if not matched_indicators:
            return assertions

        subject_id = f"{tool_id}:error"

        error_pattern = re.compile(
            r"(?i)(?:error|exception|fail|timeout|denied|invalid|not found|unauthorized|forbidden)"
            r"[\s:]*([^\n]{1,200})",
        )
        matches = error_pattern.findall(content)

        if matches:
            primary_message = matches[0].strip()
            assertions.append(
                create_assertion(
                    assertion_type=AssertionType.ERROR,
                    subject_type="error",
                    subject_id=subject_id,
                    predicate="hasErrorMessage",
                    value=primary_message,
                    value_type=ValueType.STRING,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.REGEX_MATCH,
                    data_path="error_message",
                    confidence_score=0.70,
                    confidence_method=ConfidenceMethod.EXTRACTED,
                    original_format=FormatType.ERROR,
                )
            )

            if len(matches) > 1:
                additional = matches[1:]
                context_str = "; ".join(m.strip() for m in additional[:5])
                if len(context_str) > self._max_context_size:
                    context_str = context_str[: self._max_context_size]
                assertions.append(
                    create_assertion(
                        assertion_type=AssertionType.ERROR_CONTEXT,
                        subject_type="error",
                        subject_id=subject_id,
                        predicate="hasAdditionalErrors",
                        value=context_str,
                        value_type=ValueType.STRING,
                        tool_id=tool_id,
                        tool_name=tool_name,
                        invocation_id=invocation_id,
                        data_location_format=DataLocationFormat.REGEX_MATCH,
                        data_path="additional_errors",
                        confidence_score=0.70,
                        confidence_method=ConfidenceMethod.EXTRACTED,
                        original_format=FormatType.ERROR,
                    )
                )
        else:
            assertions.append(
                create_assertion(
                    assertion_type=AssertionType.ERROR,
                    subject_type="error",
                    subject_id=subject_id,
                    predicate="hasErrorIndicator",
                    value=", ".join(matched_indicators),
                    value_type=ValueType.STRING,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.REGEX_MATCH,
                    data_path="error_indicators",
                    confidence_score=0.70,
                    confidence_method=ConfidenceMethod.INFERRED,
                    original_format=FormatType.ERROR,
                )
            )

        code_pattern = re.compile(r"\b([A-Z][A-Z0-9_]*_)\b")
        code_matches = code_pattern.findall(content)
        if code_matches:
            error_code = code_matches[0].rstrip("_")
            category = self._classify_error(error_code)
            assertions.append(
                create_assertion(
                    assertion_type=AssertionType.ERROR,
                    subject_type="error",
                    subject_id=subject_id,
                    predicate="hasErrorCode",
                    value=error_code,
                    value_type=ValueType.STRING,
                    tool_id=tool_id,
                    tool_name=tool_name,
                    invocation_id=invocation_id,
                    data_location_format=DataLocationFormat.REGEX_MATCH,
                    data_path="error_code",
                    confidence_score=0.70,
                    confidence_method=ConfidenceMethod.EXTRACTED,
                    original_format=FormatType.ERROR,
                )
            )

        return assertions

    def _classify_error(self, code: str) -> str:
        code_upper = code.upper()
        for prefix, category in self.ERROR_CODE_MAPPING.items():
            if code_upper.startswith(prefix):
                return category
        return "unknown"

    def _determine_severity(self, status_code: int | None, error_code: str) -> ErrorSeverity:
        if status_code is not None:
            if status_code >= 500:
                return ErrorSeverity.CRITICAL
            if status_code >= 400:
                return ErrorSeverity.ERROR

        category = self._classify_error(error_code)
        if category in ("authentication", "permission"):
            return ErrorSeverity.ERROR
        if category in ("network", "timeout", "service"):
            return ErrorSeverity.CRITICAL

        return ErrorSeverity.WARNING

    def _decode_content(self, content: str | bytes) -> str:
        if isinstance(content, bytes):
            try:
                return content.decode("utf-8")
            except UnicodeDecodeError:
                return content.decode("utf-8", errors="replace")
        return content
