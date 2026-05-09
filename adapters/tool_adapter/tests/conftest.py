from __future__ import annotations

import pytest
from pbsm_tool_adapter import FormatType, RawOutput, ToolSpecification


@pytest.fixture
def json_raw_output():
    return RawOutput(
        content='{"type": "Server", "id": "srv-1", "name": "web-01", "status": "active"}'
    )


@pytest.fixture
def html_raw_output():
    return RawOutput(
        content=(
            "<html><head><title>Test Page</title></head>"
            "<body><table><tr><th>Name</th><th>Value</th></tr>"
            "<tr><td>key1</td><td>val1</td></tr></table></body></html>"
        )
    )


@pytest.fixture
def text_raw_output():
    return RawOutput(
        content="Server: web-01\nStatus: active\nCPU: 85%\nMemory: 4.2GB"
    )


@pytest.fixture
def csv_raw_output():
    return RawOutput(
        content="Name,Status,CPU\nweb-01,active,85%\ndb-01,running,45%"
    )


@pytest.fixture
def error_raw_output():
    return RawOutput(
        content='{"error": {"code": "TIMEOUT_ERROR", "message": "Request timed out"}}',
        status_code=500,
    )


@pytest.fixture
def sample_tool_spec():
    return ToolSpecification(
        tool_id="test-tool-1",
        tool_name="Test Tool",
        endpoint="https://api.example.com/v1",
        supported_formats=[FormatType.JSON],
    )
