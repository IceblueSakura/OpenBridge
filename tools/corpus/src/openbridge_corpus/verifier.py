"""将单个 canonical case 与脱敏的 Mock Client/Server observation 进行确定性比较。"""

from __future__ import annotations

import base64
import binascii
import json
from pathlib import Path
from typing import Any

from .corpuslib import Case, CorpusError, load_json, sha256_bytes
from .plans import find_case, validate_runtime_document


def _artifact_path(case: Case, name: str) -> Path:
    """解析 case artifact 路径，并拒绝逃出 case 目录。"""
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
    """递归比较 JSON 值，只报告字段路径而不回显可能敏感的内容。"""
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
    """校验 observation body 的 Base64、摘要和可选 JSON 投影。"""
    # 解码原始 body，并先确认 observation 自身的摘要可信。
    try:
        body = base64.b64decode(observation["body_base64"], validate=True)
    except (binascii.Error, ValueError, TypeError):
        errors.append(f"{role}.body_base64 is invalid")
        return None, None
    if sha256_bytes(body) != observation["body_sha256"]:
        errors.append(f"{role}.body_sha256 does not match body_base64")

    # 解析原始 JSON，并核对 observation 提供的 JSON 投影未被独立篡改。
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
    """读取 verifier 必需的嵌套对象，并把形状错误转换为 CorpusError。"""
    value = observation.get(field)
    if not isinstance(value, dict):
        raise CorpusError(f"{path} must be an object")
    return value


def _header_values(response: dict[str, Any]) -> dict[str, list[str]]:
    """按小写名称收集 response observation 中的 header 值。"""
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
    """验证 observation 的角色与 case identity。"""
    if observation.get("role") != role:
        errors.append(f"{role}.role differs")
    if observation.get("case_id") != case.case_id:
        errors.append(f"{role}.case_id differs")


def _verify_client(
    case: Case,
    observation: dict[str, Any],
    errors: list[str],
) -> None:
    """验证下游 body、transport 结果和 terminal 观察。"""
    # 校验 observation 自洽性并选择 JSON 或原始 wire oracle。
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

    # 核对 case 声明的 HTTP 与结束分类，但不推断缺失的 transport 元数据。
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

    # 核对 terminal identity 与数量，避免 item 事件被误当成 response terminal。
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
    """返回 case direction 对应的上游 endpoint path。"""
    if direction in {"chat_native", "responses_to_chat"}:
        return "/v1/chat/completions"
    return "/v1/responses"


def _verify_server(
    case: Case,
    observation: dict[str, Any],
    errors: list[str],
) -> None:
    """验证单次上游请求、响应状态和结束分类。"""
    # 校验 observation body 与 canonical 上游请求的 JSON 语义。
    _, body_json = _decode_observation_body("server", observation, errors)
    expected_json = load_json(_artifact_path(case, "expected_upstream_request"))
    _compare_json(expected_json, body_json, "server.body_json", errors)

    # 核对请求 endpoint 与单次 fixture response 的 transport 结果。
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
    """比较单个 case 的 observations，返回不含正文的稳定错误列表。

    输入文档不符合 observation schema、case 不存在或当前 case 声明超过一个上游 attempt 时抛出
    ``CorpusError``。空列表表示当前判定边界内全部匹配。
    """
    # 加载 case，并先拒绝结构不合法的 observation。
    case = find_case(root, case_id)
    validate_runtime_document(root, "observation", client_observation)
    errors: list[str] = []
    _verify_identity(case, "mock_client", client_observation, errors)
    _verify_client(case, client_observation, errors)

    # 按 case 声明核对零次或单次上游 attempt，不推断 retry/fallback 序列。
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

    # 去重并保留确定的发现顺序，便于 CLI 和测试稳定消费。
    return list(dict.fromkeys(errors))
