"""Exercise Responses turns and derived-view replay through the v2-only synthetic listener."""
import ipaddress
import json
import sys
from urllib.parse import urlsplit

import openai
from pydantic import BaseModel


class Answer(BaseModel):
    ok: bool


def client_for(base_url: str) -> openai.OpenAI:
    """Reject non-loopback targets and private client defaults before making requests."""
    if openai.__version__ != "3.19.0":
        raise RuntimeError("expected pinned openai==3.19.0")
    parsed = urlsplit(base_url)
    if (parsed.scheme != "http" or not parsed.hostname or not parsed.port
            or not ipaddress.ip_address(parsed.hostname).is_loopback
            or parsed.path != "/v1" or parsed.query or parsed.fragment
            or parsed.username or parsed.password):
        raise ValueError("only a literal loopback /v1 listener is allowed")
    return openai.OpenAI(
        api_key="synthetic-local-token", base_url=base_url, max_retries=0,
        timeout=8.0, organization="", project="", _strict_response_validation=True,
        http_client=openai.DefaultHttpxClient(trust_env=False, follow_redirects=False),
    )


def run(base_url: str, stream: bool) -> dict[str, object]:
    """Check stateless Responses history and parsed-view replay on the fixed listener."""
    client = client_for(base_url)
    tools = [
        {"type": "function", "name": "lookup", "parameters": {"type": "object", "properties": {"n": {"type": "integer"}}, "required": ["n"], "additionalProperties": False}, "strict": True},
        {"type": "custom", "name": "sql", "format": {"type": "grammar", "syntax": "regex", "definition": "SELECT [0-9]+"}},
    ]
    history: list[dict[str, object]] = [{"role": "user", "content": "hello 🧪"}]
    event_counts: list[int] = []
    try:
        for turn in (1, 2, 3):
            params = dict(model="fixture-model", input=history, instructions="Answer precisely",
                          store=False, tools=tools, tool_choice="auto" if turn == 1 else "none",
                          reasoning={"effort": "low", "summary": "auto"},
                          text={"format": {"type": "json_schema", "name": "answer", "schema": {"type": "object", "properties": {"ok": {"type": "boolean"}}, "required": ["ok"], "additionalProperties": False}, "strict": True}})
            if turn == 2:
                # Consume through the pinned parser so turn 3 replays real derived views.
                params.pop("text")
                if stream:
                    events = []
                    with client.responses.stream(**params, text_format=Answer) as response_stream:
                        for event in response_stream:
                            events.append(event.type)
                        result = response_stream.get_final_response()
                    assert events.count("response.completed") == 1
                    event_counts.append(len(events))
                else:
                    result = client.responses.parse(**params, text_format=Answer)
                assert result.output_parsed == Answer(ok=True), "final body must come from modified IR"
            elif stream:
                events = []
                with client.responses.stream(**params) as response_stream:
                    for event in response_stream:
                        events.append(event.type)
                    result = response_stream.get_final_response()
                assert events.count("response.completed") == 1
                event_counts.append(len(events))
            else:
                result = client.responses.create(**params)
            assert result.status == "completed" and result.usage.total_tokens == 8
            if turn == 1:
                calls = [item for item in result.output if item.type in ("function_call", "custom_tool_call")]
                assert len(calls) == 2 and {call.call_id for call in calls} == {"c_lookup", "c_sql"}
                assert any(item.type == "reasoning" for item in result.output)
                history.extend(item.model_dump(exclude_none=True) for item in result.output)
                history.extend([
                    {"type": "custom_tool_call_output", "call_id": "c_sql", "output": "1"},
                    {"type": "function_call_output", "call_id": "c_lookup", "output": "{\"n\":1}"},
                ])
            elif turn == 2:
                dumped = [item.model_dump(exclude_none=True) for item in result.output]
                assert any("parsed" in part for item in dumped if item["type"] == "message"
                           for part in item["content"]), "dump must carry the derived view"
                history.extend(dumped)
            else:
                assert result.output_text == '{"ok":false}', "raw body stays authoritative on replay"
        return {"turns": 3, "stream": stream, "event_counts": event_counts}
    finally:
        client.close()


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[2] not in ("json", "sse"):
        raise SystemExit("usage: responses_text_loop.py LOOPBACK_BASE_URL json|sse")
    try:
        print(json.dumps(run(sys.argv[1], sys.argv[2] == "sse")))
    except Exception as exc:
        cause = exc.__cause__
        errors = ([{"loc": list(entry["loc"]), "type": entry["type"]} for entry in cause.errors()]
                  if cause is not None and hasattr(cause, "errors") else [])
        print(json.dumps({"error": type(exc).__name__, "status": getattr(exc, "status_code", None), "validation": errors[:8]}), file=sys.stderr)
        raise SystemExit(1) from None
