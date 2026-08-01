"""使用 h11 执行独立的 HTTP mock client，并记录原始响应观察结果。"""

from __future__ import annotations

import asyncio
import base64
import json
import time
from typing import Any
from urllib.parse import urlsplit

import h11

from .corpuslib import CorpusError, sha256_bytes
from .mockserver import SENSITIVE_HEADERS
from .sse import IncrementalSseParser, SseEvent


def _text_headers(
    headers: list[tuple[bytes, bytes]], *, redact: bool = True
) -> list[list[str]]:
    """将 HTTP header 转为文本，并按安全边界脱敏敏感值。"""
    result: list[list[str]] = []
    for raw_name, raw_value in headers:
        name = raw_name.decode("ascii", errors="replace").lower()
        value = raw_value.decode("latin-1", errors="replace")
        if redact and name in SENSITIVE_HEADERS:
            value = "<redacted>"
        result.append([name, value])
    return result


async def run_mock_client(plan: dict[str, Any]) -> dict[str, Any]:
    """按预编译 client plan 发起请求并返回可校验的 observation。"""
    parsed = urlsplit(plan["url"])
    if parsed.scheme != "http" or not parsed.hostname:
        raise CorpusError("mock client currently supports absolute http:// URLs only")
    port = parsed.port or 80
    target = parsed.path or "/"
    if parsed.query:
        target += f"?{parsed.query}"
    body = base64.b64decode(plan["body_base64"], validate=True)
    if sha256_bytes(body) != plan["body_sha256"]:
        raise CorpusError("client plan body_sha256 does not match body_base64")

    started_at_ns = time.monotonic_ns()
    response_event: h11.Response | None = None
    response_body: list[bytes] = []
    sse_events: list[SseEvent] = []
    parser = IncrementalSseParser()
    end = "transport_error"
    error: str | None = None
    reader: asyncio.StreamReader | None = None
    writer: asyncio.StreamWriter | None = None
    try:
        async with asyncio.timeout(plan["timeout_ms"] / 1000):
            reader, writer = await asyncio.open_connection(parsed.hostname, port)
            connection = h11.Connection(h11.CLIENT)
            request_headers = [
                (name.encode("ascii"), value.encode("latin-1"))
                for name, value in plan["headers"]
                if name.lower() not in {"host", "content-length"}
            ]
            host = parsed.hostname if parsed.port is None else f"{parsed.hostname}:{port}"
            request_headers.extend(
                [
                    (b"host", host.encode("ascii")),
                    (b"content-length", str(len(body)).encode("ascii")),
                    (b"connection", b"close"),
                ]
            )
            writer.write(
                connection.send(
                    h11.Request(
                        method=plan["method"].encode("ascii"),
                        target=target.encode("ascii"),
                        headers=request_headers,
                    )
                )
            )
            if body:
                writer.write(connection.send(h11.Data(data=body)))
            writer.write(connection.send(h11.EndOfMessage()))
            await writer.drain()
            cancelled = False
            content_type = ""
            while True:
                event = connection.next_event()
                if event is h11.NEED_DATA:
                    received = await reader.read(65536)
                    connection.receive_data(received)
                    continue
                if isinstance(event, h11.Response):
                    response_event = event
                    content_type = next(
                        (
                            value.decode("latin-1")
                            for name, value in event.headers
                            if name.lower() == b"content-type"
                        ),
                        "",
                    )
                elif isinstance(event, h11.Data):
                    response_body.append(event.data)
                    if content_type.startswith("text/event-stream"):
                        new_events = parser.feed(event.data)
                        cancel_after = plan["cancel_after_event"]
                        if cancel_after is not None:
                            remaining = cancel_after - len(sse_events)
                            sse_events.extend(new_events[: max(0, remaining)])
                            if len(sse_events) >= cancel_after:
                                cancelled = True
                                writer.transport.abort()
                                break
                        else:
                            sse_events.extend(new_events)
                elif isinstance(event, h11.EndOfMessage):
                    if response_event is not None and response_event.status_code >= 400:
                        if content_type.startswith("text/event-stream"):
                            parser.close()
                        end = "error_response"
                    elif content_type.startswith("text/event-stream"):
                        parser.close()
                        end = (
                            "terminal"
                            if any(item.terminal for item in sse_events)
                            else "eof"
                        )
                    else:
                        end = "response"
                    break
                elif isinstance(event, h11.ConnectionClosed):
                    parser.close()
                    end = "transport_error"
                    error = "connection closed before HTTP message completed"
                    break
            if cancelled:
                parser.close()
                end = "cancelled"
    except TimeoutError:
        parser.close()
        error = "timeout"
        end = "transport_error"
    except (
        OSError,
        h11.RemoteProtocolError,
        h11.LocalProtocolError,
    ) as caught:
        parser.close()
        error = f"{type(caught).__name__}: {caught}"
        end = "transport_error"
    finally:
        if writer is not None and not writer.is_closing():
            writer.close()
            try:
                await writer.wait_closed()
            except (BrokenPipeError, ConnectionResetError):
                pass

    raw_body = b"".join(response_body)
    body_json: Any = None
    if response_event is not None:
        content_type = next(
            (
                value.decode("latin-1")
                for name, value in response_event.headers
                if name.lower() == b"content-type"
            ),
            "",
        )
        if content_type.startswith("application/json"):
            try:
                body_json = json.loads(raw_body)
            except (json.JSONDecodeError, UnicodeError):
                pass
    return {
        "body_base64": base64.b64encode(raw_body).decode("ascii"),
        "body_chunks_base64": [
            base64.b64encode(chunk).decode("ascii") for chunk in response_body
        ],
        "body_json": body_json,
        "body_sha256": sha256_bytes(raw_body),
        "case_id": plan["case_id"],
        "end": end,
        "error": error,
        "events": [event.as_dict() for event in sse_events],
        "response": {
            "headers": (
                _text_headers(list(response_event.headers))
                if response_event is not None
                else []
            ),
            "http_version": (
                response_event.http_version.decode("ascii", errors="replace")
                if response_event is not None
                else None
            ),
            "status": response_event.status_code if response_event is not None else None,
            "terminal_kinds": [
                event.terminal for event in sse_events if event.terminal
            ],
        },
        "role": "mock_client",
        "schema_version": "0.1",
        "timing": {
            "finished_at_ns": time.monotonic_ns(),
            "started_at_ns": started_at_ns,
        },
    }
