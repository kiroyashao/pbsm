from __future__ import annotations

import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Callable, Optional

from .types import FormatType, InvocationStatus, StructuredAssertion, ToolAdapterError


@dataclass(kw_only=True)
class ToolAdapterEvent:
    event_id: str = field(default="")
    event_type: str = field(default="")
    timestamp: str = field(default="")

    def __post_init__(self):
        if not self.event_id:
            self.event_id = str(uuid.uuid4())
        if not self.timestamp:
            self.timestamp = datetime.now(timezone.utc).isoformat()


@dataclass
class ParserRegisteredEvent(ToolAdapterEvent):
    parser_format: FormatType
    priority: int
    previous_parser: Optional[str] = None


@dataclass
class ParserUnregisteredEvent(ToolAdapterEvent):
    parser_format: FormatType
    reason: str = ""


@dataclass
class ParsingStartedEvent(ToolAdapterEvent):
    call_id: str
    tool_id: str
    raw_output_size: int
    expected_format: Optional[FormatType] = None


@dataclass
class ParsingCompletedEvent(ToolAdapterEvent):
    call_id: str
    tool_id: str
    actual_format: FormatType
    format_confidence: float
    assertion_count: int
    parsing_duration_ms: float
    is_partial: bool
    warnings: list[str] = field(default_factory=list)


@dataclass
class ParsingFailedEvent(ToolAdapterEvent):
    call_id: str
    tool_id: str
    error: ToolAdapterError
    recoverable: bool
    partial_assertions: list[StructuredAssertion] = field(default_factory=list)


@dataclass
class ToolRegisteredEvent(ToolAdapterEvent):
    tool_id: str
    tool_name: str
    supported_formats: list[FormatType] = field(default_factory=list)


@dataclass
class ToolUnregisteredEvent(ToolAdapterEvent):
    tool_id: str
    reason: str
    active_calls: int = 0


@dataclass
class ToolInvocationStartedEvent(ToolAdapterEvent):
    call_id: str
    tool_id: str
    method: str
    endpoint: str
    prediction_id: Optional[str] = None
    intent_id: Optional[str] = None


@dataclass
class ToolInvocationCompletedEvent(ToolAdapterEvent):
    call_id: str
    tool_id: str
    status: InvocationStatus
    status_code: int
    response_size: int
    duration_ms: float
    assertions_generated: int


@dataclass
class ToolInvocationFailedEvent(ToolAdapterEvent):
    call_id: str
    tool_id: str
    error: ToolAdapterError
    retryable: bool
    last_retry_attempt: int = 0


@dataclass
class AssertionGeneratedEvent(ToolAdapterEvent):
    batch_id: str
    call_id: str
    tool_id: str
    assertion: StructuredAssertion
    assertions_in_batch: int


@dataclass
class AssertionBatchCompletedEvent(ToolAdapterEvent):
    batch_id: str
    call_id: str
    tool_id: str
    total_assertions: int
    entity_assertions: int
    relation_assertions: int
    event_assertions: int
    error_assertions: int
    average_confidence: float


@dataclass
class HighValueAssertionEvent(ToolAdapterEvent):
    assertion: StructuredAssertion
    value_score: float
    reasoning: str


class EventBus:
    def __init__(self):
        self._handlers: dict[str, list[Callable]] = {}

    def subscribe(self, event_type: str, handler: Callable) -> None:
        if event_type not in self._handlers:
            self._handlers[event_type] = []
        self._handlers[event_type].append(handler)

    def unsubscribe(self, event_type: str, handler: Callable) -> None:
        if event_type in self._handlers:
            self._handlers[event_type].remove(handler)

    def publish(self, event: ToolAdapterEvent) -> None:
        handlers = self._handlers.get(event.event_type, [])
        for handler in handlers:
            try:
                handler(event)
            except Exception:
                pass

    def publish_batch(self, events: list[ToolAdapterEvent]) -> None:
        for event in events:
            self.publish(event)
