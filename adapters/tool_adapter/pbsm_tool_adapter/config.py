from __future__ import annotations

import copy
from dataclasses import asdict, dataclass, field
from typing import Any, Optional

from .types import FormatType


@dataclass
class RetryPolicyConfig:
    enabled: bool = True
    max_retries: int = 3
    initial_delay_ms: int = 1000
    max_delay_ms: int = 30000
    retryable_error_prefixes: list[str] = field(
        default_factory=lambda: ["NET_", "TIMEOUT_", "SVC_"]
    )


@dataclass
class CircuitBreakerConfig:
    enabled: bool = True
    failure_threshold: int = 5
    success_threshold: int = 2
    recovery_timeout_ms: int = 60000


@dataclass
class ErrorHandlingConfig:
    retry_policy: RetryPolicyConfig = field(default_factory=RetryPolicyConfig)
    circuit_breaker: CircuitBreakerConfig = field(default_factory=CircuitBreakerConfig)


@dataclass
class JsonParserConfig:
    max_depth: int = 10
    max_nodes: int = 1000
    type_field: str = "type"
    id_fields: list[str] = field(
        default_factory=lambda: ["id", "identifier", "uuid", "name"]
    )
    ignore_fields: list[str] = field(
        default_factory=lambda: ["metadata", "_meta"]
    )


@dataclass
class HtmlParserConfig:
    extract_tables: bool = True
    extract_lists: bool = True
    max_text_length: int = 10000
    clean_scripts: bool = True


@dataclass
class TextParserConfig:
    confidence_base: float = 0.60
    max_lines: int = 5000


@dataclass
class CsvParserConfig:
    delimiter: str = "auto"
    has_header: bool = True
    quote_char: str = '"'
    max_rows: int = 10000


@dataclass
class ErrorParserConfig:
    include_stack_trace: bool = False
    max_context_size: int = 1000


@dataclass
class FormatConfigs:
    json: JsonParserConfig = field(default_factory=JsonParserConfig)
    html: HtmlParserConfig = field(default_factory=HtmlParserConfig)
    text: TextParserConfig = field(default_factory=TextParserConfig)
    csv: CsvParserConfig = field(default_factory=CsvParserConfig)
    error: ErrorParserConfig = field(default_factory=ErrorParserConfig)


@dataclass
class GlobalParserConfig:
    max_parsing_time_ms: int = 5000
    default_confidence: float = 0.75
    enable_partial_parsing: bool = True
    enable_deduplication: bool = True


@dataclass
class ToolAdapterConfig:
    parser: GlobalParserConfig = field(default_factory=GlobalParserConfig)
    formats: FormatConfigs = field(default_factory=FormatConfigs)
    error_handling: ErrorHandlingConfig = field(default_factory=ErrorHandlingConfig)


def _deep_merge(base: dict[str, Any], override: dict[str, Any]) -> dict[str, Any]:
    result = copy.deepcopy(base)
    for key, value in override.items():
        if key in result and isinstance(result[key], dict) and isinstance(value, dict):
            result[key] = _deep_merge(result[key], value)
        else:
            result[key] = copy.deepcopy(value)
    return result


class ConfigManager:
    def __init__(self) -> None:
        self._global = ToolAdapterConfig()
        self._tool_overrides: dict[str, dict[str, Any]] = {}

    def get_config(self, category: str | None = None) -> dict[str, Any]:
        if category is None:
            return asdict(self._global)
        if category in ("parser", "formats", "error_handling"):
            return asdict(self._global)[category]
        if category.startswith("tool:"):
            tool_id = category[5:]
            global_config = asdict(self._global)
            tool_overrides = self._tool_overrides.get(tool_id, {})
            return _deep_merge(global_config, tool_overrides)
        return {}

    def update_config(
        self, category: str, settings: dict[str, Any], merge: bool = True
    ) -> dict[str, Any]:
        current = self.get_config(category)
        if category in ("parser", "formats", "error_handling"):
            current_full = asdict(self._global)
            if merge:
                updated = _deep_merge(current, settings)
            else:
                updated = copy.deepcopy(settings)
            current_full[category] = updated
            self._global = self._rebuild_config(current_full)
        elif category.startswith("tool:"):
            tool_id = category[5:]
            if merge:
                existing = self._tool_overrides.get(tool_id, {})
                self._tool_overrides[tool_id] = _deep_merge(existing, settings)
            else:
                self._tool_overrides[tool_id] = copy.deepcopy(settings)
        return current

    def reset_config(self, category: str | None = None) -> None:
        if category is None:
            self._global = ToolAdapterConfig()
            self._tool_overrides.clear()
        elif category in ("parser", "formats", "error_handling"):
            defaults = ToolAdapterConfig()
            default_dict = asdict(defaults)
            current_full = asdict(self._global)
            current_full[category] = default_dict[category]
            self._global = self._rebuild_config(current_full)
        elif category.startswith("tool:"):
            tool_id = category[5:]
            self._tool_overrides.pop(tool_id, None)

    def validate_config(self, config: dict[str, Any]) -> list[str]:
        errors: list[str] = []

        parser = config.get("parser", {})
        if "default_confidence" in parser:
            val = parser["default_confidence"]
            if not (0.0 <= val <= 1.0):
                errors.append(f"parser.default_confidence {val} not in [0, 1]")
        if "max_parsing_time_ms" in parser:
            val = parser["max_parsing_time_ms"]
            if val <= 0:
                errors.append(f"parser.max_parsing_time_ms {val} must be > 0")

        formats = config.get("formats", {})
        json_fmt = formats.get("json", {})
        if "max_depth" in json_fmt:
            val = json_fmt["max_depth"]
            if val <= 0:
                errors.append(f"formats.json.max_depth {val} must be > 0")
        if "max_nodes" in json_fmt:
            val = json_fmt["max_nodes"]
            if val <= 0:
                errors.append(f"formats.json.max_nodes {val} must be > 0")

        text_fmt = formats.get("text", {})
        if "confidence_base" in text_fmt:
            val = text_fmt["confidence_base"]
            if not (0.0 <= val <= 1.0):
                errors.append(f"formats.text.confidence_base {val} not in [0, 1]")
        if "max_lines" in text_fmt:
            val = text_fmt["max_lines"]
            if val <= 0:
                errors.append(f"formats.text.max_lines {val} must be > 0")

        csv_fmt = formats.get("csv", {})
        if "max_rows" in csv_fmt:
            val = csv_fmt["max_rows"]
            if val <= 0:
                errors.append(f"formats.csv.max_rows {val} must be > 0")

        html_fmt = formats.get("html", {})
        if "max_text_length" in html_fmt:
            val = html_fmt["max_text_length"]
            if val <= 0:
                errors.append(f"formats.html.max_text_length {val} must be > 0")

        error_fmt = formats.get("error", {})
        if "max_context_size" in error_fmt:
            val = error_fmt["max_context_size"]
            if val <= 0:
                errors.append(f"formats.error.max_context_size {val} must be > 0")

        error_handling = config.get("error_handling", {})
        retry = error_handling.get("retry_policy", {})
        if "max_retries" in retry:
            val = retry["max_retries"]
            if val < 0:
                errors.append(f"error_handling.retry_policy.max_retries {val} must be >= 0")
        if "initial_delay_ms" in retry:
            val = retry["initial_delay_ms"]
            if val <= 0:
                errors.append(f"error_handling.retry_policy.initial_delay_ms {val} must be > 0")
        if "max_delay_ms" in retry:
            val = retry["max_delay_ms"]
            if val <= 0:
                errors.append(f"error_handling.retry_policy.max_delay_ms {val} must be > 0")

        cb = error_handling.get("circuit_breaker", {})
        if "failure_threshold" in cb:
            val = cb["failure_threshold"]
            if val <= 0:
                errors.append(f"error_handling.circuit_breaker.failure_threshold {val} must be > 0")
        if "success_threshold" in cb:
            val = cb["success_threshold"]
            if val <= 0:
                errors.append(f"error_handling.circuit_breaker.success_threshold {val} must be > 0")
        if "recovery_timeout_ms" in cb:
            val = cb["recovery_timeout_ms"]
            if val <= 0:
                errors.append(f"error_handling.circuit_breaker.recovery_timeout_ms {val} must be > 0")

        return errors

    def get_effective_config(
        self,
        tool_id: str | None = None,
        format_type: FormatType | None = None,
    ) -> dict[str, Any]:
        result = asdict(self._global)
        if format_type is not None:
            format_key = format_type.value.lower()
            format_config = result.get("formats", {}).get(format_key, {})
            result = _deep_merge(result, {"formats": {format_key: format_config}})
            result["_format_specific"] = copy.deepcopy(format_config)
        if tool_id is not None:
            tool_overrides = self._tool_overrides.get(tool_id, {})
            if tool_overrides:
                result = _deep_merge(result, tool_overrides)
        return result

    @staticmethod
    def _rebuild_config(data: dict[str, Any]) -> ToolAdapterConfig:
        parser_data = data.get("parser", {})
        formats_data = data.get("formats", {})
        error_handling_data = data.get("error_handling", {})

        retry_data = error_handling_data.get("retry_policy", {})
        cb_data = error_handling_data.get("circuit_breaker", {})

        json_data = formats_data.get("json", {})
        html_data = formats_data.get("html", {})
        text_data = formats_data.get("text", {})
        csv_data = formats_data.get("csv", {})
        error_data = formats_data.get("error", {})

        return ToolAdapterConfig(
            parser=GlobalParserConfig(**parser_data),
            formats=FormatConfigs(
                json=JsonParserConfig(**json_data),
                html=HtmlParserConfig(**html_data),
                text=TextParserConfig(**text_data),
                csv=CsvParserConfig(**csv_data),
                error=ErrorParserConfig(**error_data),
            ),
            error_handling=ErrorHandlingConfig(
                retry_policy=RetryPolicyConfig(**retry_data),
                circuit_breaker=CircuitBreakerConfig(**cb_data),
            ),
        )
