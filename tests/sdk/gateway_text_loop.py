"""Exercise the production gateway Router against a synthetic HTTP Provider only."""
import ipaddress
import json
import sys
from sdk_support import check
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
                check(result.model == "public-model" and result.usage.total_tokens == 5)
                message = result.choices[0].message
                if turn == 1:
                    call = message.tool_calls[0]
                    check(call.id == "call-local" and json.loads(call.function.arguments) == {"n": 1})
                    history.append(message.model_dump(exclude_none=True))
                    history.append({"role": "tool", "tool_call_id": call.id, "content": '{"n":1}'})
                else:
                    check(result.choices[0].finish_reason == "stop" and message.content == "old 🧪")
        for stream in (False, True):
            history = [{"role": "user", "content": [
                {"type": "input_text", "text": "lookup"},
                {"type": "input_file", "file_data": "data:application/pdf;base64,AQID",
                 "filename": "synthetic.pdf", "detail": "low"},
                {"type": "input_image", "image_url": "data:image/png;base64,AQID"},
                {"type": "input_file", "file_url": "https://example.invalid/resource?sig=synthetic",
                 "filename": "remote.pdf"},
            ]}]
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
                    check(completed == 1)
                else:
                    result = client.responses.parse(**params)
                requests += 1
                check(result.model == "public-model" and result.status == "completed")
                check(result.usage.total_tokens == 8 and result.max_output_tokens == 32)
                if turn == 1:
                    dumped = [item.model_dump(exclude_none=True) for item in result.output]
                    check({item["call_id"] for item in dumped if item["type"] in (
                        "function_call", "custom_tool_call")} == {"c_lookup", "c_sql"})
                    check(any(item.get("encrypted_content") == "synthetic-final-token" for item in dumped))
                    history.extend(dumped)
                    history.extend([
                        {"type": "function_call_output", "call_id": "c_lookup", "output": '{"n":1}'},
                        {"type": "custom_tool_call_output", "call_id": "c_sql", "output": "1"},
                    ])
                else:
                    check(result.output_text == '{"ok":false}')
        # Validate empty owner items with a complete independent Responses envelope;
        # do not invent missing Chat-reported settings to satisfy strict SDK validation.
        for stream in (False, True):
            params = dict(model="public-model", input="lookup", store=False,
                          metadata={"case": "empty-owner"},
                          tools=[{"type": "function", **function}])
            if stream:
                completed = 0
                with client.responses.stream(**params) as events:
                    for event in events:
                        completed += event.type == "response.completed"
                    result = events.get_final_response()
                check(completed == 1)
            else:
                result = client.responses.parse(**params)
            requests += 1
            check(result.model == "public-model" and result.status == "completed")
            check(len(result.output) == 2)
            owner, call = result.output
            check(owner.type == "message" and owner.status == "completed" and owner.content == [])
            check(call.type == "function_call" and call.call_id == "call-local")
            check(json.loads(call.arguments) == {"n": 1})
        image = client.images.generate(model="public-image", prompt="synthetic image", n=1,
            stream=False, size="1536x1024", quality="high", background="transparent",
            output_format="webp", output_compression=80, moderation="low", user="synthetic-sdk-user")
        requests += 1
        check(image.created == 7 and image.data is not None and len(image.data) == 1)
        check(image.data[0].b64_json == "AQID" and image.data[0].url is None)
        check(image.size == "1536x1024" and image.quality == "high"
              and image.background == "transparent" and image.output_format == "webp")
        check(image.usage is None)
    print(json.dumps({"requests": requests, "protocols": 2, "deliveries": 2, "image_requests": 1}))


if __name__ == "__main__":
    run(sys.argv[1])
