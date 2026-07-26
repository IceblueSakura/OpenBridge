from __future__ import annotations

import base64
from pathlib import Path
from typing import Any
from urllib.parse import urljoin

from jsonschema import Draft202012Validator, FormatChecker

from .corpuslib import (
    Case,
    CorpusError,
    discover_cases,
    load_json,
    sha256_bytes,
)


def validate_runtime_document(
    root: Path, schema_name: str, document: dict[str, Any]
) -> None:
    schema_path = root / "schemas" / f"{schema_name}.schema.json"
    schema = load_json(schema_path)
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    errors = sorted(validator.iter_errors(document), key=lambda item: list(item.path))
    if errors:
        error = errors[0]
        location = ".".join(str(part) for part in error.path) or "(root)"
        raise CorpusError(
            f"{schema_name} document is invalid at {location}: {error.message}"
        )


def find_case(root: Path, case_id: str) -> Case:
    for case in discover_cases(root):
        if case.case_id == case_id:
            return case
    raise CorpusError(f"unknown case id: {case_id}")


def _protocol_path(direction: str, side: str) -> str:
    if side == "client":
        chat = direction in {"chat_native", "chat_to_responses"}
    elif side == "upstream":
        chat = direction in {"chat_native", "responses_to_chat"}
    else:
        raise ValueError(f"unknown protocol side: {side}")
    return "/v1/chat/completions" if chat else "/v1/responses"


def _artifact_bytes(case: Case, artifact_name: str) -> bytes:
    relative = case.data["artifacts"].get(artifact_name)
    if not relative:
        raise CorpusError(f"{case.case_id}: missing artifact {artifact_name}")
    path = (case.directory / relative).resolve()
    if case.directory.resolve() not in path.parents:
        raise CorpusError(f"{case.case_id}: artifact escapes case directory")
    return path.read_bytes()


def _variant_chunks(
    root: Path,
    case: Case,
    artifact_name: str,
    variant: str,
) -> tuple[list[str], str]:
    canonical = _artifact_bytes(case, artifact_name)
    if variant == "canonical":
        return [base64.b64encode(canonical).decode("ascii")], sha256_bytes(canonical)
    candidates = sorted(
        (root / "generated" / case.case_id).glob(
            f"{artifact_name}.*.{variant}.json"
        )
    )
    if len(candidates) != 1:
        raise CorpusError(
            f"{case.case_id}: expected one generated {variant!r} variant for "
            f"{artifact_name}, found {len(candidates)}; run corpus generate first"
        )
    payload = load_json(candidates[0])
    return payload["chunks_base64"], payload["wire_sha256"]


def build_server_scenario(
    root: Path,
    case_id: str,
    *,
    variant: str = "canonical",
    chunk_delay_ms: int = 0,
    abort_delay_ms: int = 10,
) -> dict[str, Any]:
    case = find_case(root, case_id)
    artifacts = case.data["artifacts"]
    if "expected_upstream_request" not in artifacts:
        raise CorpusError(f"{case_id}: case does not make an upstream request")
    if "upstream_stream" in artifacts:
        response_artifact = "upstream_stream"
    elif "upstream_response" in artifacts:
        response_artifact = "upstream_response"
    else:
        raise CorpusError(f"{case_id}: missing upstream response artifact")
    request = _artifact_bytes(case, "expected_upstream_request")
    request_json = load_json(case.directory / artifacts["expected_upstream_request"])
    chunks, wire_sha256 = _variant_chunks(
        root, case, response_artifact, variant
    )
    transport = case.data.get("transport", {})
    content_type = transport.get(
        "upstream_content_type",
        "text/event-stream" if case.data["stream"] else "application/json",
    )
    upstream_end = transport.get("upstream_end", "terminal")
    response_headers = [["content-type", content_type]]
    response_headers.extend(transport.get("upstream_headers", []))
    scenario = {
        "case_id": case_id,
        "expected_request": {
            "body_json": request_json,
            "body_sha256": sha256_bytes(request),
            "method": "POST",
            "path": _protocol_path(case.data["direction"], "upstream"),
        },
        "response": {
            "abort_delay_ms": abort_delay_ms,
            "chunk_delay_ms": chunk_delay_ms,
            "chunks_base64": chunks,
            "headers": response_headers,
            "status": transport.get("upstream_http_status", 200),
            "termination": (
                "abort" if upstream_end == "transport_error" else "complete"
            ),
            "wire_sha256": wire_sha256,
        },
        "schema_version": "0.1",
        "variant": variant,
    }
    validate_runtime_document(root, "server-scenario", scenario)
    return scenario


def build_client_plan(
    root: Path,
    case_id: str,
    *,
    base_url: str,
    timeout_ms: int = 5000,
) -> dict[str, Any]:
    case = find_case(root, case_id)
    body = _artifact_bytes(case, "client_request")
    path = _protocol_path(case.data["direction"], "client")
    transport = case.data.get("transport", {})
    plan = {
        "body_base64": base64.b64encode(body).decode("ascii"),
        "body_sha256": sha256_bytes(body),
        "cancel_after_event": transport.get("cancellation_after_event"),
        "case_id": case_id,
        "headers": [["content-type", "application/json"]],
        "method": "POST",
        "schema_version": "0.1",
        "stream": case.data["stream"],
        "timeout_ms": timeout_ms,
        "url": urljoin(base_url.rstrip("/") + "/", path.lstrip("/")),
    }
    validate_runtime_document(root, "client-plan", plan)
    return plan


def build_server_suite(
    root: Path,
    case_ids: list[str],
    *,
    variant: str = "canonical",
    chunk_delay_ms: int = 0,
    abort_delay_ms: int = 10,
    suite_id: str = "server-suite",
) -> dict[str, Any]:
    if not case_ids:
        raise CorpusError("server suite requires at least one case")
    exchanges = [
        build_server_scenario(
            root,
            case_id,
            variant=variant,
            chunk_delay_ms=chunk_delay_ms,
            abort_delay_ms=abort_delay_ms,
        )
        for case_id in case_ids
    ]
    suite = {
        "exchanges": exchanges,
        "schema_version": "0.1",
        "suite_id": suite_id,
    }
    validate_runtime_document(root, "server-suite", suite)
    return suite
