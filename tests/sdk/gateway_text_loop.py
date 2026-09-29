"""Exercise the production gateway Router against a synthetic HTTP Provider only."""
import ipaddress
import json
import sys
from urllib.parse import urlsplit

import openai


def run(base_url: str) -> None:
    """Verify real SDK tools, reasoning replay and JSON/SSE consumption without retries."""
    parsed = urlsplit(base_url)
    if (openai.__version__ != "3.19.0" or parsed.scheme != "http"
            or not parsed.hostname or not parsed.port
            or not ipaddress.ip_address(parsed.hostname).is_loopback
            or parsed.path != "/v1" or parsed.query or parsed.fragment
            or parsed.username or parsed.password):
        raise ValueError("pinned SDK and literal loopback /v1 required")
    schema = {"type": "object", "properties": {"n": {"type": "integer"}},
              "required": ["n"], "additionalProperties": False}
    function = {"name": "lookup", "parameters": schema, "strict": True}
    requests = 0
    with openai.OpenAI(
        api_key="synthetic-gateway-client-token-0001", base_url=base_url,
        organization="", project="", max_retries=0, timeout=5.0,
        _strict_response_validation=True,
        http_client=openai.DefaultHttpxClient(trust_env=False, follow_redirects=False),
    ) as client:
        for stream in (False, True):
            history = [{"role": "user", "content": "lookup"}]
            for turn in (1, 2):
                params = dict(model="public-model", messages=history,
                              tools=[{"type": "function", "function": function}],
                              tool_choice="auto" if turn == 1 else "none")
                if stream:
                    with client.chat.completions.stream(
                        **params, stream_options={"include_usage": True, "include_obfuscation": False}
                    ) as events:
                        for _ in events:
                            pass
                        result = events.get_final_completion()
                else:
                    result = client.chat.completions.parse(**params)
                requests += 1
                assert result.model == "public-model" and result.usage.total_tokens == 5
                message = result.choices[0].message
                if turn == 1:
                    call = message.tool_calls[0]
                    assert call.id == "call-local" and json.loads(call.function.arguments) == {"n": 1}
                    history.append(message.model_dump(exclude_none=True))
                    history.append({"role": "tool", "tool_call_id": call.id, "content": '{"n":1}'})
                else:
                    assert result.choices[0].finish_reason == "stop" and message.content == "old 🧪"
        for stream in (False, True):
            history = [{"role": "user", "content": "lookup"}]
            tools = [{"type": "function", **function},
                     {"type": "custom", "name": "sql", "format": {
                         "type": "grammar", "syntax": "regex", "definition": "SELECT [0-9]+"}}]
            for turn in (1, 2):
                params = dict(model="public-model", input=history, instructions="Answer precisely",
                              store=False, tools=tools, tool_choice="auto" if turn == 1 else "none")
                if stream:
                    completed = 0
                    with client.responses.stream(**params) as events:
                        for event in events:
                            completed += event.type == "response.completed"
                        result = events.get_final_response()
                    assert completed == 1
                else:
                    result = client.responses.parse(**params)
                requests += 1
                assert result.model == "public-model" and result.status == "completed"
                assert result.usage.total_tokens == 8 and result.max_output_tokens == 32
                if turn == 1:
                    dumped = [item.model_dump(exclude_none=True) for item in result.output]
                    assert {item["call_id"] for item in dumped if item["type"] in (
                        "function_call", "custom_tool_call")} == {"c_lookup", "c_sql"}
                    assert any(item.get("encrypted_content") == "synthetic-final-token" for item in dumped)
                    history.extend(dumped)
                    history.extend([
                        {"type": "function_call_output", "call_id": "c_lookup", "output": '{"n":1}'},
                        {"type": "custom_tool_call_output", "call_id": "c_sql", "output": "1"},
                    ])
                else:
                    assert result.output_text == '{"ok":false}'
    print(json.dumps({"requests": requests, "protocols": 2, "deliveries": 2}))


if __name__ == "__main__":
    run(sys.argv[1])
