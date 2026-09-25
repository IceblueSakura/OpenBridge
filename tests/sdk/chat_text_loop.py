"""Exercise Chat create and typed chunks; parsed-view replay is a separate gate."""
import json
import sys
from responses_text_loop import client_for


def run(base_url: str, stream: bool) -> dict[str, object]:
    """Use synthetic tools, no execution, no external target and no automatic retry."""
    client = client_for(base_url)
    history = [{"role": "user", "content": "hello 🧪"}]
    tools = [{"type": "function", "function": {"name": "lookup", "parameters": {"type": "object", "properties": {"n": {"type": "integer"}}, "required": ["n"], "additionalProperties": False}, "strict": True}}]
    counts = []
    try:
        for turn in (1, 2):
            params = dict(model="fixture-model", messages=history, tools=tools,
                          tool_choice="auto" if turn == 1 else "none", n=1, stream=stream)
            if turn == 2:
                params["response_format"] = {"type": "json_schema", "json_schema": {"name": "answer", "strict": True, "schema": {"type": "object", "properties": {"answer": {"type": "string"}}, "required": ["answer"], "additionalProperties": False}}}
            if stream:
                content = ""
                calls = {}
                finish = None
                usage = None
                count = 0
                params["stream_options"] = {"include_usage": True, "include_obfuscation": False}
                with client.chat.completions.create(**params) as chunks:
                    for chunk in chunks:
                        count += 1
                        assert chunk.id == "chat-local" and chunk.model == "fixture-model"
                        if not chunk.choices:
                            assert finish is not None and usage is None
                            usage = chunk.usage
                            continue
                        assert finish is None
                        choice = chunk.choices[0]
                        assert choice.index == 0
                        delta = choice.delta
                        content += delta.content or ""
                        for call in delta.tool_calls or []:
                            if call.index not in calls:
                                assert call.id and call.function.name
                                calls[call.index] = {"id": call.id, "type": "function", "function": {"name": call.function.name, "arguments": ""}}
                            calls[call.index]["function"]["arguments"] += call.function.arguments or ""
                        finish = choice.finish_reason
                message = {"role": "assistant", "content": content or None}
                if calls:
                    message["tool_calls"] = list(calls.values())
                counts.append(count)
            else:
                result = client.chat.completions.create(**params)
                assert len(result.choices) == 1
                choice = result.choices[0]
                finish, usage = choice.finish_reason, result.usage
                message = choice.message.model_dump(exclude_none=True)
            assert usage is not None and usage.total_tokens == 5
            if turn == 1:
                assert finish == "tool_calls"
                call = message["tool_calls"][0]
                assert call["id"] == "call-local"
                assert call["function"] == {"name": "lookup", "arguments": '{"n":1}'}
                history.extend([message, {"role": "tool", "tool_call_id": "call-local", "content": "synthetic result"}])
            else:
                assert finish == "stop" and message["content"] == '{"answer":"new 🧪"}'
                assert json.loads(message["content"]) == {"answer": "new 🧪"}
        return {"turns": 2, "stream": stream, "event_counts": counts}
    finally:
        client.close()


if __name__ == "__main__":
    if len(sys.argv) != 3 or sys.argv[2] not in ("json", "sse"):
        raise SystemExit("usage: chat_text_loop.py LOOPBACK_BASE_URL json|sse")
    try:
        print(json.dumps(run(sys.argv[1], sys.argv[2] == "sse")))
    except Exception as exc:
        print(json.dumps({"error": type(exc).__name__, "status": getattr(exc, "status_code", None)}), file=sys.stderr)
        raise SystemExit(1) from None
