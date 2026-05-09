from __future__ import annotations

from pbsm_tool_adapter import ToolRegistry, ToolSpecification, FormatType, ToolStatus


def test_register_tool():
    registry = ToolRegistry()
    spec = ToolSpecification(
        tool_id="tool-1",
        tool_name="Tool One",
        endpoint="https://api.example.com",
        supported_formats=[FormatType.JSON],
    )
    success, tool_id, warnings = registry.register_tool(spec)
    assert success
    assert tool_id == "tool-1"


def test_register_duplicate_tool():
    registry = ToolRegistry()
    spec = ToolSpecification(
        tool_id="tool-1",
        tool_name="Tool One",
        endpoint="https://api.example.com",
        supported_formats=[FormatType.JSON],
    )
    registry.register_tool(spec)
    success, tool_id, warnings = registry.register_tool(spec)
    assert not success


def test_unregister_tool():
    registry = ToolRegistry()
    spec = ToolSpecification(
        tool_id="tool-1",
        tool_name="Tool One",
        endpoint="https://api.example.com",
        supported_formats=[FormatType.JSON],
    )
    registry.register_tool(spec)
    result = registry.unregister_tool("tool-1")
    assert result
    assert registry.get_tool("tool-1") is None


def test_get_tool():
    registry = ToolRegistry()
    spec = ToolSpecification(
        tool_id="tool-1",
        tool_name="Tool One",
        endpoint="https://api.example.com",
        supported_formats=[FormatType.JSON],
    )
    registry.register_tool(spec)
    record = registry.get_tool("tool-1")
    assert record is not None
    assert record.tool_id == "tool-1"
    assert record.tool_name == "Tool One"


def test_get_nonexistent_tool():
    registry = ToolRegistry()
    result = registry.get_tool("nonexistent")
    assert result is None


def test_list_tools():
    registry = ToolRegistry()
    spec1 = ToolSpecification(
        tool_id="tool-1",
        tool_name="Tool One",
        endpoint="https://api1.example.com",
        supported_formats=[FormatType.JSON],
    )
    spec2 = ToolSpecification(
        tool_id="tool-2",
        tool_name="Tool Two",
        endpoint="https://api2.example.com",
        supported_formats=[FormatType.HTML],
    )
    registry.register_tool(spec1)
    registry.register_tool(spec2)
    tools = registry.list_tools()
    assert len(tools) == 2


def test_update_tool():
    registry = ToolRegistry()
    spec = ToolSpecification(
        tool_id="tool-1",
        tool_name="Tool One",
        endpoint="https://api.example.com",
        supported_formats=[FormatType.JSON],
    )
    registry.register_tool(spec)
    updated = registry.update_tool("tool-1", {"tool_name": "Updated Tool"})
    assert updated
    record = registry.get_tool("tool-1")
    assert record.tool_name == "Updated Tool"


def test_tool_capabilities():
    registry = ToolRegistry()
    spec = ToolSpecification(
        tool_id="tool-1",
        tool_name="Tool One",
        endpoint="https://api.example.com",
        supported_formats=[FormatType.JSON],
        authentication={"type": "API_KEY", "key": "abc123"},
    )
    registry.register_tool(spec)
    caps = registry.get_tool_capabilities("tool-1")
    assert caps is not None
    assert FormatType.JSON in caps["supported_formats"]


def test_is_tool_enabled():
    registry = ToolRegistry()
    spec = ToolSpecification(
        tool_id="tool-1",
        tool_name="Tool One",
        endpoint="https://api.example.com",
        supported_formats=[FormatType.JSON],
    )
    registry.register_tool(spec)
    assert registry.is_tool_enabled("tool-1")


def test_record_invocation_success():
    registry = ToolRegistry()
    spec = ToolSpecification(
        tool_id="tool-1",
        tool_name="Tool One",
        endpoint="https://api.example.com",
        supported_formats=[FormatType.JSON],
    )
    registry.register_tool(spec)
    registry.record_invocation("tool-1", success=True)
    record = registry.get_tool("tool-1")
    assert record.invocation_count == 1
    assert record.error_count == 0


def test_record_invocation_failure():
    registry = ToolRegistry()
    spec = ToolSpecification(
        tool_id="tool-1",
        tool_name="Tool One",
        endpoint="https://api.example.com",
        supported_formats=[FormatType.JSON],
    )
    registry.register_tool(spec)
    registry.record_invocation("tool-1", success=False)
    record = registry.get_tool("tool-1")
    assert record.invocation_count == 1
    assert record.error_count == 1
