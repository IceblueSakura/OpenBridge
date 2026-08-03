"""Deterministically compare one canonical case with redacted Mock Client/Server observations."""

from __future__ import annotations

import base64
import binascii
import json
from pathlib import Path
from typing import Any

from .corpuslib import Case, CorpusError, load_json, sha256_bytes
from .plans import find_case, validate_runtime_document


def _artifact_path(case: Case, name: str) -> Path:
    """Resolve a case artifact path and reject paths that escape the case directory."""
    relative = case.data["artifacts"].get(name)
    if not relative:
        raise CorpusError(f"{case.case_id}: missing artifact {name}")
    path = (case.directory / relative).resolve()
    if case.directory.resolve() not in path.parents:
        raise CorpusError(f"{case.case_id}: artifact escapes case directory")
    return path


def _compare_json(
    expected: Any,
    actual: Any,
    path: str,
    errors: list[str],
) -> None:
    """Recursively compare JSON values and report field paths without echoing sensitive content."""
    if type(expected) is not type(actual):
        errors.append(f"{path} has a different JSON type")
        return
    if isinstance(expected, dict):
        for key in sorted(expected.keys() - actual.keys()):
            errors.append(f"{path}.{key} is missing")
        if actual.keys() - expected.keys():
            errors.append(f"{path} has unexpected field(s)")
        for key in sorted(expected.keys() & actual.keys()):
            _compare_json(expected[key], actual[key], f"{path}.{key}", errors)
        return
    if isinstance(expected, list):
        if len(expected) != len(actual):
            errors.append(f"{path} has a different item count")
        for index, (expected_item, actual_item) in enumerate(zip(expected, actual)):
            _compare_json(
                expected_item,
                actual_item,
                f"{path}[{index}]",
                errors,
            )
        return
    if expected != actual:
        errors.append(f"{path} differs")


def _decode_observation_body(
    role: str,
    observation: dict[str, Any],
    errors: list[str],
) -> tuple[bytes | None, Any]:
    """Validate an observation body's Base64, digest, and optional JSON projection."""
    # Decode the raw body and first verify that the observation's own digest is trustworthy.
    try:
        body = base64.b64decode(observation["body_base64"], validate=True)
    except (binascii.Error, ValueError, TypeError):
        errors.append(f"{role}.body_base64 is invalid")
        return None, None
    if sha256_bytes(body) != observation["body_sha256"]:
        errors.append(f"{role}.body_sha256 does not match body_base64")

    # Parse the raw JSON and verify that the observation's JSON projection was not altered independently.
    parsed: Any = None
    try:
        parsed = json.loads(body)
    except (json.JSONDecodeError, UnicodeError):
        pass
    projected = observation.get("body_json")
    if projected is not None:
        if parsed is None:
            errors.append(f"{role}.body_json is present for a non-JSON body")
        else:
            _compare_json(parsed, projected, f"{role}.body_json", errors)
    return body, parsed


def _required_object(
    observation: dict[str, Any],
    field: str,
    path: str,
) -> dict[str, Any]:
    """Read nested objects required by the verifier and convert shape errors to CorpusError."""
    value = observation.get(field)
    if not isinstance(value, dict):
        raise CorpusError(f"{path} must be an object")
    return value


def _header_values(response: dict[str, Any]) -> dict[str, list[str]]:
    """Collect response-observation header values by lowercase name."""
    result: dict[str, list[str]] = {}
    for item in response.get("headers", []):
        if not isinstance(item, list) or len(item) != 2:
            continue
        name, value = item
        if isinstance(name, str) and isinstance(value, str):
            result.setdefault(name.lower(), []).append(value)
    return result


def _verify_identity(
    case: Case,
    role: str,
    observation: dict[str, Any],
    errors: list[str],
) -> None:
    """Validate observation role and case identity."""
    if observation.get("role") != role:
        errors.append(f"{role}.role differs")
    if observation.get("case_id") != case.case_id:
        errors.append(f"{role}.case_id differs")


def _verify_client(
    case: Case,
    observation: dict[str, Any],
    errors: list[str],
) -> None:
    """Validate downstream body, transport results, and terminal observations."""
    # Validate observation consistency and select the JSON or raw-wire oracle.
    body, body_json = _decode_observation_body("client", observation, errors)
    artifacts = case.data["artifacts"]
    if "expected_client_stream" in artifacts:
        expected_body = _artifact_path(case, "expected_client_stream").read_bytes()
        if body is not None and body != expected_body:
            errors.append("client.body_sha256 differs from expected stream")
    elif "expected_client_response" in artifacts:
        expected_path = _artifact_path(case, "expected_client_response")
        if expected_path.suffix == ".json":
            expected_json = load_json(expected_path)
            _compare_json(expected_json, body_json, "client.body_json", errors)
        elif body is not None and body != expected_path.read_bytes():
            errors.append("client.body_sha256 differs from expected response")
    else:
        raise CorpusError(f"{case.case_id}: missing expected client artifact")

    # Check the case-declared HTTP and completion classifications without inferring missing transport metadata.
    transport = case.data.get("transport")
    response = _required_object(observation, "response", "client.response")
    if transport is not None:
        if observation.get("end") != transport["client_end"]:
            errors.append("client.end differs")
        if response.get("status") != transport["client_http_status"]:
            errors.append("client.response.status differs")
        headers = _header_values(response)
        expected_headers = [
            ["content-type", transport["client_content_type"]],
            *transport.get("client_headers", []),
        ]
        for name, value in expected_headers:
            if value not in headers.get(name.lower(), []):
                errors.append(f"client.response.headers.{name.lower()} differs")

    # Check terminal identity and count so item events cannot be mistaken for response terminals.
    terminal_kinds = response.get("terminal_kinds", [])
    expectation = case.data["expectation"]
    expected_terminal = expectation["terminal"]
    expected_terminals = (
        []
        if expected_terminal == "none"
        else [expected_terminal] * expectation["terminal_count"]
    )
    if terminal_kinds != expected_terminals:
        errors.append("client.response.terminal_kinds differs")
    if len(terminal_kinds) != expectation["terminal_count"]:
        errors.append("client.response.terminal_kinds has a different item count")


def _expected_upstream_path(direction: str) -> str:
    """Return the upstream endpoint path for the case direction."""
    if direction in {"chat_native", "responses_to_chat"}:
        return "/v1/chat/completions"
    return "/v1/responses"


def _verify_server(
    case: Case,
    observation: dict[str, Any],
    errors: list[str],
) -> None:
    """Validate one upstream request, response status, and completion classification."""
    # Validate the JSON semantics of the observation body against the canonical upstream request.
    _, body_json = _decode_observation_body("server", observation, errors)
    expected_json = load_json(_artifact_path(case, "expected_upstream_request"))
    _compare_json(expected_json, body_json, "server.body_json", errors)

    # Check the request endpoint and transport result of the single fixture response.
    request = _required_object(observation, "request", "server.request")
    response = _required_object(observation, "response", "server.response")
    if request.get("method") != "POST":
        errors.append("server.request.method differs")
    if request.get("target") != _expected_upstream_path(case.data["direction"]):
        errors.append("server.request.target differs")
    transport = case.data.get("transport")
    if transport is not None:
        if observation.get("end") != transport["upstream_end"]:
            errors.append("server.end differs")
        if response.get("status") != transport["upstream_http_status"]:
            errors.append("server.response.status differs")


def verify_case_observations(
    root: Path,
    case_id: str,
    *,
    client_observation: dict[str, Any],
    server_observation: dict[str, Any] | None,
) -> list[str]:
    """Compare observations for one case and return stable errors without bodies.

    Raise ``CorpusError`` when the input fails the observation schema, the case is missing, or the case
    declares more than one upstream attempt. An empty list means all values match within this boundary.
    """
    # Load the case and first reject structurally invalid observations.
    case = find_case(root, case_id)
    validate_runtime_document(root, "observation", client_observation)
    errors: list[str] = []
    _verify_identity(case, "mock_client", client_observation, errors)
    _verify_client(case, client_observation, errors)

    # Check the zero or one upstream attempt declared by the case without inferring retry/fallback sequences.
    attempts = case.data["expectation"]["upstream_attempts"]
    if attempts == 0:
        if server_observation is not None:
            errors.append("server observation is unexpected for zero upstream attempts")
    elif attempts == 1:
        if server_observation is None:
            errors.append("server observation is required for one upstream attempt")
        else:
            validate_runtime_document(root, "observation", server_observation)
            _verify_identity(case, "mock_server", server_observation, errors)
            _verify_server(case, server_observation, errors)
    else:
        raise CorpusError(
            f"{case.case_id}: single-case verifier supports at most one upstream attempt"
        )

    # Deduplicate while preserving discovery order for stable CLI and test consumption.
    return list(dict.fromkeys(errors))
