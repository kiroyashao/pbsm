from __future__ import annotations

import time
import uuid
from datetime import datetime, timezone
from typing import Any, Optional

from .types import (
    FormatType,
    RawOutput,
    ParseOptions,
    ParseResult,
    FormatIdentification,
    StructuredAssertion,
    AssertionType,
    ToolSpecification,
    InvocationParameters,
    InvocationContext,
    InvocationResult,
    ToolAdapterError,
)
from .parsers.json_parser import JsonParser
from .parsers.html_parser import HtmlParser
from .parsers.text_parser import TextParser
from .parsers.csv_parser import CsvParser
from .parsers.error_parser import ErrorParser
from .parsers.base_parser import ParserRegistry
from .tool_registry import ToolRegistry
from .invocation import ToolInvoker
from .pbsm_bindings import PyO3Bridge
from .config import ConfigManager
from .events import (
    EventBus,
    ParsingStartedEvent,
    ParsingCompletedEvent,
    ParsingFailedEvent,
    ToolRegisteredEvent,
    ToolUnregisteredEvent,
    AssertionBatchCompletedEvent,
)


class ToolAdapter:

    def __init__(self, config: dict[str, Any] | None = None, pbsm_config_json: Optional[str] = None):
        self._config_manager = ConfigManager()
        if config:
            self._config_manager.update_config("parser", config, merge=True)
        self._parser_registry = ParserRegistry()
        self._tool_registry = ToolRegistry()
        self._event_bus = EventBus()
        self._pbsm_bridge = PyO3Bridge(config_json=pbsm_config_json)
        self._invoker = ToolInvoker(self._tool_registry, self._config_manager)
        self._parser_registry.register(JsonParser())
        self._parser_registry.register(HtmlParser())
        self._parser_registry.register(TextParser())
        self._parser_registry.register(CsvParser())
        self._parser_registry.register(ErrorParser())

    def parse_tool_output(
        self,
        raw_output: RawOutput,
        tool_id: str = "",
        tool_name: str = "",
        invocation_id: str = "",
        options: ParseOptions | None = None,
    ) -> ParseResult:
        start_time = time.monotonic()
        call_id = str(uuid.uuid4())

        if raw_output.metadata is None:
            raw_output.metadata = {}

        if tool_id:
            raw_output.metadata["tool_id"] = tool_id
        if tool_name:
            raw_output.metadata["tool_name"] = tool_name
        if invocation_id:
            raw_output.metadata["invocation_id"] = invocation_id

        effective_tool_id = tool_id or raw_output.metadata.get("tool_id", "")

        raw_size = len(raw_output.content) if raw_output.content else 0

        self._event_bus.publish(
            ParsingStartedEvent(
                call_id=call_id,
                tool_id=effective_tool_id,
                raw_output_size=raw_size,
                expected_format=options.force_format if options else None,
            )
        )

        if options and options.force_format:
            format_id = FormatIdentification(
                format=options.force_format,
                confidence=1.0,
                detection_method="forced",
            )
        else:
            format_id = self._parser_registry.identify_format(raw_output)

        parser = self._parser_registry.get_parser(format_id.format)

        if parser is None:
            elapsed_ms = (time.monotonic() - start_time) * 1000
            error = ToolAdapterError(
                code="NO_PARSER",
                message=f"No parser registered for format {format_id.format.value}",
                timestamp=datetime.now(timezone.utc).isoformat(),
                correlation_id=call_id,
                recoverable=False,
            )
            self._event_bus.publish(
                ParsingFailedEvent(
                    call_id=call_id,
                    tool_id=effective_tool_id,
                    error=error,
                    recoverable=False,
                )
            )
            return ParseResult(
                success=False,
                assertions=[],
                format=format_id.format,
                format_confidence=format_id.confidence,
                errors=[f"No parser for format {format_id.format.value}"],
                parsing_duration_ms=elapsed_ms,
            )

        result = parser.parse(raw_output, options)

        if options and options.confidence_threshold > 0:
            result.assertions = [
                a for a in result.assertions
                if a.confidence.score >= options.confidence_threshold
            ]

        elapsed_ms = (time.monotonic() - start_time) * 1000
        result.parsing_duration_ms = elapsed_ms

        if result.success:
            self._event_bus.publish(
                ParsingCompletedEvent(
                    call_id=call_id,
                    tool_id=effective_tool_id,
                    actual_format=result.format,
                    format_confidence=result.format_confidence,
                    assertion_count=len(result.assertions),
                    parsing_duration_ms=elapsed_ms,
                    is_partial=result.is_partial,
                    warnings=result.warnings,
                )
            )
        else:
            error = ToolAdapterError(
                code="PARSE_FAILED",
                message="; ".join(result.errors) if result.errors else "Parsing failed",
                timestamp=datetime.now(timezone.utc).isoformat(),
                correlation_id=call_id,
                recoverable=result.is_partial,
            )
            self._event_bus.publish(
                ParsingFailedEvent(
                    call_id=call_id,
                    tool_id=effective_tool_id,
                    error=error,
                    recoverable=result.is_partial,
                    partial_assertions=result.assertions,
                )
            )

        entity_count = 0
        relation_count = 0
        event_count = 0
        error_count = 0
        total_confidence = 0.0

        for assertion in result.assertions:
            if assertion.assertion_type == AssertionType.ENTITY_ATTRIBUTE:
                entity_count += 1
            elif assertion.assertion_type == AssertionType.RELATION:
                relation_count += 1
            elif assertion.assertion_type == AssertionType.EVENT:
                event_count += 1
            elif assertion.assertion_type in (AssertionType.ERROR, AssertionType.ERROR_CONTEXT):
                error_count += 1
            total_confidence += assertion.confidence.score

        avg_confidence = (
            total_confidence / len(result.assertions)
            if result.assertions
            else 0.0
        )

        self._event_bus.publish(
            AssertionBatchCompletedEvent(
                batch_id=str(uuid.uuid4()),
                call_id=call_id,
                tool_id=effective_tool_id,
                total_assertions=len(result.assertions),
                entity_assertions=entity_count,
                relation_assertions=relation_count,
                event_assertions=event_count,
                error_assertions=error_count,
                average_confidence=avg_confidence,
            )
        )

        return result

    def register_tool(
        self, tool_spec: ToolSpecification
    ) -> tuple[bool, str, list[str]]:
        result = self._tool_registry.register_tool(tool_spec)
        success, tool_id, warnings = result
        if success:
            self._event_bus.publish(
                ToolRegisteredEvent(
                    tool_id=tool_id,
                    tool_name=tool_spec.tool_name,
                    supported_formats=tool_spec.supported_formats,
                )
            )
        return result

    async def invoke_tool(
        self,
        tool_id: str,
        parameters: InvocationParameters,
        context: InvocationContext | None = None,
    ) -> InvocationResult:
        result = await self._invoker.invoke_tool(tool_id, parameters, context)
        if result.success and result.body is not None:
            raw_output = RawOutput(
                content=result.body if isinstance(result.body, str) else str(result.body),
                status_code=result.status_code,
            )
            parse_result = self.parse_tool_output(
                raw_output=raw_output,
                tool_id=tool_id,
                invocation_id=result.call_id,
            )
            result.parse_result = parse_result
            result.assertions = parse_result.assertions
        return result

    def parse_tool_output_for_tool(
        self,
        tool_id: str,
        raw_output: RawOutput,
    ) -> ParseResult:
        tool = self._tool_registry.get_tool(tool_id)

        force_format: FormatType | None = None
        custom_options: dict[str, Any] = {}

        if tool is not None:
            if tool.parser_config:
                custom_options.update(tool.parser_config)
            if tool.supported_formats:
                force_format = tool.supported_formats[0]

        options = ParseOptions()

        if custom_options:
            if options.custom_options is None:
                options.custom_options = {}
            options.custom_options.update(custom_options)

        if force_format is not None:
            options.force_format = force_format

        return self.parse_tool_output(
            raw_output=raw_output,
            tool_id=tool_id,
            options=options,
        )

    def submit_to_core(
        self, assertions: list[StructuredAssertion]
    ) -> dict[str, Any]:
        return self._pbsm_bridge.submit_assertions(assertions)

    def verify_prediction(
        self,
        prediction_id: str,
        observations: list[dict[str, Any]],
    ) -> dict[str, Any]:
        return self._pbsm_bridge.verify_prediction(prediction_id, observations)

    def identify_format(
        self,
        raw_output: RawOutput,
        options: ParseOptions | None = None,
    ) -> FormatIdentification:
        return self._parser_registry.identify_format(raw_output)

    def get_config(
        self, category: str | None = None
    ) -> dict[str, Any]:
        return self._config_manager.get_config(category)

    def update_config(
        self,
        category: str,
        settings: dict[str, Any],
        merge: bool = True,
    ) -> dict[str, Any]:
        return self._config_manager.update_config(category, settings, merge)

    @property
    def event_bus(self) -> EventBus:
        return self._event_bus

    @property
    def is_native_mode(self) -> bool:
        return self._pbsm_bridge.is_native

    def start_task(self, description: str) -> dict[str, Any]:
        return self._pbsm_bridge.start_task(description)

    def execute_cycle(self) -> dict[str, Any]:
        return self._pbsm_bridge.execute_cycle()

    def handle_pbsm_error(
        self, error_description: str, severity: str = "medium"
    ) -> dict[str, Any]:
        return self._pbsm_bridge.handle_error(error_description, severity)

    def get_belief_graph_stats(self) -> dict[str, Any]:
        return self._pbsm_bridge.get_belief_graph_stats()

    def get_pbsm_config_json(self) -> str:
        return self._pbsm_bridge.get_config_json()
