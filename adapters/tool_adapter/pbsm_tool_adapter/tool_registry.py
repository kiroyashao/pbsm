from __future__ import annotations

import re
import uuid
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Optional

from .types import (
    AuthenticationType,
    FormatType,
    ToolAdapterError,
    ToolSpecification,
    ToolStatus,
)


@dataclass
class ToolRecord:
    tool_id: str
    tool_name: str
    endpoint: str
    supported_formats: list[FormatType]
    description: Optional[str] = None
    version: str = "1.0.0"
    authentication: dict[str, Any] = field(default_factory=dict)
    parser_config: dict[str, Any] = field(default_factory=dict)
    max_response_size: Optional[int] = None
    timeout_ms: int = 30000
    status: ToolStatus = ToolStatus.DISABLED
    registered_at: str = ""
    invocation_count: int = 0
    last_invoked_at: Optional[str] = None
    error_count: int = 0


_VERSION_PATTERN = re.compile(r"^\d+\.\d+\.\d+$")


class ToolRegistry:
    def __init__(self) -> None:
        self._tools: dict[str, ToolRecord] = {}

    def register_tool(
        self, tool_spec: ToolSpecification, skip_validation: bool = False
    ) -> tuple[bool, str, list[str]]:
        validation_warnings: list[str] = []

        if not skip_validation:
            if tool_spec.tool_id in self._tools:
                return (False, tool_spec.tool_id, [f"Tool '{tool_spec.tool_id}' is already registered"])

            required_fields = ["tool_id", "tool_name", "endpoint"]
            for fld in required_fields:
                if not getattr(tool_spec, fld, None):
                    validation_warnings.append(f"Required field '{fld}' is missing or empty")

            if not _VERSION_PATTERN.match(tool_spec.version):
                validation_warnings.append(f"Version '{tool_spec.version}' does not match semver format (X.Y.Z)")

            if validation_warnings:
                return (False, tool_spec.tool_id, validation_warnings)

        record = ToolRecord(
            tool_id=tool_spec.tool_id,
            tool_name=tool_spec.tool_name,
            endpoint=tool_spec.endpoint,
            supported_formats=tool_spec.supported_formats,
            description=tool_spec.description,
            version=tool_spec.version,
            authentication=tool_spec.authentication,
            parser_config=tool_spec.parser_config,
            max_response_size=tool_spec.max_response_size,
            timeout_ms=tool_spec.timeout_ms,
            status=ToolStatus.ENABLED,
            registered_at=datetime.now(timezone.utc).isoformat(),
        )

        self._tools[tool_spec.tool_id] = record
        return (True, tool_spec.tool_id, validation_warnings)

    def unregister_tool(self, tool_id: str, reason: str = "") -> bool:
        if tool_id in self._tools:
            del self._tools[tool_id]
            return True
        return False

    def get_tool(self, tool_id: str) -> ToolRecord | None:
        return self._tools.get(tool_id)

    def list_tools(self) -> list[ToolRecord]:
        return list(self._tools.values())

    def update_tool(self, tool_id: str, updates: dict[str, Any]) -> bool:
        record = self._tools.get(tool_id)
        if record is None:
            return False
        for key, value in updates.items():
            if hasattr(record, key):
                setattr(record, key, value)
        return True

    def get_tool_capabilities(self, tool_id: str) -> dict[str, Any] | None:
        record = self._tools.get(tool_id)
        if record is None:
            return None
        return {
            "supported_formats": record.supported_formats,
            "max_response_size": record.max_response_size,
            "timeout_ms": record.timeout_ms,
            "authentication": record.authentication.get("type", AuthenticationType.NONE)
            if isinstance(record.authentication, dict)
            else AuthenticationType.NONE,
        }

    def is_tool_enabled(self, tool_id: str) -> bool:
        record = self._tools.get(tool_id)
        return record is not None and record.status == ToolStatus.ENABLED

    def record_invocation(self, tool_id: str, success: bool) -> None:
        record = self._tools.get(tool_id)
        if record is None:
            return
        record.invocation_count += 1
        record.last_invoked_at = datetime.now(timezone.utc).isoformat()
        if not success:
            record.error_count += 1
