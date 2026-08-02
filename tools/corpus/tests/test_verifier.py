"""验证 canonical case 对单次 client/server observation 的判定行为。"""

from __future__ import annotations

import base64
import json
from pathlib import Path

import pytest

from openbridge_corpus.cli import main
from openbridge_corpus.corpuslib import CorpusError, sha256_bytes
from openbridge_corpus.sse import parse_sse
from openbridge_corpus.verifier import verify_case_observations


CORPUS_ROOT = Path(__file__).parents[3] / "testdata"


def _matching_observations(case_id: str) -> tuple[dict, dict]:
    """根据 canonical artifact 构造一对应通过校验的脱敏 observation。"""
    case_path = next(CORPUS_ROOT.rglob(f"{case_id}/case.json"))
    case = json.loads(case_path.read_text(encoding="utf-8"))
    expected_client = (
        case_path.parent / case["artifacts"]["expected_client_response"]
    ).read_bytes()
    expected_upstream_bytes = (
        case_path.parent / case["artifacts"]["expected_upstream_request"]
    ).read_bytes()
    expected_upstream = json.loads(expected_upstream_bytes)
    client = {
        "body_base64": base64.b64encode(expected_client).decode("ascii"),
        "body_json": json.loads(expected_client),
        "body_sha256": sha256_bytes(expected_client),
        "case_id": case_id,
        "end": "response",
        "error": None,
        "response": {
            "headers": [["content-type", "application/json"]],
            "status": 200,
            "terminal_kinds": [],
        },
        "role": "mock_client",
        "schema_version": "0.1",
    }
    server = {
        "body_base64": base64.b64encode(expected_upstream_bytes).decode("ascii"),
        "body_json": expected_upstream,
        "body_sha256": sha256_bytes(expected_upstream_bytes),
        "case_id": case_id,
        "end": "response",
        "error": None,
        "request": {
            "headers": [],
            "method": "POST",
            "target": "/v1/responses",
        },
        "response": {"status": 200, "terminal_kinds": []},
        "role": "mock_server",
        "schema_version": "0.1",
    }
    return client, server


def test_matching_client_and_server_observations_pass() -> None:
    """验证匹配的 client/server observation 通过确定性比较。"""
    client, server = _matching_observations("responses_native.text.non_stream")

    assert verify_case_observations(
        CORPUS_ROOT,
        "responses_native.text.non_stream",
        client_observation=client,
        server_observation=server,
    ) == []


def test_mismatches_report_safe_field_paths() -> None:
    """验证差异只报告安全字段路径，不回显私有正文。"""
    client, server = _matching_observations("responses_native.text.non_stream")
    client["body_json"]["output"][0]["content"][0]["text"] = "private output"
    client["body_json"]["private field name"] = "private value"
    server["request"]["target"] = "/v1/chat/completions"

    errors = verify_case_observations(
        CORPUS_ROOT,
        "responses_native.text.non_stream",
        client_observation=client,
        server_observation=server,
    )

    assert "client.body_json.output[0].content[0].text differs" in errors
    assert "client.body_json has unexpected field(s)" in errors
    assert "server.request.target differs" in errors
    assert all("private" not in error for error in errors)


def test_reject_case_does_not_require_server_observation() -> None:
    """验证 reject case 不执行上游请求时无需 server observation。"""
    case_id = "responses_to_chat.continuation.reject"
    case_path = next(CORPUS_ROOT.rglob(f"{case_id}/case.json"))
    case = json.loads(case_path.read_text(encoding="utf-8"))
    expected = (
        case_path.parent / case["artifacts"]["expected_client_response"]
    ).read_bytes()
    client = {
        "body_base64": base64.b64encode(expected).decode("ascii"),
        "body_json": json.loads(expected),
        "body_sha256": sha256_bytes(expected),
        "case_id": case_id,
        "end": "error_response",
        "error": None,
        "response": {"headers": [], "status": 400, "terminal_kinds": []},
        "role": "mock_client",
        "schema_version": "0.1",
    }

    assert verify_case_observations(
        CORPUS_ROOT,
        case_id,
        client_observation=client,
        server_observation=None,
    ) == []


def test_upstream_case_requires_server_observation() -> None:
    """验证发生一次上游 attempt 的 case 必须提供 server observation。"""
    client, _ = _matching_observations("responses_native.text.non_stream")

    errors = verify_case_observations(
        CORPUS_ROOT,
        "responses_native.text.non_stream",
        client_observation=client,
        server_observation=None,
    )

    assert errors == ["server observation is required for one upstream attempt"]


def test_stream_observations_compare_exact_wire_and_terminal() -> None:
    """验证流式 observation 同时比较原始 wire hash 和 terminal 列表。"""
    case_id = "responses_native.sse_framing"
    case_path = next(CORPUS_ROOT.rglob(f"{case_id}/case.json"))
    case = json.loads(case_path.read_text(encoding="utf-8"))
    client_wire = (
        case_path.parent / case["artifacts"]["expected_client_stream"]
    ).read_bytes()
    upstream_request = (
        case_path.parent / case["artifacts"]["expected_upstream_request"]
    ).read_bytes()
    client = {
        "body_base64": base64.b64encode(client_wire).decode("ascii"),
        "body_json": None,
        "body_sha256": sha256_bytes(client_wire),
        "case_id": case_id,
        "end": "terminal",
        "error": None,
        "response": {
            "headers": [["content-type", "text/event-stream"]],
            "status": 200,
            "terminal_kinds": [
                event.terminal for event in parse_sse(client_wire) if event.terminal
            ],
        },
        "role": "mock_client",
        "schema_version": "0.1",
    }
    server = {
        "body_base64": base64.b64encode(upstream_request).decode("ascii"),
        "body_json": json.loads(upstream_request),
        "body_sha256": sha256_bytes(upstream_request),
        "case_id": case_id,
        "end": "terminal",
        "error": None,
        "request": {"headers": [], "method": "POST", "target": "/v1/responses"},
        "response": {"status": 200, "terminal_kinds": ["response_completed"]},
        "role": "mock_server",
        "schema_version": "0.1",
    }

    assert verify_case_observations(
        CORPUS_ROOT,
        case_id,
        client_observation=client,
        server_observation=server,
    ) == []


def test_http_error_observations_require_declared_response_headers() -> None:
    """验证 HTTP error 的 response headers 必须与 canonical 声明一致。"""
    case_id = "responses_native.rate_limit.non_stream"
    client, server = _matching_observations(case_id)
    client["end"] = "error_response"
    client["response"] = {
        "headers": [
            ["content-type", "application/json"],
            ["retry-after", "1"],
        ],
        "status": 429,
        "terminal_kinds": [],
    }
    server["end"] = "error_response"
    server["response"]["status"] = 429

    assert verify_case_observations(
        CORPUS_ROOT,
        case_id,
        client_observation=client,
        server_observation=server,
    ) == []

    client["response"]["headers"] = [["content-type", "application/json"]]
    errors = verify_case_observations(
        CORPUS_ROOT,
        case_id,
        client_observation=client,
        server_observation=server,
    )
    assert errors == ["client.response.headers.retry-after differs"]


def test_observation_hash_must_match_recorded_body() -> None:
    """验证 observation 的 body hash 必须匹配记录的 base64 body。"""
    client, server = _matching_observations("responses_native.text.non_stream")
    client["body_sha256"] = "0" * 64

    errors = verify_case_observations(
        CORPUS_ROOT,
        "responses_native.text.non_stream",
        client_observation=client,
        server_observation=server,
    )

    assert "client.body_sha256 does not match body_base64" in errors


def test_malformed_nested_observation_shape_is_a_corpus_error() -> None:
    """验证嵌套 observation 类型错误会形成 CorpusError。"""
    client, server = _matching_observations("responses_native.text.non_stream")
    client["response"] = []

    with pytest.raises(CorpusError, match="client.response must be an object"):
        verify_case_observations(
            CORPUS_ROOT,
            "responses_native.text.non_stream",
            client_observation=client,
            server_observation=server,
        )


def test_cli_reports_verdict_without_echoing_body(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """验证 CLI 只输出 verdict 和安全字段路径，不回显响应正文。"""
    case_id = "responses_native.text.non_stream"
    client, server = _matching_observations(case_id)
    client_path = tmp_path / "client.json"
    server_path = tmp_path / "server.json"
    client_path.write_text(json.dumps(client), encoding="utf-8")
    server_path.write_text(json.dumps(server), encoding="utf-8")

    exit_code = main(
        [
            "--root",
            str(CORPUS_ROOT),
            "verify-observations",
            "--case",
            case_id,
            "--client-observation",
            str(client_path),
            "--server-observation",
            str(server_path),
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 0
    assert captured.out == f"{case_id}: observations passed\n"
    assert captured.err == ""

    client["body_json"]["output"][0]["content"][0]["text"] = "private output"
    client_path.write_text(json.dumps(client), encoding="utf-8")
    exit_code = main(
        [
            "--root",
            str(CORPUS_ROOT),
            "verify-observations",
            "--case",
            case_id,
            "--client-observation",
            str(client_path),
            "--server-observation",
            str(server_path),
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 1
    assert captured.out == ""
    assert "client.body_json.output[0].content[0].text differs" in captured.err
    assert "private output" not in captured.err
