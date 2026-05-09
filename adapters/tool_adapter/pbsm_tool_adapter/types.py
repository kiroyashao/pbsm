from __future__ import annotations

import json
import uuid
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Any, Optional


class FormatType(Enum):
    JSON = "JSON"
    HTML = "HTML"
    TEXT = "TEXT"
    CSV = "CSV"
    ERROR = "ERROR"
    UNKNOWN = "UNKNOWN"


class AssertionType(Enum):
    ENTITY_ATTRIBUTE = "ENTITY_ATTRIBUTE"
    RELATION = "RELATION"
    EVENT = "EVENT"
    ERROR = "ERROR"
    ERROR_CONTEXT = "ERROR_CONTEXT"
    METADATA = "METADATA"


class ValueType(Enum):
    STRING = "STRING"
    NUMBER = "NUMBER"
    BOOLEAN = "BOOLEAN"
    NULL = "NULL"
    ARRAY = "ARRAY"
    OBJECT = "OBJECT"
    DATE = "DATE"
    DURATION = "DURATION"


class ConfidenceMethod(Enum):
    EXACT = "EXACT"
    EXTRACTED = "EXTRACTED"
    INFERRED = "INFERRED"
    ASSUMED = "ASSUMED"


class DataLocationFormat(Enum):
    JSON_PATH = "JSON_PATH"
    CSS_SELECTOR = "CSS_SELECTOR"
    LINE_NUMBER = "LINE_NUMBER"
    CELL_REFERENCE = "CELL_REFERENCE"
    XML_XPATH = "XML_XPATH"
    REGEX_MATCH = "REGEX_MATCH"
    UNKNOWN = "UNKNOWN"


class InvocationStatus(Enum):
    PENDING = "PENDING"
    RUNNING = "RUNNING"
    SUCCESS = "SUCCESS"
    PARTIAL = "PARTIAL"
    FAILED = "FAILED"
    TIMEOUT = "TIMEOUT"


class ToolStatus(Enum):
    ENABLED = "ENABLED"
    DISABLED = "DISABLED"


class AuthenticationType(Enum):
    NONE = "NONE"
    API_KEY = "API_KEY"
    OAUTH2 = "OAUTH2"
    BASIC = "BASIC"
    CUSTOM = "CUSTOM"


class ErrorSeverity(Enum):
    WARNING = "WARNING"
    ERROR = "ERROR"
    CRITICAL = "CRITICAL"


@dataclass
class ConfidenceFactor:
    factor: str
    contribution: float


@dataclass
class ConfidenceInfo:
    score: float
    method: ConfidenceMethod
    factors: list[ConfidenceFactor] = field(default_factory=list)


@dataclass
class DataLocation:
    format: DataLocationFormat
    path: str
    line_number: Optional[int] = None
    column_number: Optional[int] = None


@dataclass
class SourceInfo:
    tool_id: str
    tool_name: str
    invocation_id: str
    data_location: DataLocation


@dataclass
class AssertionMetadata:
    extracted_at: str
    parsing_duration_ms: Optional[int] = None
    parser_version: str = ""
    original_format: FormatType = FormatType.UNKNOWN
    is_partial: bool = False
    warnings: list[str] = field(default_factory=list)
    tags: list[str] = field(default_factory=list)


@dataclass
class AssertionSubject:
    entity_type: str
    entity_id: str
    display_name: Optional[str] = None


@dataclass
class AssertionObject:
    value: Any
    value_type: ValueType
    formatted_value: Optional[str] = None


@dataclass
class StructuredAssertion:
    assertion_id: str
    assertion_type: AssertionType
    subject: AssertionSubject
    predicate: str
    object: AssertionObject
    confidence: ConfidenceInfo
    source: SourceInfo
    metadata: AssertionMetadata

    def to_dict(self) -> dict[str, Any]:
        return {
            "assertionId": self.assertion_id,
            "assertionType": self.assertion_type.value,
            "subject": {
                "entityType": self.subject.entity_type,
                "entityId": self.subject.entity_id,
                "displayName": self.subject.display_name,
            },
            "predicate": self.predicate,
            "object": {
                "value": self.object.value,
                "valueType": self.object.value_type.value,
                "formattedValue": self.object.formatted_value,
            },
            "confidence": {
                "score": self.confidence.score,
                "method": self.confidence.method.value,
                "factors": [
                    {"factor": f.factor, "contribution": f.contribution}
                    for f in self.confidence.factors
                ],
            },
            "source": {
                "toolId": self.source.tool_id,
                "toolName": self.source.tool_name,
                "invocationId": self.source.invocation_id,
                "dataLocation": {
                    "format": self.source.data_location.format.value,
                    "path": self.source.data_location.path,
                    "lineNumber": self.source.data_location.line_number,
                    "columnNumber": self.source.data_location.column_number,
                },
            },
            "metadata": {
                "extractedAt": self.metadata.extracted_at,
                "parsingDurationMs": self.metadata.parsing_duration_ms,
                "parserVersion": self.metadata.parser_version,
                "originalFormat": self.metadata.original_format.value,
                "isPartial": self.metadata.is_partial,
                "warnings": self.metadata.warnings,
                "tags": self.metadata.tags,
            },
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> StructuredAssertion:
        obj_data = data["object"]
        conf_data = data["confidence"]
        src_data = data["source"]
        meta_data = data["metadata"]
        loc_data = src_data["dataLocation"]

        return cls(
            assertion_id=data["assertionId"],
            assertion_type=AssertionType(data["assertionType"]),
            subject=AssertionSubject(
                entity_type=data["subject"]["entityType"],
                entity_id=data["subject"]["entityId"],
                display_name=data["subject"].get("displayName"),
            ),
            predicate=data["predicate"],
            object=AssertionObject(
                value=obj_data["value"],
                value_type=ValueType(obj_data["valueType"]),
                formatted_value=obj_data.get("formattedValue"),
            ),
            confidence=ConfidenceInfo(
                score=conf_data["score"],
                method=ConfidenceMethod(conf_data["method"]),
                factors=[
                    ConfidenceFactor(factor=f["factor"], contribution=f["contribution"])
                    for f in conf_data.get("factors", [])
                ],
            ),
            source=SourceInfo(
                tool_id=src_data["toolId"],
                tool_name=src_data["toolName"],
                invocation_id=src_data["invocationId"],
                data_location=DataLocation(
                    format=DataLocationFormat(loc_data["format"]),
                    path=loc_data["path"],
                    line_number=loc_data.get("lineNumber"),
                    column_number=loc_data.get("columnNumber"),
                ),
            ),
            metadata=AssertionMetadata(
                extracted_at=meta_data["extractedAt"],
                parsing_duration_ms=meta_data.get("parsingDurationMs"),
                parser_version=meta_data["parserVersion"],
                original_format=FormatType(meta_data["originalFormat"]),
                is_partial=meta_data["isPartial"],
                warnings=meta_data.get("warnings", []),
                tags=meta_data.get("tags", []),
            ),
        )


@dataclass
class RawOutput:
    content: str | bytes
    content_type: Optional[str] = None
    metadata: Optional[dict[str, Any]] = None
    status_code: Optional[int] = None


@dataclass
class ParseOptions:
    force_format: Optional[FormatType] = None
    max_depth: int = 10
    max_nodes: int = 1000
    timeout_ms: int = 5000
    confidence_threshold: float = 0.0
    custom_options: Optional[dict[str, Any]] = None


@dataclass
class ParseWarning:
    warning_code: str
    warning_message: str
    location: Optional[dict[str, Any]] = None


@dataclass
class ParseResult:
    success: bool
    assertions: list[StructuredAssertion]
    format: FormatType
    format_confidence: float
    warnings: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)
    parsing_duration_ms: float = 0.0
    is_partial: bool = False


@dataclass
class FormatIdentification:
    format: FormatType
    confidence: float
    detection_method: str
    detected_features: list[str] = field(default_factory=list)
    alternatives: Optional[list[FormatType]] = None


@dataclass
class ParseabilityResult:
    can_parse: bool
    confidence: float
    estimated_complexity: str
    detected_features: list[str] = field(default_factory=list)


@dataclass
class ValidationResult:
    is_valid: bool
    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)


@dataclass
class ToolSpecification:
    tool_id: str
    tool_name: str
    endpoint: str
    supported_formats: list[FormatType] = field(default_factory=list)
    description: Optional[str] = None
    version: str = "1.0.0"
    authentication: dict[str, Any] = field(default_factory=dict)
    parser_config: dict[str, Any] = field(default_factory=dict)
    max_response_size: Optional[int] = None
    timeout_ms: int = 30000


@dataclass
class InvocationParameters:
    method: str
    path: Optional[str] = None
    headers: Optional[dict[str, str]] = None
    body: Optional[Any] = None
    query_params: Optional[dict[str, str]] = None


@dataclass
class InvocationContext:
    prediction_id: Optional[str] = None
    intent_id: Optional[str] = None
    timeout_ms: Optional[int] = None


@dataclass
class InvocationError:
    code: str
    message: str
    details: Optional[dict[str, Any]] = None


@dataclass
class InvocationResult:
    success: bool
    call_id: str
    tool_id: str
    status: InvocationStatus
    status_code: Optional[int] = None
    headers: Optional[dict[str, str]] = None
    body: Optional[Any] = None
    assertions: Optional[list[StructuredAssertion]] = None
    parse_result: Optional[ParseResult] = None
    error: Optional[InvocationError] = None
    timestamps: Optional[dict[str, Any]] = None


@dataclass
class ToolAdapterError:
    code: str
    message: str
    details: Optional[dict[str, Any]] = None
    recoverable: bool = True
    retryable: bool = False
    timestamp: str = ""
    correlation_id: str = ""
