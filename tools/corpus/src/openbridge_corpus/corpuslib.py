"""Maintain the OpenBridge canonical protocol corpus, derived variants, reports, and deterministic packages."""

from __future__ import annotations

import base64
import hashlib
import json
import random
import re
import shutil
import zipfile
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from jsonschema import Draft202012Validator, FormatChecker
from jsonschema.exceptions import SchemaError

from . import __version__


DERIVED_DIRECTORIES = {"generated", "reports", "dist", "runtime"}
MAX_CORPUS_FILE_BYTES = 16 * 1024 * 1024
MAX_JSON_DEPTH = 128
MAX_JSON_NODES = 200_000
MAX_JSON_STRING_BYTES = 8 * 1024 * 1024
MAX_SSE_BLOCKS = 8192
MAX_SSE_EVENTS = 4096
STREAM_ARTIFACTS = {"upstream_stream", "expected_client_stream"}
TERMINAL_TYPES = {
    "response.completed": "response_completed",
    "response.failed": "response_failed",
    "response.incomplete": "response_incomplete",
    "error": "error",
}
SECRET_PATTERNS = [
    re.compile(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{16,}\b"),
    re.compile(r"(?i)\bauthorization\s*[:=]\s*bearer\s+\S+"),
    re.compile(
        r"(?i)\b(?:api[_-]?key|access[_-]?token)\s*[:=]\s*[\"']?[A-Za-z0-9_-]{16,}"
    ),
    re.compile(r"(?i)\bcookie\s*[:=]\s*\S+"),
]


class CorpusError(RuntimeError):
    """Indicate invalid corpus content, schema, or derived-output boundaries."""

    pass


class _DuplicateJsonKeyError(ValueError):
    """Mark a duplicate JSON key without retaining its value in diagnostics."""


class _NonFiniteJsonNumberError(ValueError):
    """Mark a non-finite JSON number without retaining its token in diagnostics."""


@dataclass(frozen=True)
class Case:
    """Represent a canonical corpus case described by case.json."""

    path: Path
    data: dict[str, Any]

    @property
    def directory(self) -> Path:
        """Return the directory containing the case."""
        return self.path.parent

    @property
    def case_id(self) -> str:
        """Return the stable case ID from the manifest."""
        return str(self.data["id"])


@dataclass(frozen=True)
class SemanticCase:
    """Represent a protocol-neutral semantic case described by case.json."""

    path: Path
    data: dict[str, Any]

    @property
    def directory(self) -> Path:
        """Return the directory containing the semantic case."""
        return self.path.parent

    @property
    def case_id(self) -> str:
        """Return the stable semantic case ID from the manifest."""
        return str(self.data["id"])


def load_json(path: Path) -> Any:
    """Read bounded strict JSON and reject duplicate keys, non-finite values, or deep trees."""
    try:
        if path.stat().st_size > MAX_CORPUS_FILE_BYTES:
            raise CorpusError("JSON file exceeds the corpus size limit")
        return _loads_bounded_json(path.read_text(encoding="utf-8"))
    except CorpusError as error:
        raise CorpusError(f"{path}: {error}") from error
    except (
        OSError,
        RecursionError,
        UnicodeError,
        json.JSONDecodeError,
        ValueError,
    ) as error:
        raise CorpusError(f"{path}: cannot read JSON under strict policy") from error


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """Build a JSON object and fail when one object repeats a key."""
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise _DuplicateJsonKeyError("duplicate object key")
        result[key] = value
    return result


def _reject_non_finite_json(_: str) -> None:
    """Reject NaN and infinities because RFC JSON has no non-finite numbers."""
    raise _NonFiniteJsonNumberError("non-finite JSON number")


def _validate_json_complexity(data: Any) -> None:
    """Bound JSON depth, node count, and individual UTF-8 string size."""
    stack: list[tuple[Any, int]] = [(data, 0)]
    nodes = 0
    while stack:
        value, depth = stack.pop()
        nodes += 1
        if nodes > MAX_JSON_NODES:
            raise CorpusError("JSON document exceeds the node limit")
        if depth > MAX_JSON_DEPTH:
            raise CorpusError("JSON document exceeds the depth limit")
        if isinstance(value, str):
            if len(value.encode("utf-8")) > MAX_JSON_STRING_BYTES:
                raise CorpusError("JSON string exceeds the size limit")
        elif isinstance(value, dict):
            stack.extend((item, depth + 1) for item in value.values())
        elif isinstance(value, list):
            stack.extend((item, depth + 1) for item in value)


def _loads_json(text: str) -> Any:
    """Parse RFC JSON with duplicate-key and non-finite-number detection."""
    return json.loads(
        text,
        object_pairs_hook=_reject_duplicate_keys,
        parse_constant=_reject_non_finite_json,
    )


def _loads_bounded_json(text: str) -> Any:
    """Parse one bounded strict JSON value and validate its in-memory complexity."""
    if len(text.encode("utf-8")) > MAX_CORPUS_FILE_BYTES:
        raise CorpusError("JSON text exceeds the corpus size limit")
    try:
        data = _loads_json(text)
    except _DuplicateJsonKeyError as error:
        raise CorpusError("duplicate object key") from error
    except _NonFiniteJsonNumberError as error:
        raise CorpusError("non-finite JSON number") from error
    except (RecursionError, json.JSONDecodeError, ValueError) as error:
        raise CorpusError("cannot read JSON under strict policy") from error
    _validate_json_complexity(data)
    return data


def dump_json(data: Any) -> str:
    """Serialize strict JSON with stable ordering and UTF-8-friendly formatting."""
    return (
        json.dumps(
            data,
            allow_nan=False,
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )


def sha256_bytes(data: bytes) -> str:
    """Return the hexadecimal SHA-256 digest of bytes."""
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    """Read a file and return its SHA-256 digest."""
    return sha256_bytes(path.read_bytes())


def _schema_validator(root: Path, name: str) -> Draft202012Validator:
    """Load and validate the selected corpus schema, returning a validator with format checks."""
    schema_path = root / "schemas" / f"{name}.schema.json"
    schema = load_json(schema_path)
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema, format_checker=FormatChecker())


def _schema_errors(
    validator: Draft202012Validator, data: Any, label: str
) -> list[str]:
    """Convert schema errors to paths and rule names without instance values."""
    errors: list[str] = []
    for error in sorted(validator.iter_errors(data), key=lambda item: list(item.path)):
        location = ".".join(str(part) for part in error.path) or "(root)"
        errors.append(
            f"{label}: {location}: does not satisfy schema rule {error.validator!r}"
        )
    return errors


def discover_cases(root: Path) -> list[Case]:
    """Discover all canonical cases under root in stable path order."""
    cases_root = root / "cases"
    return [
        Case(path=path, data=load_json(path))
        for path in sorted(cases_root.rglob("case.json"))
    ]


def discover_semantic_cases(root: Path) -> list[SemanticCase]:
    """Discover protocol-neutral semantic cases under root in stable path order."""
    cases_root = root / "semantic-cases"
    return [
        SemanticCase(path=path, data=load_json(path))
        for path in sorted(cases_root.rglob("case.json"))
    ]


def _resolve_inside(base: Path, relative: str, allowed_root: Path) -> Path:
    """Resolve a relative path and reject artifacts or derived files outside the allowed root."""
    candidate = (base / relative).resolve()
    resolved_root = allowed_root.resolve()
    if candidate != resolved_root and resolved_root not in candidate.parents:
        raise CorpusError(f"path escapes allowed root: {relative}")
    return candidate


def _derived_output(root: Path, output: Path | None, directory: str) -> Path:
    """Resolve a derived-output directory and prevent overwriting the canonical corpus."""
    allowed_root = (root / directory).resolve()
    candidate = (output if output is not None else allowed_root).resolve()
    if candidate != allowed_root and allowed_root not in candidate.parents:
        raise CorpusError(f"output must stay inside {allowed_root}")
    return candidate


def _parse_sse_events(data: bytes) -> list[dict[str, Any]]:
    """Parse SSE bytes and convert each event to the dictionary required by lint."""
    if len(data) > MAX_CORPUS_FILE_BYTES:
        raise CorpusError("SSE artifact exceeds the corpus size limit")
    text = data.decode("utf-8")
    normalized = text.replace("\r\n", "\n").replace("\r", "\n")
    if normalized.count("\n\n") + 1 > MAX_SSE_BLOCKS:
        raise CorpusError("SSE artifact exceeds the block limit")
    blocks = normalized.split("\n\n")
    events: list[dict[str, Any]] = []
    for block in blocks:
        if not block.strip():
            continue
        event_name: str | None = None
        data_lines: list[str] = []
        for line in block.split("\n"):
            if not line or line.startswith(":"):
                continue
            field, separator, value = line.partition(":")
            if separator and value.startswith(" "):
                value = value[1:]
            if field == "event":
                event_name = value
            elif field == "data":
                data_lines.append(value)
        if not data_lines:
            continue
        payload_text = "\n".join(data_lines)
        payload: Any
        if payload_text == "[DONE]":
            payload = "[DONE]"
        else:
            try:
                payload = _loads_bounded_json(payload_text)
            except CorpusError as error:
                raise CorpusError("invalid SSE data JSON under strict policy") from error
        events.append({"event": event_name, "data": payload})
        if len(events) > MAX_SSE_EVENTS:
            raise CorpusError("SSE artifact exceeds the event limit")
    return events


def terminal_kinds(data: bytes) -> list[str]:
    """Parse bytes and return terminal types in occurrence order."""
    terminals: list[str] = []
    for item in _parse_sse_events(data):
        payload = item["data"]
        if payload == "[DONE]":
            terminals.append("chat_done")
            continue
        event_type = item["event"]
        if isinstance(payload, dict):
            event_type = payload.get("type", event_type)
        if event_type in TERMINAL_TYPES:
            terminals.append(TERMINAL_TYPES[event_type])
    return terminals


def _event_type_conflicts(data: bytes) -> list[tuple[str, str]]:
    """Return events whose SSE field conflicts with the JSON payload type."""
    conflicts: list[tuple[str, str]] = []
    for item in _parse_sse_events(data):
        event_name = item["event"]
        payload = item["data"]
        payload_type = payload.get("type") if isinstance(payload, dict) else None
        if event_name and payload_type and event_name != payload_type:
            conflicts.append((event_name, payload_type))
    return conflicts


def _events_after_terminal(data: bytes) -> int:
    """Count SSE events that appear after the first terminal event."""
    terminal_seen = False
    trailing_events = 0
    for item in _parse_sse_events(data):
        payload = item["data"]
        event_type = item["event"]
        if payload == "[DONE]":
            event_type = "chat_done"
        elif isinstance(payload, dict):
            event_type = payload.get("type", event_type)
        is_terminal = event_type == "chat_done" or event_type in TERMINAL_TYPES
        if terminal_seen:
            trailing_events += 1
        if is_terminal:
            terminal_seen = True
    return trailing_events


def _validate_case_semantics(case: Case, root: Path) -> list[str]:
    """Validate one case's artifact, transport, feature, and terminal semantics."""
    errors: list[str] = []
    data = case.data
    expectation = data["expectation"]
    artifacts = data["artifacts"]
    classification = expectation["classification"]
    stream = data["stream"]
    transport = data.get("transport")
    features = set(data["features"])
    pre_output_failure = (
        transport is not None
        and transport["failure_phase"] == "before_first_output"
    )

    # Validate case directories, classifications, and stream artifact combinations.
    if case.directory.name != case.case_id:
        errors.append(
            f"{case.path}: directory name must equal case id {case.case_id!r}"
        )

    if classification == "reject":
        if expectation["upstream_attempts"] != 0:
            errors.append(f"{case.path}: reject case must have zero upstream attempts")
        forbidden = {
            "expected_upstream_request",
            "upstream_response",
            "upstream_stream",
            "expected_client_stream",
        }.intersection(artifacts)
        if forbidden:
            errors.append(
                f"{case.path}: reject case has forbidden artifacts {sorted(forbidden)}"
            )
        if "expected_client_response" not in artifacts:
            errors.append(f"{case.path}: reject case requires expected_client_response")
        if data["recipes"]:
            errors.append(f"{case.path}: reject case must not define recipes")
    else:
        if expectation["upstream_attempts"] > 0 and "expected_upstream_request" not in artifacts:
            errors.append(
                f"{case.path}: attempted case requires expected_upstream_request"
            )
        if expectation["upstream_attempts"] == 0:
            forbidden = {
                "expected_upstream_request",
                "upstream_response",
                "upstream_stream",
            }.intersection(artifacts)
            if forbidden:
                errors.append(
                    f"{case.path}: zero-attempt case has upstream artifacts "
                    f"{sorted(forbidden)}"
                )
        if stream and not pre_output_failure:
            if "upstream_stream" not in artifacts:
                errors.append(f"{case.path}: streaming case requires upstream_stream")
            if "expected_client_stream" not in artifacts:
                errors.append(
                    f"{case.path}: streaming case requires expected_client_stream"
                )
            forbidden = {
                "upstream_response",
                "expected_client_response",
            }.intersection(artifacts)
            if forbidden:
                errors.append(
                    f"{case.path}: streaming case has non-stream artifacts "
                    f"{sorted(forbidden)}"
                )
        else:
            if "upstream_response" not in artifacts:
                errors.append(f"{case.path}: non-stream case requires upstream_response")
            if "expected_client_response" not in artifacts:
                errors.append(
                    f"{case.path}: non-stream case requires expected_client_response"
                )
            forbidden = {
                "upstream_stream",
                "expected_client_stream",
            }.intersection(artifacts)
            if forbidden:
                errors.append(
                    f"{case.path}: non-stream case has stream artifacts "
                    f"{sorted(forbidden)}"
                )
            if data["recipes"] and not stream:
                errors.append(f"{case.path}: non-stream case must not define recipes")

    # Validate downstream SSE terminal declarations against their counts.
    terminal = expectation["terminal"]
    terminal_count = expectation["terminal_count"]
    if (terminal == "none") != (terminal_count == 0):
        errors.append(
            f"{case.path}: terminal {terminal!r} and terminal_count "
            f"{terminal_count} are inconsistent"
        )
    if not stream and terminal != "none":
        errors.append(f"{case.path}: non-stream case cannot define an SSE terminal")

    # Validate transport failure stages, cancellation, and completion classifications.
    if transport is not None:
        failure_phase = transport["failure_phase"]
        output_observed = transport["downstream_output_observed"]
        if failure_phase == "before_first_output" and output_observed:
            errors.append(
                f"{case.path}: before-output failure cannot observe downstream output"
            )
        if failure_phase == "after_first_output" and not output_observed:
            errors.append(
                f"{case.path}: after-output failure requires downstream output"
            )
        if expectation["outcome"] == "cancelled":
            if transport["client_end"] != "cancelled":
                errors.append(
                    f"{case.path}: cancelled outcome requires cancelled client_end"
                )
            if transport["cancellation_after_event"] is None:
                errors.append(
                    f"{case.path}: cancelled outcome requires cancellation_after_event"
                )
        elif transport["cancellation_after_event"] is not None:
            errors.append(
                f"{case.path}: cancellation_after_event requires cancelled outcome"
            )
        if terminal != "none" and transport["client_end"] != "terminal":
            errors.append(
                f"{case.path}: declared terminal requires terminal client_end"
            )
        if not stream and expectation["outcome"] == "completed":
            if transport["client_end"] != "response":
                errors.append(
                    f"{case.path}: completed non-stream case requires response "
                    "client_end"
                )
            if transport["upstream_end"] != "response":
                errors.append(
                    f"{case.path}: completed non-stream case requires response "
                    "upstream_end"
                )

    # Parse every artifact and validate JSON, SSE, paths, and declaration completeness.
    referenced_artifacts: set[Path] = set()
    for artifact_name, relative in artifacts.items():
        try:
            artifact_path = _resolve_inside(case.directory, relative, case.directory)
        except CorpusError as error:
            errors.append(f"{case.path}: {error}")
            continue
        referenced_artifacts.add(artifact_path)
        if not artifact_path.is_file():
            errors.append(f"{case.path}: missing artifact {artifact_name}: {relative}")
            continue
        try:
            raw = artifact_path.read_bytes()
            raw.decode("utf-8")
        except (OSError, UnicodeError) as error:
            errors.append(f"{artifact_path}: invalid UTF-8 artifact: {error}")
            continue
        if artifact_path.suffix == ".json":
            try:
                _loads_bounded_json(raw.decode("utf-8"))
            except CorpusError as error:
                errors.append(f"{artifact_path}: invalid JSON artifact: {error}")
        if artifact_name in STREAM_ARTIFACTS:
            try:
                _parse_sse_events(raw)
            except (CorpusError, UnicodeError) as error:
                errors.append(f"{artifact_path}: {error}")
                continue
            conflicts = _event_type_conflicts(raw)
            if conflicts and not (
                artifact_name == "upstream_stream"
                and "event_type_conflict" in features
            ):
                errors.append(
                    f"{artifact_path}: SSE event/data type conflicts {conflicts!r}"
                )
            trailing_events = _events_after_terminal(raw)
            if trailing_events and not (
                artifact_name == "upstream_stream"
                and "event_after_terminal" in features
            ):
                errors.append(
                    f"{artifact_path}: {trailing_events} event(s) occur after terminal"
                )

    # Prove special feature declarations with upstream SSE content.
    upstream_stream = artifacts.get("upstream_stream")
    if upstream_stream:
        stream_path = _resolve_inside(case.directory, upstream_stream, case.directory)
        if stream_path.is_file():
            raw = stream_path.read_bytes()
            if (
                "event_type_conflict" in features
                and not _event_type_conflicts(raw)
            ):
                errors.append(
                    f"{case.path}: event_type_conflict feature requires an upstream "
                    "SSE event/data type conflict"
                )
            if (
                "duplicate_terminal" in features
                and len(terminal_kinds(raw)) < 2
            ):
                errors.append(
                    f"{case.path}: duplicate_terminal feature requires at least two "
                    "upstream terminals"
                )
            if (
                "event_after_terminal" in features
                and _events_after_terminal(raw) == 0
            ):
                errors.append(
                    f"{case.path}: event_after_terminal feature requires a trailing "
                    "upstream event"
                )

    # Prove terminal identity and count with expected downstream SSE content.
    expected_stream = artifacts.get("expected_client_stream")
    if expected_stream:
        stream_path = _resolve_inside(case.directory, expected_stream, case.directory)
        if stream_path.is_file():
            try:
                terminals = terminal_kinds(stream_path.read_bytes())
            except (CorpusError, UnicodeError):
                terminals = []
            if len(terminals) != expectation["terminal_count"]:
                errors.append(
                    f"{case.path}: expected terminal_count "
                    f"{expectation['terminal_count']}, observed {len(terminals)}"
                )
            expected_terminal = expectation["terminal"]
            if terminals and any(kind != expected_terminal for kind in terminals):
                errors.append(
                    f"{case.path}: terminal kinds {terminals!r} do not match "
                    f"{expected_terminal!r}"
                )
    elif expectation["terminal_count"] != 0:
        errors.append(
            f"{case.path}: terminal_count is non-zero without expected_client_stream"
        )

    # Reject extra oracles in a case directory that the manifest does not declare.
    declared_files = referenced_artifacts | {case.path.resolve()}
    for path in sorted(case.directory.rglob("*")):
        if path.is_file() and path.resolve() not in declared_files:
            errors.append(f"{case.path}: undeclared case file {path.relative_to(case.directory)}")

    # Validate that provenance and generation recipes remain within the corpus root and exist.
    provenance = data["provenance_ref"]
    try:
        provenance_path = _resolve_inside(root, provenance, root)
        if not provenance_path.is_file():
            errors.append(f"{case.path}: missing provenance {provenance}")
    except CorpusError as error:
        errors.append(f"{case.path}: {error}")

    for recipe in data["recipes"]:
        try:
            recipe_path = _resolve_inside(root, recipe, root)
            if not recipe_path.is_file():
                errors.append(f"{case.path}: missing recipe {recipe}")
        except CorpusError as error:
            errors.append(f"{case.path}: {error}")

    return errors


def _strict_schema_errors(schema: Any, path: str) -> list[str]:
    """Return paths where a strict function schema leaves object fields open or optional."""
    errors: list[str] = []
    if isinstance(schema, dict):
        # Enforce strict object closure and require every declared property.
        properties = schema.get("properties")
        if schema.get("type") == "object" or isinstance(properties, dict):
            if schema.get("additionalProperties") is not False:
                errors.append(f"{path}.additionalProperties must be false")
            property_names = set(properties or {})
            required_names = set(schema.get("required", []))
            if property_names != required_names:
                errors.append(f"{path}.required must contain every property")

        # Recurse through every schema-valued keyword that can contain nested objects.
        for keyword in (
            "properties",
            "patternProperties",
            "$defs",
            "definitions",
            "dependentSchemas",
        ):
            children = schema.get(keyword)
            if isinstance(children, dict):
                for name, child in children.items():
                    errors.extend(_strict_schema_errors(child, f"{path}.{keyword}.{name}"))
        for keyword in ("allOf", "anyOf", "oneOf", "prefixItems"):
            children = schema.get(keyword)
            if isinstance(children, list):
                for index, child in enumerate(children):
                    errors.extend(
                        _strict_schema_errors(child, f"{path}.{keyword}[{index}]")
                    )
        items = schema.get("items")
        if isinstance(items, (dict, list)):
            errors.extend(_strict_schema_errors(items, f"{path}.items"))
        for keyword in (
            "additionalProperties",
            "contains",
            "else",
            "if",
            "not",
            "propertyNames",
            "then",
            "unevaluatedItems",
            "unevaluatedProperties",
        ):
            child = schema.get(keyword)
            if isinstance(child, (dict, list)):
                errors.extend(_strict_schema_errors(child, f"{path}.{keyword}"))
    elif isinstance(schema, list):
        for index, child in enumerate(schema):
            errors.extend(_strict_schema_errors(child, f"{path}[{index}]"))
    return errors


def _semantic_provenance_errors(case: SemanticCase, root: Path) -> list[str]:
    """Resolve one semantic case provenance record inside the canonical corpus."""
    provenance = case.data["provenance_ref"]
    try:
        provenance_path = _resolve_inside(root, provenance, root)
        if not provenance_path.is_file():
            return [f"{case.path}: missing provenance {provenance}"]
    except CorpusError as error:
        return [f"{case.path}: {error}"]
    return []


def _validate_semantic_case_semantics(
    case: SemanticCase, root: Path
) -> list[str]:
    """Validate semantic artifacts, tools, controls, oracle consistency, and provenance."""
    errors: list[str] = []
    data = case.data

    # Validate the case identity, reference trace, and undeclared-file boundary.
    if case.directory.name != case.case_id:
        errors.append(
            f"{case.path}: directory name must equal semantic case id {case.case_id!r}"
        )
    reference = data["artifacts"]["reference_trace"]
    try:
        trace_path = _resolve_inside(case.directory, reference, case.directory)
    except CorpusError as error:
        errors.append(f"{case.path}: {error}")
        trace_path = None
    else:
        if not trace_path.is_file():
            errors.append(f"{case.path}: missing reference trace {reference}")
    declared_files = {case.path.resolve()}
    if trace_path is not None:
        declared_files.add(trace_path)
    for path in sorted(case.directory.rglob("*")):
        if path.is_file() and path.resolve() not in declared_files:
            relative = path.relative_to(case.directory)
            errors.append(f"{case.path}: undeclared semantic case file {relative}")

    task = data["task"]
    task_kind = task.get("kind")
    if task_kind == "context":
        recipe = task["context"]
        generation_fields = [
            task["instruction"],
            task["question"],
            recipe["needle"],
            recipe["distractor_template"],
        ]
        if not all(value.isascii() for value in generation_fields):
            errors.append(f"{case.path}: context generation fields must be ASCII")
        template = recipe["distractor_template"]
        if "{index" not in template or "{token}" not in template:
            errors.append(
                f"{case.path}: distractor_template must include index and token placeholders"
            )
        if data["oracle"]["calls"]["required"] or data["oracle"]["results"]["required"]:
            errors.append(f"{case.path}: context cases cannot require tool calls or results")
        if not data["oracle"]["final_response"]["required"]:
            errors.append(f"{case.path}: context cases require a final response")
        if not errors:
            from .semantic_plan import build_semantic_plan

            for target_bytes in recipe["target_bytes"]:
                for placement in recipe["placements"]:
                    try:
                        build_semantic_plan(
                            root,
                            case.case_id,
                            target_bytes=target_bytes,
                            placement=placement,
                        )
                    except CorpusError as error:
                        errors.append(f"{case.path}: invalid context recipe: {error}")
        errors.extend(_semantic_provenance_errors(case, root))
        return errors

    if task_kind == "structured":
        response_format = task["response_format"]
        response_schema = response_format["schema"]
        try:
            Draft202012Validator.check_schema(response_schema)
        except SchemaError:
            errors.append(f"{case.path}: response_format.schema is not valid JSON Schema")
        else:
            if response_format["strict"]:
                for error in _strict_schema_errors(
                    response_schema, "task.response_format.schema"
                ):
                    errors.append(f"{case.path}: strict response schema {error}")
        if data["oracle"]["calls"]["required"] or data["oracle"]["results"]["required"]:
            errors.append(
                f"{case.path}: structured cases cannot require tool calls or results"
            )
        if not data["oracle"]["final_response"]["required"]:
            errors.append(f"{case.path}: structured cases require a final response")
        errors.extend(_semantic_provenance_errors(case, root))
        return errors

    # Validate each tool schema and collect stable names for oracle cross-checks.
    tools = data["task"]["tools"]
    tool_names = [tool["name"] for tool in tools]
    if len(set(tool_names)) != len(tool_names):
        errors.append(f"{case.path}: semantic tool names must be unique")
    tools_by_name = {tool["name"]: tool for tool in tools}
    for index, tool in enumerate(tools):
        parameters = tool["parameters"]
        try:
            Draft202012Validator.check_schema(parameters)
        except SchemaError:
            errors.append(
                f"{case.path}: task.tools[{index}].parameters is not a valid JSON Schema"
            )
            continue
        if tool["strict"]:
            for error in _strict_schema_errors(
                parameters, f"task.tools[{index}].parameters"
            ):
                errors.append(f"{case.path}: strict tool schema {error}")

    # Cross-check controls with the required call set and declared tool names.
    controls = data["task"]["controls"]
    calls = data["oracle"]["calls"]
    required_calls = calls["required"]
    required_results = data["oracle"]["results"]["required"]
    choice = controls["tool_choice"]
    if choice["mode"] == "function" and choice.get("name") not in tools_by_name:
        errors.append(f"{case.path}: forced tool_choice name is not declared")
    if choice["mode"] == "none" and required_calls:
        errors.append(f"{case.path}: tool_choice none cannot require tool calls")
    if choice["mode"] == "required" and not required_calls:
        errors.append(f"{case.path}: tool_choice required needs a required call")
    if not controls["parallel_tool_calls"] and len(required_calls) > 1:
        errors.append(
            f"{case.path}: parallel_tool_calls false cannot require multiple calls"
        )
    result_call_indices = [
        result["required_call_index"] for result in required_results
    ]
    if len(set(result_call_indices)) != len(result_call_indices):
        errors.append(f"{case.path}: required result call indices must be unique")
    for index, call_index in enumerate(result_call_indices):
        if call_index >= len(required_calls):
            errors.append(
                f"{case.path}: required_results[{index}].required_call_index "
                "does not identify a required call"
            )

    # Validate each expected call against its declared function parameter schema.
    for index, expected in enumerate(required_calls):
        tool = tools_by_name.get(expected["name"])
        if tool is None:
            errors.append(f"{case.path}: required_calls[{index}].name is not declared")
            continue
        validator = Draft202012Validator(
            tool["parameters"], format_checker=FormatChecker()
        )
        if list(validator.iter_errors(expected["arguments"])):
            errors.append(
                f"{case.path}: required_calls[{index}].arguments do not satisfy "
                "the declared parameters"
            )
    if choice["mode"] == "function" and any(
        expected["name"] != choice["name"] for expected in required_calls
    ):
        errors.append(f"{case.path}: required calls conflict with forced tool_choice")

    errors.extend(_semantic_provenance_errors(case, root))
    return errors


def lint_corpus(root: Path) -> list[str]:
    """Validate corpus schemas, references, semantics, integrity, and suspected secrets."""
    # Resolve the corpus and first require every canonical directory and schema.
    root = root.resolve()
    errors: list[str] = []
    required = [
        root / "VERSION",
        root / "catalog.json",
        root / "cases",
        root / "semantic-cases",
        root / "sources",
        root / "recipes",
        root / "schemas" / "catalog.schema.json",
        root / "schemas" / "case.schema.json",
        root / "schemas" / "semantic-case.schema.json",
        root / "schemas" / "semantic-plan.schema.json",
        root / "schemas" / "semantic-trace.schema.json",
        root / "schemas" / "provenance.schema.json",
        root / "schemas" / "recipe.schema.json",
        root / "schemas" / "server-scenario.schema.json",
        root / "schemas" / "server-suite.schema.json",
        root / "schemas" / "client-plan.schema.json",
        root / "schemas" / "observation.schema.json",
        root / "schemas" / "server-run-observation.schema.json",
    ]
    for path in required:
        if not path.exists():
            errors.append(f"missing required corpus file: {path}")
    if errors:
        return errors

    # Reject oversized canonical files before any schema, wire, hash, or pack read.
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        relative = path.relative_to(root)
        if relative.parts and relative.parts[0] in DERIVED_DIRECTORIES:
            continue
        try:
            oversized = path.stat().st_size > MAX_CORPUS_FILE_BYTES
        except OSError:
            errors.append(f"cannot stat canonical corpus file: {relative}")
            continue
        if oversized:
            errors.append(f"canonical corpus file exceeds size limit: {relative}")
    if errors:
        return errors

    # Initialize all schema validators before reading documents that depend on them.
    try:
        catalog_validator = _schema_validator(root, "catalog")
        case_validator = _schema_validator(root, "case")
        semantic_case_validator = _schema_validator(root, "semantic-case")
        _schema_validator(root, "semantic-plan")
        semantic_trace_validator = _schema_validator(root, "semantic-trace")
        provenance_validator = _schema_validator(root, "provenance")
        recipe_validator = _schema_validator(root, "recipe")
        _schema_validator(root, "server-scenario")
        _schema_validator(root, "server-suite")
        _schema_validator(root, "client-plan")
        _schema_validator(root, "observation")
        _schema_validator(root, "server-run-observation")
    except Exception as error:
        return [f"schema initialization failed: {error}"]

    # Validate catalog identity and release version consistency.
    catalog = load_json(root / "catalog.json")
    errors.extend(_schema_errors(catalog_validator, catalog, "catalog.json"))
    version = (root / "VERSION").read_text(encoding="utf-8").strip()
    if catalog.get("corpus_version") != version:
        errors.append(
            f"catalog corpus_version {catalog.get('corpus_version')!r} "
            f"does not match VERSION {version!r}"
        )

    # Discover wire cases and cross-check their stable catalog membership.
    try:
        cases = discover_cases(root)
    except CorpusError as error:
        return errors + [str(error)]
    discovered_ids = [case.case_id for case in cases]
    if sorted(catalog.get("case_ids", [])) != sorted(discovered_ids):
        missing = sorted(set(catalog.get("case_ids", [])) - set(discovered_ids))
        extra = sorted(set(discovered_ids) - set(catalog.get("case_ids", [])))
        errors.append(f"catalog/case mismatch: missing={missing}, extra={extra}")

    # Validate every wire manifest and its declared artifacts and transport semantics.
    for case in cases:
        schema_errors = _schema_errors(case_validator, case.data, str(case.path))
        errors.extend(schema_errors)
        if not schema_errors:
            errors.extend(_validate_case_semantics(case, root))

    # Discover semantic cases and cross-check their independent catalog membership.
    try:
        semantic_cases = discover_semantic_cases(root)
    except CorpusError as error:
        return errors + [str(error)]
    discovered_semantic_ids = [case.case_id for case in semantic_cases]
    if sorted(catalog.get("semantic_case_ids", [])) != sorted(
        discovered_semantic_ids
    ):
        missing = sorted(
            set(catalog.get("semantic_case_ids", [])) - set(discovered_semantic_ids)
        )
        extra = sorted(
            set(discovered_semantic_ids) - set(catalog.get("semantic_case_ids", []))
        )
        errors.append(
            f"catalog/semantic-case mismatch: missing={missing}, extra={extra}"
        )

    # Validate semantic manifests, reference traces, and positive oracle verdicts.
    for case in semantic_cases:
        schema_errors = _schema_errors(
            semantic_case_validator, case.data, str(case.path)
        )
        errors.extend(schema_errors)
        if schema_errors:
            continue
        semantic_errors = _validate_semantic_case_semantics(case, root)
        errors.extend(semantic_errors)
        reference = case.directory / case.data["artifacts"]["reference_trace"]
        if semantic_errors or not reference.is_file():
            continue
        try:
            trace = load_json(reference)
        except CorpusError as error:
            errors.append(str(error))
            continue
        trace_schema_errors = _schema_errors(
            semantic_trace_validator, trace, str(reference)
        )
        errors.extend(trace_schema_errors)
        if not trace_schema_errors:
            from .semantic import verify_semantic_trace

            for error in verify_semantic_trace(root, case.case_id, trace):
                errors.append(f"{reference}: reference trace: {error}")

    # Validate provenance and generation recipes as independent canonical documents.
    for path in sorted((root / "sources").glob("*.json")):
        try:
            data = load_json(path)
        except CorpusError as error:
            errors.append(str(error))
        else:
            errors.extend(_schema_errors(provenance_validator, data, str(path)))

    for path in sorted((root / "recipes").glob("*.json")):
        try:
            data = load_json(path)
        except CorpusError as error:
            errors.append(str(error))
        else:
            errors.extend(_schema_errors(recipe_validator, data, str(path)))

    # Scan all canonical manifests and artifacts for common credential patterns.
    scan_paths: set[Path] = set()
    for case in cases:
        scan_paths.add(case.path)
        for relative in case.data.get("artifacts", {}).values():
            try:
                scan_paths.add(_resolve_inside(case.directory, relative, root))
            except CorpusError:
                pass
    for case in semantic_cases:
        scan_paths.add(case.path)
        for relative in case.data.get("artifacts", {}).values():
            try:
                scan_paths.add(_resolve_inside(case.directory, relative, root))
            except CorpusError:
                pass
    scan_paths.update((root / "sources").glob("*.json"))
    for path in sorted(scan_paths):
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for pattern in SECRET_PATTERNS:
            if pattern.search(text):
                errors.append(f"{path}: suspected secret matched {pattern.pattern!r}")

    return errors


def _chunk_one_byte(data: bytes) -> list[bytes]:
    """Split wire data into single-byte chunks to cover the smallest read boundaries."""
    return [data[index : index + 1] for index in range(len(data))]


def _chunk_lines(data: bytes) -> list[bytes]:
    """Split wire data at line boundaries while preserving line endings."""
    return data.splitlines(keepends=True) or [data]


def _chunk_utf8_split(data: bytes) -> list[bytes]:
    """Prefer splitting wire data inside multibyte UTF-8 sequences."""
    for index, value in enumerate(data):
        if value >= 0x80 and index + 1 < len(data):
            return [data[: index + 1], data[index + 1 :]]
    midpoint = max(1, len(data) // 2)
    return [data[:midpoint], data[midpoint:]]


def _chunk_seeded(data: bytes, seed: int) -> list[bytes]:
    """Generate a bounded random chunk sequence from a stable seed."""
    generator = random.Random(seed)
    chunks: list[bytes] = []
    index = 0
    maximum = max(1, min(17, len(data)))
    while index < len(data):
        size = generator.randint(1, maximum)
        chunks.append(data[index : index + size])
        index += size
    return chunks


def _chunk_event_pairs(data: bytes) -> list[bytes]:
    """Pair chunks around adjacent SSE events to cover events crossing read boundaries."""
    parts = re.split(rb"((?:\r\n|\r|\n){2})", data)
    frames: list[bytes] = []
    for index in range(0, len(parts) - 1, 2):
        frames.append(parts[index] + parts[index + 1])
    if len(parts) % 2 == 1 and parts[-1]:
        frames.append(parts[-1])
    if not frames:
        return [data]
    return [b"".join(frames[index : index + 2]) for index in range(0, len(frames), 2)]


def _to_crlf(data: bytes) -> bytes:
    """Normalize existing line endings to CRLF without applying conversion twice."""
    return data.replace(b"\r\n", b"\n").replace(b"\r", b"\n").replace(b"\n", b"\r\n")


def _variant_payload(
    case_id: str,
    artifact: str,
    kind: str,
    seed: int,
    canonical: bytes,
    wire: bytes,
    chunks: list[bytes],
    transformation: str,
) -> dict[str, Any]:
    """Build and validate digest, chunk, and reconstruction metadata for one wire variant."""
    rebuilt = b"".join(chunks)
    if rebuilt != wire:
        raise CorpusError(f"{case_id}/{artifact}/{kind}: chunks do not reconstruct wire")
    return {
        "artifact": artifact,
        "canonical_sha256": sha256_bytes(canonical),
        "case_id": case_id,
        "chunks_base64": [
            base64.b64encode(chunk).decode("ascii") for chunk in chunks
        ],
        "encoding": "base64",
        "kind": kind,
        "reconstructed_sha256": sha256_bytes(rebuilt),
        "seed": seed,
        "source_sha256": sha256_bytes(canonical),
        "transformation": transformation,
        "wire_sha256": sha256_bytes(wire),
    }


def generate_variants(
    root: Path, seed: int | None = None, output: Path | None = None
) -> dict[str, Any]:
    """Generate a deterministic wire variant with the recipe's chunking, line endings, and encoding."""
    root = root.resolve()
    errors = lint_corpus(root)
    if errors:
        raise CorpusError("corpus lint failed:\n" + "\n".join(errors))
    catalog = load_json(root / "catalog.json")
    effective_seed = (
        int(seed) if seed is not None else int(catalog["default_generation_seed"])
    )
    output = _derived_output(root, output, "generated")
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)

    generated: list[dict[str, Any]] = []
    for case in discover_cases(root):
        for recipe_ref in case.data["recipes"]:
            recipe = load_json(_resolve_inside(root, recipe_ref, root))
            for artifact_name in sorted(STREAM_ARTIFACTS):
                relative = case.data["artifacts"].get(artifact_name)
                if not relative:
                    continue
                source = _resolve_inside(case.directory, relative, root).read_bytes()
                variants: list[tuple[str, int, bytes, list[bytes], str]] = []
                for kind in recipe["kinds"]:
                    if kind == "one_byte":
                        variants.append(
                            (kind, effective_seed, source, _chunk_one_byte(source), "none")
                        )
                    elif kind == "line_boundaries":
                        variants.append(
                            (kind, effective_seed, source, _chunk_lines(source), "none")
                        )
                    elif kind == "utf8_split":
                        variants.append(
                            (kind, effective_seed, source, _chunk_utf8_split(source), "none")
                        )
                    elif kind == "all_in_one":
                        variants.append(
                            (kind, effective_seed, source, [source], "none")
                        )
                    elif kind == "event_pairs":
                        variants.append(
                            (
                                kind,
                                effective_seed,
                                source,
                                _chunk_event_pairs(source),
                                "none",
                            )
                        )
                    elif kind == "crlf":
                        crlf = _to_crlf(source)
                        variants.append(
                            (
                                kind,
                                effective_seed,
                                crlf,
                                _chunk_lines(crlf),
                                "line_endings_crlf",
                            )
                        )
                    elif kind == "seeded":
                        for index in range(recipe["seeded_variants"]):
                            derived = int.from_bytes(
                                hashlib.sha256(
                                    f"{effective_seed}:{case.case_id}:{artifact_name}:{index}".encode()
                                ).digest()[:8],
                                "big",
                            )
                            variants.append(
                                (
                                    f"seeded_{index + 1}",
                                    derived,
                                    source,
                                    _chunk_seeded(source, derived),
                                    "none",
                                )
                            )
                for kind, variant_seed, wire, chunks, transformation in variants:
                    payload = _variant_payload(
                        case.case_id,
                        artifact_name,
                        kind,
                        variant_seed,
                        source,
                        wire,
                        chunks,
                        transformation,
                    )
                    relative_output = (
                        Path(case.case_id)
                        / f"{artifact_name}.{recipe['id']}.{kind}.json"
                    )
                    destination = output / relative_output
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    destination.write_text(dump_json(payload), encoding="utf-8")
                    generated.append(
                        {
                            "path": relative_output.as_posix(),
                            "sha256": sha256_file(destination),
                        }
                    )

    manifest = {
        "corpus_version": catalog["corpus_version"],
        "files": sorted(generated, key=lambda item: item["path"]),
        "generator_version": __version__,
        "schema_version": "0.1",
        "seed": effective_seed,
    }
    (output / "manifest.json").write_text(dump_json(manifest), encoding="utf-8")
    return manifest


def build_report(root: Path) -> dict[str, Any]:
    """Summarize case, feature, status, provenance, and generation coverage."""
    # Validate the corpus before trusting any coverage counters.
    root = root.resolve()
    errors = lint_corpus(root)
    if errors:
        raise CorpusError("corpus lint failed:\n" + "\n".join(errors))
    catalog = load_json(root / "catalog.json")
    cases = discover_cases(root)
    semantic_cases = discover_semantic_cases(root)
    # Count wire classifications, directions, statuses, and feature labels.
    status = Counter(case.data["status"] for case in cases)
    classifications = Counter(
        case.data["expectation"]["classification"] for case in cases
    )
    directions: dict[str, Counter[str]] = defaultdict(Counter)
    features = Counter()
    for case in cases:
        directions[case.data["direction"]]["stream" if case.data["stream"] else "non_stream"] += 1
        features.update(case.data["features"])
    # Count protocol-neutral semantic statuses, targets, and feature labels.
    semantic_features = Counter()
    semantic_status = Counter()
    semantic_targets = Counter()
    for case in semantic_cases:
        semantic_features.update(case.data["features"])
        semantic_status.update([case.data["status"]])
        semantic_targets.update(case.data["applies_to"])

    # Surface unresolved provenance metadata instead of treating it as validated.
    pending_sources: list[str] = []
    unpinned_sources: list[str] = []
    for path in sorted((root / "sources").glob("*.json")):
        source = load_json(path)
        if source["license_status"] == "pending":
            pending_sources.append(source["id"])
        if source["ref"] is None:
            unpinned_sources.append(source["id"])

    # Compare observed wire, semantic, and generation coverage with catalog requirements.
    required = set(catalog["required_core_features"])
    required_semantic = set(catalog["required_semantic_features"])
    required_generation = set(catalog["required_generation_kinds"])
    observed_generation: set[str] = set()
    for path in sorted((root / "recipes").glob("*.json")):
        observed_generation.update(load_json(path)["kinds"])
    # Build a stable machine-readable report for CLI and documentation consumers.
    return {
        "case_count": len(cases),
        "classifications": dict(sorted(classifications.items())),
        "corpus_version": catalog["corpus_version"],
        "directions": {
            direction: dict(sorted(counts.items()))
            for direction, counts in sorted(directions.items())
        },
        "feature_counts": dict(sorted(features.items())),
        "missing_required_features": sorted(required - set(features)),
        "missing_required_generation_kinds": sorted(
            required_generation - observed_generation
        ),
        "missing_required_semantic_features": sorted(
            required_semantic - set(semantic_features)
        ),
        "pending_license_sources": pending_sources,
        "schema_version": "0.1",
        "semantic_case_count": len(semantic_cases),
        "semantic_feature_counts": dict(sorted(semantic_features.items())),
        "semantic_statuses": dict(sorted(semantic_status.items())),
        "semantic_targets": dict(sorted(semantic_targets.items())),
        "statuses": dict(sorted(status.items())),
        "unpinned_sources": unpinned_sources,
    }


def write_report(root: Path, output: Path | None = None) -> dict[str, Any]:
    """Build the corpus report and optionally write it to the protected reports output directory."""
    report = build_report(root)
    if output is not None:
        output = _derived_output(root.resolve(), output, "reports")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(dump_json(report), encoding="utf-8")
    return report


def _packable_files(root: Path) -> Iterable[Path]:
    """Enumerate canonical files permitted in a deterministic ZIP."""
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        relative = path.relative_to(root)
        if relative.parts and relative.parts[0] in DERIVED_DIRECTORIES:
            continue
        yield path


def pack_corpus(root: Path, output: Path | None = None) -> tuple[Path, str]:
    """Validate and generate a canonical corpus ZIP with fixed timestamps and reproducible digests."""
    root = root.resolve()
    errors = lint_corpus(root)
    if errors:
        raise CorpusError("corpus lint failed:\n" + "\n".join(errors))
    version = (root / "VERSION").read_text(encoding="utf-8").strip()
    output = _derived_output(
        root,
        output or root / "dist" / f"openbridge-protocol-corpus-{version}.zip",
        "dist",
    )
    output.parent.mkdir(parents=True, exist_ok=True)

    entries = [
        {
            "path": path.relative_to(root).as_posix(),
            "sha256": sha256_file(path),
            "size": path.stat().st_size,
        }
        for path in _packable_files(root)
    ]
    manifest = {
        "corpus_version": version,
        "files": entries,
        "packager_version": __version__,
        "schema_version": "0.1",
    }

    with zipfile.ZipFile(
        output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
    ) as archive:
        for path in _packable_files(root):
            relative = path.relative_to(root).as_posix()
            info = zipfile.ZipInfo(relative, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, path.read_bytes(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
        manifest_info = zipfile.ZipInfo(
            "manifest.json", date_time=(1980, 1, 1, 0, 0, 0)
        )
        manifest_info.compress_type = zipfile.ZIP_DEFLATED
        manifest_info.external_attr = 0o100644 << 16
        archive.writestr(
            manifest_info,
            dump_json(manifest).encode("utf-8"),
            compress_type=zipfile.ZIP_DEFLATED,
            compresslevel=9,
        )
    digest = sha256_file(output)
    output.with_suffix(output.suffix + ".sha256").write_text(
        f"{digest}  {output.name}\n", encoding="ascii"
    )
    return output, digest
