from __future__ import annotations

import base64
import time

from pbsm_tool_adapter import (
    ToolInvoker,
    ToolRegistry,
    ConfigManager,
    CircuitState,
    CircuitBreaker,
    RetryPolicy,
    AuthenticationHandler,
    ToolSpecification,
    InvocationParameters,
    InvocationContext,
    FormatType,
)


def test_circuit_breaker_closed_allows():
    cb = CircuitBreaker()
    assert cb.state == CircuitState.CLOSED
    assert cb.allow_request() is True


def test_circuit_breaker_opens_after_failures():
    cb = CircuitBreaker()
    for _ in range(5):
        cb.record_failure()
    assert cb.state == CircuitState.OPEN


def test_circuit_breaker_half_open_after_recovery():
    cb = CircuitBreaker(recovery_timeout_ms=50)
    for _ in range(5):
        cb.record_failure()
    assert cb.state == CircuitState.OPEN
    time.sleep(0.1)
    assert cb.allow_request() is True
    assert cb.state == CircuitState.HALF_OPEN


def test_circuit_breaker_closes_after_successes():
    cb = CircuitBreaker()
    cb.state = CircuitState.HALF_OPEN
    cb.record_success()
    cb.record_success()
    assert cb.state == CircuitState.CLOSED


def test_circuit_breaker_reset():
    cb = CircuitBreaker()
    for _ in range(5):
        cb.record_failure()
    cb.reset()
    assert cb.state == CircuitState.CLOSED
    assert cb.failure_count == 0
    assert cb.success_count == 0


def test_retry_policy_defaults():
    policy = RetryPolicy()
    assert policy.max_retries == 3
    assert policy.initial_delay_ms == 1000


def test_auth_handler_none():
    handler = AuthenticationHandler()
    headers = {"Content-Type": "application/json"}
    params = {}
    result_headers, result_params = handler.apply_auth(
        headers, params, {"type": "NONE"}
    )
    assert result_headers == headers
    assert result_params == params


def test_auth_handler_api_key():
    handler = AuthenticationHandler()
    headers = {}
    params = {}
    result_headers, result_params = handler.apply_auth(
        headers, params, {"type": "API_KEY", "key": "abc123"}
    )
    assert "X-API-Key" in result_headers
    assert result_headers["X-API-Key"] == "abc123"


def test_auth_handler_bearer():
    handler = AuthenticationHandler()
    headers = {}
    params = {}
    result_headers, result_params = handler.apply_auth(
        headers, params, {"type": "OAUTH2", "token": "abc"}
    )
    assert "Authorization" in result_headers
    assert result_headers["Authorization"] == "Bearer abc"


def test_auth_handler_basic():
    handler = AuthenticationHandler()
    headers = {}
    params = {}
    result_headers, result_params = handler.apply_auth(
        headers, params, {"type": "BASIC", "username": "u", "password": "p"}
    )
    assert "Authorization" in result_headers
    assert result_headers["Authorization"].startswith("Basic ")
    expected_credential = base64.b64encode(b"u:p").decode()
    assert result_headers["Authorization"] == f"Basic {expected_credential}"


def test_invoker_creation():
    registry = ToolRegistry()
    config = ConfigManager()
    invoker = ToolInvoker(registry, config)
    assert invoker is not None


async def test_invoke_nonexistent_tool():
    registry = ToolRegistry()
    config = ConfigManager()
    invoker = ToolInvoker(registry, config)
    params = InvocationParameters(method="GET")
    result = await invoker.invoke_tool("nonexistent", params)
    assert not result.success
