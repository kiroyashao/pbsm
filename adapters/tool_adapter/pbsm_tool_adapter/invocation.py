from __future__ import annotations

import base64
import time
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum
from typing import Any, Optional

import httpx

from .config import ConfigManager
from .tool_registry import ToolRegistry
from .types import (
    AuthenticationType,
    FormatType,
    InvocationContext,
    InvocationError,
    InvocationParameters,
    InvocationResult,
    InvocationStatus,
    RawOutput,
    ToolAdapterError,
    ToolSpecification,
)


class CircuitState(Enum):
    CLOSED = "CLOSED"
    OPEN = "OPEN"
    HALF_OPEN = "HALF_OPEN"


@dataclass
class RetryPolicy:
    max_retries: int = 3
    initial_delay_ms: int = 1000
    max_delay_ms: int = 30000
    retryable_prefixes: list[str] = field(default_factory=lambda: ["NET_", "TIMEOUT_", "SVC_"])
    backoff_multiplier: float = 2.0


@dataclass
class CircuitBreaker:
    state: CircuitState = CircuitState.CLOSED
    failure_count: int = 0
    success_count: int = 0
    failure_threshold: int = 5
    success_threshold: int = 2
    recovery_timeout_ms: int = 60000
    last_failure_time: float = 0.0

    def record_success(self) -> None:
        if self.state == CircuitState.HALF_OPEN:
            self.success_count += 1
            if self.success_count >= self.success_threshold:
                self.reset()

    def record_failure(self) -> None:
        self.failure_count += 1
        if self.failure_count >= self.failure_threshold:
            self.state = CircuitState.OPEN
            self.last_failure_time = time.monotonic()

    def allow_request(self) -> bool:
        if self.state == CircuitState.CLOSED:
            return True
        if self.state == CircuitState.OPEN:
            elapsed_ms = (time.monotonic() - self.last_failure_time) * 1000
            if elapsed_ms >= self.recovery_timeout_ms:
                self.state = CircuitState.HALF_OPEN
                self.success_count = 0
                return True
            return False
        if self.state == CircuitState.HALF_OPEN:
            return True
        return False

    def reset(self) -> None:
        self.state = CircuitState.CLOSED
        self.failure_count = 0
        self.success_count = 0


class AuthenticationHandler:
    def apply_auth(
        self,
        headers: dict[str, str],
        params: dict[str, str],
        auth_config: dict[str, Any],
    ) -> tuple[dict[str, str], dict[str, str]]:
        auth_type = auth_config.get("type", "NONE")

        if auth_type == "NONE" or auth_type == AuthenticationType.NONE.value:
            return headers, params

        if auth_type == "API_KEY" or auth_type == AuthenticationType.API_KEY.value:
            key_value = auth_config.get("key", auth_config.get("token", ""))
            location = auth_config.get("location", "header")
            if location == "header":
                headers["X-API-Key"] = key_value
            else:
                params["api_key"] = key_value
            return headers, params

        if auth_type == "BASIC" or auth_type == AuthenticationType.BASIC.value:
            username = auth_config.get("username", "")
            password = auth_config.get("password", "")
            credential = base64.b64encode(f"{username}:{password}".encode()).decode()
            headers["Authorization"] = f"Basic {credential}"
            return headers, params

        if auth_type == "OAUTH2" or auth_type == AuthenticationType.OAUTH2.value:
            token = auth_config.get("token", auth_config.get("access_token", ""))
            headers["Authorization"] = f"Bearer {token}"
            return headers, params

        if auth_type == "CUSTOM" or auth_type == AuthenticationType.CUSTOM.value:
            custom_headers = auth_config.get("custom_headers", {})
            headers.update(custom_headers)
            return headers, params

        return headers, params


class ToolInvoker:
    def __init__(self, tool_registry: ToolRegistry, config_manager: ConfigManager) -> None:
        self._registry = tool_registry
        self._config = config_manager
        self._circuit_breakers: dict[str, CircuitBreaker] = {}
        self._auth_handler = AuthenticationHandler()
        self._retry_policy = RetryPolicy()
        self._active_calls: dict[str, dict] = {}

    def _get_circuit_breaker(self, tool_id: str) -> CircuitBreaker:
        if tool_id not in self._circuit_breakers:
            self._circuit_breakers[tool_id] = CircuitBreaker()
        return self._circuit_breakers[tool_id]

    def _is_retryable(self, error_code: str) -> bool:
        return any(error_code.startswith(prefix) for prefix in self._retry_policy.retryable_prefixes)

    def _calculate_delay(self, attempt: int) -> float:
        delay_ms = self._retry_policy.initial_delay_ms * (self._retry_policy.backoff_multiplier ** attempt)
        return min(delay_ms, self._retry_policy.max_delay_ms) / 1000.0

    async def invoke_tool(
        self,
        tool_id: str,
        parameters: InvocationParameters,
        context: InvocationContext | None = None,
    ) -> InvocationResult:
        call_id = str(uuid.uuid4())
        start_time = datetime.now(timezone.utc).isoformat()

        tool = self._registry.get_tool(tool_id)
        if tool is None:
            return InvocationResult(
                success=False,
                call_id=call_id,
                tool_id=tool_id,
                status=InvocationStatus.FAILED,
                error=InvocationError(
                    code="TOOL_NOT_FOUND",
                    message=f"Tool '{tool_id}' not found in registry",
                ),
                timestamps={"started_at": start_time, "completed_at": datetime.now(timezone.utc).isoformat()},
            )

        if not self._registry.is_tool_enabled(tool_id):
            return InvocationResult(
                success=False,
                call_id=call_id,
                tool_id=tool_id,
                status=InvocationStatus.FAILED,
                error=InvocationError(
                    code="TOOL_DISABLED",
                    message=f"Tool '{tool_id}' is not enabled",
                ),
                timestamps={"started_at": start_time, "completed_at": datetime.now(timezone.utc).isoformat()},
            )

        cb = self._get_circuit_breaker(tool_id)
        if not cb.allow_request():
            return InvocationResult(
                success=False,
                call_id=call_id,
                tool_id=tool_id,
                status=InvocationStatus.FAILED,
                error=InvocationError(
                    code="CIRCUIT_OPEN",
                    message="Circuit breaker open",
                ),
                timestamps={"started_at": start_time, "completed_at": datetime.now(timezone.utc).isoformat()},
            )

        effective_timeout = (
            (context.timeout_ms if context and context.timeout_ms else None)
            or tool.timeout_ms
        )

        url = tool.endpoint + (parameters.path or "")
        method = parameters.method
        headers: dict[str, str] = {}
        if parameters.headers:
            headers.update(parameters.headers)
        query_params: dict[str, str] = {}
        if parameters.query_params:
            query_params.update(parameters.query_params)

        headers, query_params = self._auth_handler.apply_auth(
            headers, query_params, tool.authentication
        )

        self._active_calls[call_id] = {
            "tool_id": tool_id,
            "started_at": start_time,
            "method": method,
            "url": url,
        }

        last_error: InvocationError | None = None

        for attempt in range(self._retry_policy.max_retries + 1):
            try:
                async with httpx.AsyncClient() as client:
                    response = await client.request(
                        method=method,
                        url=url,
                        headers=headers,
                        params=query_params,
                        json=parameters.body if isinstance(parameters.body, (dict, list)) else None,
                        content=parameters.body if isinstance(parameters.body, str) else None,
                        timeout=effective_timeout / 1000.0,
                    )

                cb.record_success()
                self._registry.record_invocation(tool_id, True)

                raw_output = RawOutput(
                    content=response.text,
                    content_type=response.headers.get("content-type"),
                    status_code=response.status_code,
                    metadata=dict(response.headers),
                )

                self._active_calls.pop(call_id, None)

                return InvocationResult(
                    success=True,
                    call_id=call_id,
                    tool_id=tool_id,
                    status=InvocationStatus.SUCCESS,
                    status_code=response.status_code,
                    headers=dict(response.headers),
                    body=response.text,
                    timestamps={"started_at": start_time, "completed_at": datetime.now(timezone.utc).isoformat()},
                )

            except httpx.TimeoutException as exc:
                last_error = InvocationError(
                    code="TIMEOUT_ERROR",
                    message=f"Request timed out: {exc}",
                    details={"timeout_ms": effective_timeout, "attempt": attempt},
                )
                if self._is_retryable("TIMEOUT_") and attempt < self._retry_policy.max_retries:
                    delay = self._calculate_delay(attempt)
                    time.sleep(delay)
                    continue

            except httpx.HTTPStatusError as exc:
                status_code = exc.response.status_code
                if 500 <= status_code < 600:
                    error_code = f"SVC_{status_code}"
                elif 400 <= status_code < 500:
                    error_code = f"CLIENT_{status_code}"
                else:
                    error_code = f"HTTP_{status_code}"

                last_error = InvocationError(
                    code=error_code,
                    message=f"HTTP error {status_code}: {exc}",
                    details={"status_code": status_code, "attempt": attempt},
                )

                if self._is_retryable(error_code) and attempt < self._retry_policy.max_retries:
                    delay = self._calculate_delay(attempt)
                    time.sleep(delay)
                    continue

            except Exception as exc:
                last_error = InvocationError(
                    code="NET_ERROR",
                    message=f"Network error: {exc}",
                    details={"attempt": attempt, "exception_type": type(exc).__name__},
                )
                if self._is_retryable("NET_") and attempt < self._retry_policy.max_retries:
                    delay = self._calculate_delay(attempt)
                    time.sleep(delay)
                    continue

            break

        cb.record_failure()
        self._registry.record_invocation(tool_id, False)
        self._active_calls.pop(call_id, None)

        return InvocationResult(
            success=False,
            call_id=call_id,
            tool_id=tool_id,
            status=InvocationStatus.FAILED,
            error=last_error,
            timestamps={"started_at": start_time, "completed_at": datetime.now(timezone.utc).isoformat()},
        )

    def get_circuit_breaker_status(self, tool_id: str) -> dict[str, Any] | None:
        cb = self._circuit_breakers.get(tool_id)
        if cb is None:
            return None
        return {
            "state": cb.state.value,
            "failure_count": cb.failure_count,
            "success_count": cb.success_count,
            "failure_threshold": cb.failure_threshold,
            "success_threshold": cb.success_threshold,
            "recovery_timeout_ms": cb.recovery_timeout_ms,
            "last_failure_time": cb.last_failure_time,
        }
