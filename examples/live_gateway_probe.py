"""Explicit paid GPT-6 Luna gate through the real binary and pinned OpenAI SDK.

Build `openbridge`, then run with `OPENBRIDGE_GATEWAY_PROBE=1` via
`uv run --project tests/sdk --locked --offline python examples/live_gateway_probe.py`.
Only synthetic data is sent: the default text/tool matrix has at most 12 calls.
`OPENBRIDGE_GATEWAY_REASONING=1` selects 8 encrypted-reasoning acquisition/replay
calls instead. Both cap output at 2048 tokens; no retries, external tools,
inherited SDK proxies, raw-body reports or key/ciphertext logging.
"""
import json
import os
from pathlib import Path
import re
import secrets
import selectors
import signal
import subprocess
import time
import tomllib

from openai import OpenAI, DefaultHttpxClient, __version__

ROOT = Path(__file__).resolve().parents[1]
MODEL = "gpt-6-luna"
PROMPT = 'Use the lookup tool for key "alpha", then report its value in one sentence.'
TOOL = {
    "name": "lookup", "description": "Look up a synthetic stored value.",
    "parameters": {"type": "object", "properties": {"key": {"type": "string"}},
                   "required": ["key"], "additionalProperties": False}, "strict": False,
}


class UnexpectedChatFinish(AssertionError):
    """Closed terminal diagnostics without message content or raw error text."""
    def __init__(self, finish):
        self.finish = finish if finish in ('stop', 'tool_calls', 'length', 'content_filter', 'function_call') else 'missing_or_unknown'
        super().__init__('unexpected Chat terminal')


def chat_result(result, streaming, *, allowed_finishes=("stop", "tool_calls")):
    """Accumulate only the admitted typed deltas, including scoped replay fields."""
    if not streaming:
        if result.choices[0].finish_reason not in allowed_finishes:
            raise UnexpectedChatFinish(result.choices[0].finish_reason)
        message = result.choices[0].message.model_dump(mode="json", exclude_unset=True)
        return [message], message.get("content") or "", message.get("tool_calls") or []
    message = {"role": "assistant", "content": None}
    calls, details = {}, {}
    finish = None
    frames = total = 0
    for chunk in result:
        frames += 1
        wire = chunk.model_dump(mode="json", exclude_unset=True)
        total += len(json.dumps(wire))
        assert frames <= 65536 and total <= 2 * 1024 * 1024
        if not chunk.choices:
            continue
        choice = wire["choices"][0]
        if choice.get("finish_reason"):
            finish = choice["finish_reason"]
        delta = choice["delta"]
        for field in ("content", "reasoning_content"):
            if delta.get(field) is not None:
                message[field] = (message.get(field) or "") + delta[field]
        for part in delta.get("reasoning_details") or []:
            target = details.setdefault(part["index"], {})
            for key, value in part.items():
                if key in ("summary", "text"):
                    target[key] = target.get(key, "") + value
                else:
                    assert key not in target or target[key] == value
                    target[key] = value
        for call in delta.get("tool_calls") or []:
            target = calls.setdefault(call["index"], {"type": "function", "function": {"arguments": ""}})
            if call.get("id"):
                assert "id" not in target or target["id"] == call["id"]
                target["id"] = call["id"]
            fn = call.get("function") or {}
            if fn.get("name"):
                target["function"]["name"] = fn["name"]
            target["function"]["arguments"] += fn.get("arguments") or ""
    if finish not in allowed_finishes:
        raise UnexpectedChatFinish(finish)
    if details:
        message["reasoning_details"] = [details[i] for i in sorted(details)]
    if calls:
        message["tool_calls"] = [calls[i] for i in sorted(calls)]
    return [message], message.get("content") or "", message.get("tool_calls") or []


def response_result(result, streaming):
    if streaming:
        final = None
        total = frames = 0
        for event in result:
            frames += 1
            total += len(event.model_dump_json())
            assert frames <= 65536 and total <= 2 * 1024 * 1024
            if event.type == "response.completed":
                assert final is None
                final = event.response
            assert event.type not in ("response.failed", "response.incomplete", "error")
        assert final is not None
        result = final
    assert result.status == "completed"
    history = [item.model_dump(mode="json", exclude_unset=True) for item in result.output]
    text = "".join(part["text"] for item in history if item["type"] == "message"
                   for part in item["content"] if part["type"] == "output_text")
    return history, text, [item for item in history if item["type"] == "function_call"]


def opaque_records(value):
    """Keep ciphertext and issuer identity in memory only, never in diagnostics."""
    records = []
    if isinstance(value, dict):
        if value.get("type") == "reasoning.encrypted":
            records.append((value.get("id"), value.get("data")))
        elif value.get("type") == "reasoning" and value.get("encrypted_content"):
            records.append((value.get("id"), value["encrypted_content"]))
        for child in value.values():
            records.extend(opaque_records(child))
    elif isinstance(value, list):
        for child in value:
            records.extend(opaque_records(child))
    return records


class ReplayCheckingClient(DefaultHttpxClient):
    """Check the SDK's serialized body before sending it unchanged to loopback."""
    def __init__(self, limit):
        super().__init__(trust_env=False, follow_redirects=False)
        self.limit = limit
        self.sent = 0
        self.expected = None
        self.verified = False

    def send(self, request, **kwargs):
        if request.url.host != "127.0.0.1" or request.method != "POST":
            raise RuntimeError("unexpected SDK destination")
        if self.sent >= self.limit or len(request.content) > 2 * 1024 * 1024:
            raise RuntimeError("probe request limit")
        body = json.loads(request.content)
        if body.get("model") != MODEL:
            raise RuntimeError("unexpected model")
        self.verified = False
        if self.expected is not None:
            if opaque_records(body) != self.expected:
                raise RuntimeError("serialized opaque replay changed")
            self.verified = True
        self.sent += 1
        return super().send(request, **kwargs)


def run():
    if __version__ != "3.19.0":
        raise RuntimeError("pinned SDK required")
    if os.environ.get("OPENBRIDGE_GATEWAY_PROBE") != "1":
        raise RuntimeError("explicit paid gate required")
    # Parse diagnostics are never emitted: TOML errors may include secret source lines.
    data = tomllib.loads((ROOT / "config/upstream-credentials.toml").read_text())
    pools = [p for p in data["credential_pools"] if p["id"] == "openrouter-primary"]
    assert len(pools) == 1
    client_key = secrets.token_urlsafe(32)
    env = {"OPENBRIDGE_BIND": "127.0.0.1:0", "OPENBRIDGE_CLIENT_KEY": client_key,
           "OPENBRIDGE_OPENROUTER_API_KEY": pools[0]["api_keys"][0]}
    for name in ("https_proxy", "HTTPS_PROXY", "http_proxy", "HTTP_PROXY", "all_proxy", "ALL_PROXY"):
        if os.environ.get(name):
            env["OPENBRIDGE_PROXY"] = os.environ[name]
            break
    reasoning_mode = os.environ.get("OPENBRIDGE_GATEWAY_REASONING") == "1"
    limit = 8 if reasoning_mode else 12
    reports = []
    count = 0
    server = subprocess.Popen([str(ROOT / "target/debug/openbridge")], cwd=ROOT, env=env,
                              stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(server.stdout, selectors.EVENT_READ)
            assert selector.select(timeout=15), "startup timeout"
            line = server.stdout.readline(256).strip()
        match = re.fullmatch(r"OpenBridge listening on (http://127\.0\.0\.1:\d+)", line)
        assert match, "missing loopback readiness"
        transport = ReplayCheckingClient(limit)
        with OpenAI(base_url=match[1] + "/v1", api_key=client_key, max_retries=0,
                    organization="", project="", timeout=150, http_client=transport) as client:
            for protocol in ("chat", "responses"):
                for streaming in (False, True):
                    for case in (("encrypted_reasoning",) if reasoning_mode else ("text", "tool")):
                        prompt = "Reply with exactly the word pong." if case == "text" else PROMPT
                        if reasoning_mode:
                            prompt = "Compute (317 * 43) + (89 * 17). Return only a JSON object with integer field answer."
                        history = [{"role": "user", "content": prompt}]
                        expected_replay = None
                        for turn in range(1, 3 if case != "text" else 2):
                            assert count < limit
                            count += 1
                            started = time.monotonic()
                            report = {"protocol": protocol, "delivery": "sse" if streaming else "json",
                                      "case": case, "round": turn, "ok": False}
                            try:
                                params = {"model": MODEL, "stream": streaming}
                                transport.expected = expected_replay
                                if reasoning_mode:
                                    if protocol == "chat":
                                        params.update(reasoning_effort="medium", response_format={"type": "json_object"})
                                    else:
                                        params.update(reasoning={"effort": "medium", "summary": "auto"},
                                                      include=["reasoning.encrypted_content"], text={"format": {"type": "json_object"}})
                                if case == "tool":
                                    params["tool_choice"] = "required" if turn == 1 else "none"
                                    params["tools"] = [{"type": "function", "function": TOOL}] if protocol == "chat" else [{"type": "function", **TOOL}]
                                if protocol == "chat":
                                    params.update(messages=history, max_completion_tokens=2048)
                                    if streaming:
                                        params["stream_options"] = {"include_usage": True, "include_obfuscation": False}
                                    result = client.chat.completions.create(**params)
                                    output, text, calls = chat_result(result, streaming)
                                else:
                                    params.update(input=history, max_output_tokens=2048)
                                    result = client.responses.create(**params)
                                    output, text, calls = response_result(result, streaming)
                                if reasoning_mode:
                                    answer = json.loads(text)["answer"]
                                    assert type(answer) is int and answer == 15144 + (37 if turn == 2 else 0)
                                    assert not calls
                                    records = opaque_records(output)
                                    report["encrypted_records"] = len(records)
                                    report["encrypted_bytes"] = sum(len(token.encode("utf-8")) for _, token in records)
                                    if turn == 1:
                                        assert records and all(isinstance(identity, str) and identity and isinstance(token, str) and token for identity, token in records), "no final encrypted owner"
                                        expected_replay = records
                                        history.extend(output)
                                        history.append({"role": "user", "content": "Add 37 to that answer. Return only JSON with integer field answer."})
                                    else:
                                        assert transport.verified
                                        report["serialized_replay_verified"] = True
                                elif case == "text":
                                    assert text.strip() == "pong"
                                elif turn == 1:
                                    assert len(calls) == 1
                                    call = calls[0]
                                    fn = call["function"] if protocol == "chat" else call
                                    assert fn["name"] == "lookup" and json.loads(fn["arguments"]) == {"key": "alpha"}
                                    history.extend(output)
                                    if protocol == "chat":
                                        history.append({"role": "tool", "tool_call_id": call["id"], "content": '{"value":42}'})
                                    else:
                                        history.append({"type": "function_call_output", "call_id": call["call_id"], "output": '{"value":42}'})
                                else:
                                    assert not calls and "42" in text
                                report["ok"] = True
                            except Exception as error:
                                report["error_type"] = type(error).__name__
                                report["http_status"] = getattr(error, "status_code", None)
                            report["latency_ms"] = round((time.monotonic() - started) * 1000)
                            reports.append(report)
                            print(json.dumps(report), flush=True)
                            if not report["ok"]:
                                break
    finally:
        server.send_signal(signal.SIGINT)
        try:
            server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=5)
    label = "gateway-luna-reasoning" if reasoning_mode else "gateway-luna"
    directory = ROOT / "testdata/runtime" / f"{label}-{time.time_ns()}"
    directory.mkdir(parents=True)
    (directory / "calls.json").write_text(json.dumps(reports, indent=2) + "\n")
    print(f"report: {directory.relative_to(ROOT)}/calls.json")
    return count == limit and all(r["ok"] for r in reports)


if __name__ == "__main__":
    try:
        success = run()
    except Exception as error:
        # Deliberately exclude exception text, request objects and subprocess environment.
        print(f"gateway probe setup/runtime failed: {type(error).__name__}")
        success = False
    raise SystemExit(0 if success else 1)
