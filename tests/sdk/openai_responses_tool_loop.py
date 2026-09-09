# /// script
# requires-python = ">=3.10"
# dependencies = ["openai==3.10.0"]
# ///
"""Run a two-turn OpenAI Responses tool loop against a fixed loopback base URL."""

from __future__ import annotations

import json
import ipaddress
from urllib.parse import urlsplit
import sys
from typing import Any

import openai
from openai import OpenAI

EXPECTED_ARGUMENTS = '{"location":"Shanghai"}'
EXPECTED_TOOL_OUTPUT = '{"condition":"sunny","temperature_c":25}'
EXPECTED_TEXT = "Shanghai is sunny at 25C."

TOOLS = [
    {
        "type": "function",
        "name": "get_weather",
        "description": "Return synthetic weather for a city.",
        "parameters": {
            "type": "object",
            "properties": {"location": {"type": "string"}},
            "required": ["location"],
            "additionalProperties": False,
        },
        "strict": True,
    }
]


def model_dict(value: Any) -> dict[str, Any]:
    """Convert an SDK response item back to the complete wire-shaped history item."""
    return value.model_dump(exclude_none=True)


def client_for(base_url: str) -> OpenAI:
    """Use only the supplied loopback origin and disable proxy/environment discovery."""
    parsed = urlsplit(base_url)
    if (parsed.scheme != "http" or not parsed.hostname
            or not ipaddress.ip_address(parsed.hostname).is_loopback
            or not parsed.port or parsed.username or parsed.password
            or parsed.path != "/v1" or parsed.query or parsed.fragment):
        raise ValueError("SDK acceptance requires a literal loopback HTTP origin with /v1")
    return OpenAI(
        api_key="downstream-token-0000000000000000",
        base_url=base_url,
        max_retries=0,
        timeout=10.0,
        organization="",
        project="",
        http_client=openai.DefaultHttpxClient(trust_env=False, follow_redirects=False),
    )


def request_kwargs(history: list[dict[str, Any]]) -> dict[str, Any]:
    """Build a stateless request without previous-response or server-side storage state."""
    return {
        "model": "public-model",
        "input": history,
        "tools": TOOLS,
        "tool_choice": "required" if len(history) == 1 else "none",
        "store": False,
        "max_output_tokens": 64,
    }


def json_turn(client: OpenAI, history: list[dict[str, Any]]) -> tuple[Any, bool]:
    """Execute one JSON response and return it with a bounded schema observation."""
    response = client.responses.create(**request_kwargs(history), stream=False)
    return response, True


def sse_turn(client: OpenAI, history: list[dict[str, Any]]) -> tuple[Any, bool, int]:
    """Consume every SSE event, then obtain the SDK's assembled final response."""
    argument_deltas: list[str] = []
    text_deltas: list[str] = []
    completed = 0
    event_count = 0
    with client.responses.stream(**request_kwargs(history)) as stream:
        for event in stream:
            event_count += 1
            if event.type == "response.function_call_arguments.delta":
                argument_deltas.append(event.delta)
            elif event.type == "response.output_text.delta":
                text_deltas.append(event.delta)
            elif event.type == "response.completed":
                completed += 1
        response = stream.get_final_response()
    assert completed == 1, "SDK must consume exactly one completed terminal"
    expected = EXPECTED_ARGUMENTS if len(history) == 1 else EXPECTED_TEXT
    observed = argument_deltas if len(history) == 1 else text_deltas
    return response, "".join(observed) == expected, event_count


def first_turn(client: OpenAI, mode: str, history: list[dict[str, Any]]) -> tuple[Any, bool, int]:
    """Run the first turn and expose only safe event counters to the caller."""
    if mode == "json":
        response, _ = json_turn(client, history)
        return response, True, 0
    response, delta_seen, event_count = sse_turn(client, history)
    return response, delta_seen, event_count


def second_turn(client: OpenAI, mode: str, history: list[dict[str, Any]]) -> tuple[Any, bool, int]:
    """Run the final turn through the same JSON or SSE client path."""
    return first_turn(client, mode, history)


def synthetic_weather(location: str) -> dict[str, Any]:
    """Execute a deterministic local tool with no filesystem, network, or Provider access."""
    if location != "Shanghai":
        raise ValueError("unsupported synthetic location")
    return {"condition": "sunny", "temperature_c": 25}


def run(base_url: str, mode: str, ablation: str | None) -> dict[str, Any]:
    """Run the normal loop or one negative control without retaining business payloads."""
    if openai.__version__ != "3.10.0":
        raise RuntimeError("this acceptance gate requires openai==3.10.0")
    client = client_for(base_url)
    try:
        history: list[dict[str, Any]] = [
            {"role": "user", "content": "What is the weather in Shanghai?"}
        ]
        first, first_delta_seen, first_event_count = first_turn(client, mode, history)
        tool_calls = [item for item in first.output if item.type == "function_call"]
        if len(tool_calls) != 1:
            raise AssertionError("expected exactly one synthetic function call")
        call = tool_calls[0]
        if call.name != "get_weather" or call.arguments != EXPECTED_ARGUMENTS:
            raise AssertionError("synthetic function call identity changed")
        if call.call_id != "call_weather_shanghai":
            raise AssertionError("synthetic function call id changed")

        # Execute only the local synthetic tool after validating typed SDK arguments.
        arguments = json.loads(call.arguments)
        if arguments != {"location": "Shanghai"}:
            raise AssertionError("unexpected synthetic weather arguments")
        tool_result = synthetic_weather(arguments["location"])
        assert first.status == "completed" and first.usage.total_tokens == 10

        # Preserve all first-turn output items before appending the matching output item.
        history.extend(model_dict(item) for item in first.output)
        output_item: dict[str, Any] = {
            "type": "function_call_output",
            "output": json.dumps(tool_result, separators=(",", ":"))
            if ablation != "wrong-tool-result"
            else '{"condition":"rainy","temperature_c":0}',
        }
        if ablation != "missing-call-id":
            output_item["call_id"] = call.call_id
        history.append(output_item)

        try:
            final, final_delta_seen, final_event_count = second_turn(client, mode, history)
        except openai.APIStatusError as error:
            if ablation is None:
                raise
            status = getattr(error, "status_code", None)
            return {
                "ok": False,
                "expected_failure": True,
                "http_status": status,
                "error_code": getattr(error, "code", None),
            }
        if ablation is not None:
            raise AssertionError("negative control unexpectedly succeeded")

        output_text = final.output_text
        if final.status != "completed" or output_text != EXPECTED_TEXT:
            raise AssertionError("final Responses response did not preserve status/text")
        if (final.usage is None or final.usage.input_tokens != 15
                or final.usage.output_tokens != 7 or final.usage.total_tokens != 22):
            raise AssertionError("final Responses response did not preserve usage")
        if mode == "sse" and (not first_delta_seen or not final_delta_seen):
            raise AssertionError("SSE delta observations were incomplete")
        return {
            "ok": True,
            "expected_failure": False,
            "mode": mode,
            "turns": 2,
            "tool_calls": len(tool_calls),
            "final_status_completed": final.status == "completed",
            "final_output_text_matches": output_text == EXPECTED_TEXT,
            "usage_present": final.usage is not None,
            "first_event_count": first_event_count,
            "final_event_count": final_event_count,
        }
    finally:
        client.close()


def main() -> None:
    if len(sys.argv) not in (3, 4):
        raise SystemExit("usage: openai_responses_tool_loop.py BASE_URL json|sse [ablation]")
    base_url, mode = sys.argv[1:3]
    if mode not in {"json", "sse"}:
        raise SystemExit("mode must be json or sse")
    ablation = sys.argv[3] if len(sys.argv) == 4 else None
    if ablation not in {None, "missing-call-id", "wrong-tool-result"}:
        raise SystemExit("unknown ablation")
    try:
        result = run(base_url, mode, ablation)
    except Exception as error:
        print(json.dumps({"error_type": type(error).__name__, "status": getattr(error, "status_code", None)}), file=sys.stderr)
        raise SystemExit(1) from None
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
