from __future__ import annotations

import asyncio
import base64
import shutil
from pathlib import Path

import pytest

from openbridge_corpus.corpuslib import CorpusError, generate_variants, sha256_bytes
from openbridge_corpus.mockclient import run_mock_client
from openbridge_corpus.mockserver import MockServer
from openbridge_corpus.plans import (
    build_client_plan,
    build_server_scenario,
    build_server_suite,
    validate_runtime_document,
)


CORPUS_ROOT = Path(__file__).parents[3] / "testdata"


@pytest.fixture(scope="module")
def generated_root(tmp_path_factory: pytest.TempPathFactory) -> Path:
    root = tmp_path_factory.mktemp("testkit") / "testdata"
    shutil.copytree(
        CORPUS_ROOT,
        root,
        ignore=shutil.ignore_patterns("generated", "reports", "dist", "runtime"),
    )
    generate_variants(root, seed=20260726)
    return root


def test_case_compilers_produce_self_contained_valid_documents(
    generated_root: Path,
) -> None:
    scenario = build_server_scenario(
        generated_root,
        "responses_native.sse_framing",
        variant="event_pairs",
        chunk_delay_ms=3,
    )
    plan = build_client_plan(
        generated_root,
        "responses_native.sse_framing",
        base_url="http://127.0.0.1:9000",
    )
    validate_runtime_document(generated_root, "server-scenario", scenario)
    validate_runtime_document(generated_root, "client-plan", plan)
    assert scenario["variant"] == "event_pairs"
    assert len(scenario["response"]["chunks_base64"]) >= 2
    assert plan["url"] == "http://127.0.0.1:9000/v1/responses"


def test_bridge_direction_compiles_distinct_client_and_upstream_paths(
    generated_root: Path,
) -> None:
    responses_to_chat_server = build_server_scenario(
        generated_root,
        "responses_to_chat.text.non_stream",
    )
    responses_to_chat_client = build_client_plan(
        generated_root,
        "responses_to_chat.text.non_stream",
        base_url="http://127.0.0.1:9000",
    )
    chat_to_responses_server = build_server_scenario(
        generated_root,
        "chat_to_responses.text.non_stream",
    )
    chat_to_responses_client = build_client_plan(
        generated_root,
        "chat_to_responses.text.non_stream",
        base_url="http://127.0.0.1:9000",
    )
    assert responses_to_chat_server["expected_request"]["path"] == (
        "/v1/chat/completions"
    )
    assert responses_to_chat_client["url"].endswith("/v1/responses")
    assert chat_to_responses_server["expected_request"]["path"] == "/v1/responses"
    assert chat_to_responses_client["url"].endswith("/v1/chat/completions")


def test_reject_case_cannot_compile_an_upstream_scenario(
    generated_root: Path,
) -> None:
    with pytest.raises(CorpusError, match="does not make an upstream request"):
        build_server_scenario(
            generated_root,
            "responses_to_chat.empty_arguments.reject",
        )


def test_mock_server_client_loopback_streams_and_records_both_sides(
    generated_root: Path,
) -> None:
    async def exercise() -> None:
        scenario = build_server_scenario(
            generated_root,
            "responses_native.sse_framing",
            variant="one_byte",
        )
        server = MockServer(scenario)
        port = await server.start()
        try:
            plan = build_client_plan(
                generated_root,
                "responses_native.sse_framing",
                base_url=f"http://127.0.0.1:{port}",
            )
            plan["headers"].append(
                ["authorization", "Bearer local-test-token"]
            )
            client = await run_mock_client(plan)
            server_observation = await server.wait()
        finally:
            await server.close()
        validate_runtime_document(generated_root, "observation", client)
        validate_runtime_document(
            generated_root, "observation", server_observation
        )
        assert client["end"] == "terminal"
        assert client["response"]["terminal_kinds"] == ["response_completed"]
        assert any(event["data_text"] for event in client["events"])
        assert server_observation["request"]["target"] == "/v1/responses"
        assert ["authorization", "<redacted>"] in server_observation["request"][
            "headers"
        ]
        assert (
            server_observation["body_sha256"]
            == plan["body_sha256"]
        )

    asyncio.run(exercise())


def test_mock_server_rejects_tampered_wire_hash(generated_root: Path) -> None:
    scenario = build_server_scenario(
        generated_root,
        "responses_native.sse_framing",
    )
    scenario["response"]["wire_sha256"] = "0" * 64
    with pytest.raises(CorpusError, match="wire_sha256"):
        MockServer(scenario)


def test_mock_server_health_invalid_json_and_unknown_endpoint_do_not_consume_exchange(
    generated_root: Path,
) -> None:
    async def exercise() -> None:
        scenario = build_server_scenario(
            generated_root,
            "responses_native.text.non_stream",
        )
        server = MockServer(scenario)
        port = await server.start()
        base_url = f"http://127.0.0.1:{port}"
        try:
            valid = build_client_plan(
                generated_root,
                "responses_native.text.non_stream",
                base_url=base_url,
            )
            health = dict(valid)
            health.update(
                {
                    "body_base64": "",
                    "body_sha256": sha256_bytes(b""),
                    "case_id": "mock.health",
                    "method": "GET",
                    "url": f"{base_url}/healthz",
                }
            )
            invalid = dict(valid)
            invalid_body = b"{"
            invalid.update(
                {
                    "body_base64": base64.b64encode(invalid_body).decode("ascii"),
                    "body_sha256": sha256_bytes(invalid_body),
                    "case_id": "mock.invalid_json",
                }
            )
            unknown = dict(valid)
            unknown.update(
                {
                    "case_id": "mock.unknown_endpoint",
                    "url": f"{base_url}/v1/unknown",
                }
            )
            health_result = await run_mock_client(health)
            invalid_result = await run_mock_client(invalid)
            unknown_result = await run_mock_client(unknown)
            valid_result = await run_mock_client(valid)
            observations = await server.wait_all()
        finally:
            await server.close()
        assert health_result["response"]["status"] == 200
        assert health_result["body_json"]["status"] == "ok"
        assert invalid_result["response"]["status"] == 400
        assert invalid_result["body_json"]["error"]["code"] == "invalid_json"
        assert unknown_result["response"]["status"] == 404
        assert valid_result["response"]["status"] == 200
        assert len(observations) == 1

    asyncio.run(exercise())


def test_mock_server_suite_handles_multiple_protocols_and_rate_limit_headers(
    generated_root: Path,
) -> None:
    async def exercise() -> None:
        suite = build_server_suite(
            generated_root,
            [
                "chat_native.text.non_stream",
                "responses_native.rate_limit.non_stream",
            ],
            suite_id="rust-mock-parity",
        )
        server = MockServer(suite)
        port = await server.start()
        base_url = f"http://127.0.0.1:{port}"
        try:
            chat_plan = build_client_plan(
                generated_root,
                "chat_native.text.non_stream",
                base_url=base_url,
            )
            responses_plan = build_client_plan(
                generated_root,
                "responses_native.rate_limit.non_stream",
                base_url=base_url,
            )
            chat_result = await run_mock_client(chat_plan)
            rate_limit_result = await run_mock_client(responses_plan)
            observations = await server.wait_all()
        finally:
            await server.close()
        assert chat_result["end"] == "response"
        assert rate_limit_result["end"] == "error_response"
        assert rate_limit_result["response"]["status"] == 429
        assert ["retry-after", "1"] in rate_limit_result["response"]["headers"]
        assert [item["case_id"] for item in observations] == [
            "chat_native.text.non_stream",
            "responses_native.rate_limit.non_stream",
        ]

    asyncio.run(exercise())


def test_mock_client_observes_http_error_response(
    generated_root: Path,
) -> None:
    async def exercise() -> None:
        scenario = build_server_scenario(
            generated_root,
            "responses_native.transport_error.before_output",
        )
        server = MockServer(scenario)
        port = await server.start()
        try:
            plan = build_client_plan(
                generated_root,
                "responses_native.transport_error.before_output",
                base_url=f"http://127.0.0.1:{port}",
            )
            client = await run_mock_client(plan)
            await server.wait()
        finally:
            await server.close()
        assert client["end"] == "error_response"
        assert client["response"]["status"] == 503
        assert isinstance(client["body_json"], dict)

    asyncio.run(exercise())


def test_mock_client_distinguishes_transport_abort_after_output(
    generated_root: Path,
) -> None:
    async def exercise() -> None:
        scenario = build_server_scenario(
            generated_root,
            "responses_native.transport_error.after_output",
            variant="event_pairs",
        )
        server = MockServer(scenario)
        port = await server.start()
        try:
            plan = build_client_plan(
                generated_root,
                "responses_native.transport_error.after_output",
                base_url=f"http://127.0.0.1:{port}",
            )
            client = await run_mock_client(plan)
            await server.wait()
        finally:
            await server.close()
        assert base64.b64decode(client["body_base64"])
        assert client["end"] == "transport_error"
        assert client["response"]["terminal_kinds"] == []

    asyncio.run(exercise())


def test_mock_client_can_cancel_after_logical_event(
    generated_root: Path,
) -> None:
    async def exercise() -> None:
        scenario = build_server_scenario(
            generated_root,
            "responses_native.cancel.after_output",
            variant="one_byte",
            chunk_delay_ms=1,
        )
        server = MockServer(scenario)
        port = await server.start()
        try:
            plan = build_client_plan(
                generated_root,
                "responses_native.cancel.after_output",
                base_url=f"http://127.0.0.1:{port}",
            )
            client = await run_mock_client(plan)
            await server.wait()
        finally:
            await server.close()
        assert client["end"] == "cancelled"
        assert len(client["events"]) == plan["cancel_after_event"]

    asyncio.run(exercise())
