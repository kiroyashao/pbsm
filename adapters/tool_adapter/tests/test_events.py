from __future__ import annotations

from pbsm_tool_adapter import (
    EventBus,
    ParsingCompletedEvent,
    FormatType,
    ToolRegisteredEvent,
    ParsingStartedEvent,
)


def test_event_bus_subscribe_publish():
    bus = EventBus()
    received = []

    def handler(event):
        received.append(event)

    bus.subscribe("ParsingStartedEvent", handler)
    event = ParsingStartedEvent(
        event_type="ParsingStartedEvent",
        call_id="call-1",
        tool_id="tool-1",
        raw_output_size=100,
    )
    bus.publish(event)
    assert len(received) == 1
    assert received[0] is event


def test_event_bus_unsubscribe():
    bus = EventBus()
    received = []

    def handler(event):
        received.append(event)

    bus.subscribe("ParsingStartedEvent", handler)
    bus.unsubscribe("ParsingStartedEvent", handler)
    event = ParsingStartedEvent(
        event_type="ParsingStartedEvent",
        call_id="call-1",
        tool_id="tool-1",
        raw_output_size=100,
    )
    bus.publish(event)
    assert len(received) == 0


def test_event_bus_multiple_handlers():
    bus = EventBus()
    received_a = []
    received_b = []

    def handler_a(event):
        received_a.append(event)

    def handler_b(event):
        received_b.append(event)

    bus.subscribe("ParsingStartedEvent", handler_a)
    bus.subscribe("ParsingStartedEvent", handler_b)
    event = ParsingStartedEvent(
        event_type="ParsingStartedEvent",
        call_id="call-1",
        tool_id="tool-1",
        raw_output_size=100,
    )
    bus.publish(event)
    assert len(received_a) == 1
    assert len(received_b) == 1


def test_event_bus_publish_no_handlers():
    bus = EventBus()
    event = ParsingStartedEvent(
        event_type="ParsingStartedEvent",
        call_id="call-1",
        tool_id="tool-1",
        raw_output_size=100,
    )
    bus.publish(event)


def test_event_bus_publish_batch():
    bus = EventBus()
    received = []

    def handler(event):
        received.append(event)

    bus.subscribe("ParsingStartedEvent", handler)
    events = [
        ParsingStartedEvent(
            event_type="ParsingStartedEvent",
            call_id=f"call-{i}", tool_id="tool-1", raw_output_size=100
        )
        for i in range(3)
    ]
    bus.publish_batch(events)
    assert len(received) == 3


def test_event_auto_id():
    event = ParsingStartedEvent(
        call_id="call-1",
        tool_id="tool-1",
        raw_output_size=100,
    )
    assert event.event_id != ""


def test_event_auto_timestamp():
    event = ParsingStartedEvent(
        call_id="call-1",
        tool_id="tool-1",
        raw_output_size=100,
    )
    assert event.timestamp != ""


def test_parsing_completed_event():
    event = ParsingCompletedEvent(
        call_id="call-1",
        tool_id="tool-1",
        actual_format=FormatType.JSON,
        format_confidence=0.95,
        assertion_count=5,
        parsing_duration_ms=12.3,
        is_partial=False,
    )
    assert event.call_id == "call-1"
    assert event.actual_format == FormatType.JSON
    assert event.assertion_count == 5


def test_tool_registered_event():
    event = ToolRegisteredEvent(
        tool_id="tool-1",
        tool_name="Test Tool",
        supported_formats=[FormatType.JSON],
    )
    assert event.tool_id == "tool-1"
    assert event.tool_name == "Test Tool"
    assert FormatType.JSON in event.supported_formats
