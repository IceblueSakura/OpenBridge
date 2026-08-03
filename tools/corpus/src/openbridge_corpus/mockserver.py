"""Independent h11-based HTTP mock server and bidirectional protocol observation recorder."""

from __future__ import annotations

import asyncio
import base64
import json
import time
from typing import Any

import h11

from .corpuslib import CorpusError, sha256_bytes
from .sse import parse_sse


SENSITIVE_HEADERS = {
    "authorization",
    "cookie",
    "proxy-authorization",
    "set-cookie",
    "x-api-key",
}


def _headers_as_text(
    headers: list[tuple[bytes, bytes]], *, redact: bool
) -> list[list[str]]:
    """Convert request and response headers to observation text and redact them as needed."""
    result: list[list[str]] = []
    for raw_name, raw_value in headers:
        name = raw_name.decode("ascii", errors="replace").lower()
        value = raw_value.decode("latin-1", errors="replace")
        if redact and name in SENSITIVE_HEADERS:
            value = "<redacted>"
        result.append([name, value])
    return result


class MockServer:
    """Respond in precompiled scenario order and reject undeclared exchanges."""

    def __init__(
        self,
        scenario: dict[str, Any],
        *,
        host: str = "127.0.0.1",
        port: int = 0,
    ) -> None:
        """Validate the precompiled scenario and initialize independent server exchange state."""
        self.scenarios = (
            list(scenario["exchanges"]) if "exchanges" in scenario else [scenario]
        )
        if not self.scenarios:
            raise CorpusError("mock server requires at least one exchange")
        self.host = host
        self.port = port
        for exchange in self.scenarios:
            wire = b"".join(
                base64.b64decode(value, validate=True)
                for value in exchange["response"]["chunks_base64"]
            )
            if sha256_bytes(wire) != exchange["response"]["wire_sha256"]:
                raise CorpusError(
                    f"{exchange['case_id']}: server scenario wire_sha256 does not "
                    "match chunks_base64"
                )
        self.bound_port: int | None = None
        self.observation: dict[str, Any] | None = None
        self.observations: list[dict[str, Any] | None] = [
            None for _ in self.scenarios
        ]
        self._server: asyncio.Server | None = None
        self._finished = asyncio.Event()
        self._claim_lock = asyncio.Lock()
        self._next_exchange = 0

    async def start(self) -> int:
        """Bind the listening socket and return the actual port."""
        self._server = await asyncio.start_server(
            self._handle_connection, self.host, self.port
        )
        socket = self._server.sockets[0]
        self.bound_port = int(socket.getsockname()[1])
        return self.bound_port

    async def wait(self, timeout: float = 30.0) -> dict[str, Any]:
        """Wait for one exchange to finish and return its observation."""
        if len(self.scenarios) != 1:
            raise CorpusError("wait() is only valid for a single exchange")
        await asyncio.wait_for(self._finished.wait(), timeout=timeout)
        assert self.observation is not None
        return self.observation

    async def wait_all(self, timeout: float = 30.0) -> list[dict[str, Any]]:
        """Wait for all exchanges and return observations in scenario order."""
        await asyncio.wait_for(self._finished.wait(), timeout=timeout)
        assert all(item is not None for item in self.observations)
        return [item for item in self.observations if item is not None]

    async def close(self) -> None:
        """Stop the server and wait for the listening socket to close."""
        if self._server is not None:
            self._server.close()
            await self._server.wait_closed()

    async def _handle_connection(
        self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter
    ) -> None:
        """Handle health checks, protocol errors, and fixture exchanges claimed in order."""
        started_at_ns = time.monotonic_ns()
        connection = h11.Connection(h11.SERVER)
        request_event: h11.Request | None = None
        body_parts: list[bytes] = []
        exchange: dict[str, Any] | None = None
        exchange_index: int | None = None
        error: str | None = None
        client_disconnected = False
        try:
            request_event, body_parts = await self._read_request(connection, reader)
            target = request_event.target.decode("ascii", errors="replace")
            path = target.partition("?")[0]
            if path in {"/health", "/healthz"}:
                await self._write_json_response(
                    connection,
                    writer,
                    200,
                    {
                        "pending_exchanges": len(self.scenarios)
                        - self._next_exchange,
                        "status": "ok",
                    },
                )
                return
            if request_event.method != b"POST":
                await self._write_fixture_error(
                    connection, writer, 405, "method_not_allowed"
                )
                return
            if path not in {"/v1/chat/completions", "/v1/responses"}:
                await self._write_fixture_error(
                    connection, writer, 404, "unknown_fixture_endpoint"
                )
                return
            try:
                json.loads(b"".join(body_parts))
            except (json.JSONDecodeError, UnicodeError):
                await self._write_fixture_error(
                    connection, writer, 400, "invalid_json"
                )
                return
            exchange_index, exchange = await self._claim_exchange()
            if exchange is None:
                await self._write_json_response(
                    connection,
                    writer,
                    409,
                    {"error": {"code": "no_pending_exchange"}},
                )
                return
            await self._write_response(connection, writer, exchange)
        except (
            BrokenPipeError,
            ConnectionResetError,
            h11.RemoteProtocolError,
            h11.LocalProtocolError,
        ) as caught:
            error = f"{type(caught).__name__}: {caught}"
            client_disconnected = True
        except Exception as caught:
            error = f"{type(caught).__name__}: {caught}"
        finally:
            if not writer.is_closing():
                writer.close()
                try:
                    await writer.wait_closed()
                except (BrokenPipeError, ConnectionResetError):
                    client_disconnected = True
            if exchange is not None and exchange_index is not None:
                body = b"".join(body_parts)
                observation = self._build_observation(
                    exchange,
                    request_event,
                    body,
                    error=error,
                    client_disconnected=client_disconnected,
                    started_at_ns=started_at_ns,
                )
                self.observation = observation
                self.observations[exchange_index] = observation
                if all(item is not None for item in self.observations):
                    self._finished.set()

    async def _claim_exchange(
        self,
    ) -> tuple[int | None, dict[str, Any] | None]:
        """Claim one exchange in order under the lock so concurrent requests cannot reuse a fixture."""
        async with self._claim_lock:
            if self._next_exchange >= len(self.scenarios):
                return None, None
            index = self._next_exchange
            self._next_exchange += 1
            return index, self.scenarios[index]

    async def _read_request(
        self, connection: h11.Connection, reader: asyncio.StreamReader
    ) -> tuple[h11.Request, list[bytes]]:
        """Read a complete HTTP request incrementally while preserving raw body chunks."""
        request: h11.Request | None = None
        body: list[bytes] = []
        while True:
            event = connection.next_event()
            if event is h11.NEED_DATA:
                data = await reader.read(65536)
                if not data:
                    connection.receive_data(b"")
                else:
                    connection.receive_data(data)
                continue
            if isinstance(event, h11.Request):
                request = event
            elif isinstance(event, h11.Data):
                body.append(event.data)
            elif isinstance(event, h11.EndOfMessage):
                if request is None:
                    raise h11.RemoteProtocolError("request ended before Request event")
                return request, body
            elif isinstance(event, h11.ConnectionClosed):
                raise ConnectionResetError("client closed before request completed")

    async def _write_response(
        self,
        connection: h11.Connection,
        writer: asyncio.StreamWriter,
        exchange: dict[str, Any],
    ) -> None:
        """Write the scenario status, headers, fragments, and complete/abort termination mode."""
        response = exchange["response"]
        headers = [
            (name.encode("ascii"), value.encode("latin-1"))
            for name, value in response["headers"]
        ]
        writer.write(
            connection.send(
                h11.Response(status_code=response["status"], headers=headers)
            )
        )
        await writer.drain()
        delay = response["chunk_delay_ms"] / 1000
        for encoded in response["chunks_base64"]:
            chunk = base64.b64decode(encoded, validate=True)
            writer.write(connection.send(h11.Data(data=chunk)))
            await writer.drain()
            if delay:
                await asyncio.sleep(delay)
        if response["termination"] == "abort":
            if response["abort_delay_ms"]:
                await asyncio.sleep(response["abort_delay_ms"] / 1000)
            transport = writer.transport
            transport.abort()
            return
        writer.write(connection.send(h11.EndOfMessage()))
        await writer.drain()

    async def _write_json_response(
        self,
        connection: h11.Connection,
        writer: asyncio.StreamWriter,
        status: int,
        document: dict[str, Any],
    ) -> None:
        """Send a deterministic JSON error or health response."""
        body = json.dumps(
            document, ensure_ascii=False, separators=(",", ":"), sort_keys=True
        ).encode("utf-8")
        headers = [
            (b"content-type", b"application/json"),
            (b"content-length", str(len(body)).encode("ascii")),
        ]
        writer.write(connection.send(h11.Response(status_code=status, headers=headers)))
        writer.write(connection.send(h11.Data(data=body)))
        writer.write(connection.send(h11.EndOfMessage()))
        await writer.drain()

    async def _write_fixture_error(
        self,
        connection: h11.Connection,
        writer: asyncio.StreamWriter,
        status: int,
        code: str,
    ) -> None:
        """Encode a mock-server request error as a standardized JSON error response."""
        await self._write_json_response(
            connection,
            writer,
            status,
            {
                "error": {
                    "code": code,
                    "message": "upstream fixture request failed",
                    "type": "invalid_request_error",
                }
            },
        )

    def _build_observation(
        self,
        exchange: dict[str, Any],
        request: h11.Request | None,
        body: bytes,
        *,
        error: str | None,
        client_disconnected: bool,
        started_at_ns: int,
    ) -> dict[str, Any]:
        """Build a redacted observation from HTTP, SSE terminal, and disconnection state."""
        response = exchange["response"]
        response_wire = b"".join(
            base64.b64decode(value) for value in response["chunks_base64"]
        )
        content_type = next(
            (
                value
                for name, value in response["headers"]
                if name.lower() == "content-type"
            ),
            "",
        )
        terminals = (
            [event.terminal for event in parse_sse(response_wire) if event.terminal]
            if content_type.startswith("text/event-stream")
            else []
        )
        if client_disconnected:
            end = "cancelled"
        elif response["termination"] == "abort":
            end = "transport_error"
        elif response["status"] >= 400:
            end = "error_response"
        elif content_type.startswith("text/event-stream"):
            end = "terminal" if terminals else "eof"
        else:
            end = "response"
        body_json: Any = None
        try:
            body_json = json.loads(body)
        except (json.JSONDecodeError, UnicodeError):
            pass
        return {
            "body_base64": base64.b64encode(body).decode("ascii"),
            "body_json": body_json,
            "body_sha256": sha256_bytes(body),
            "case_id": exchange["case_id"],
            "client_disconnected": client_disconnected,
            "end": end,
            "error": error,
            "request": {
                "headers": (
                    _headers_as_text(list(request.headers), redact=True)
                    if request is not None
                    else []
                ),
                "http_version": (
                    request.http_version.decode("ascii", errors="replace")
                    if request is not None
                    else None
                ),
                "method": (
                    request.method.decode("ascii", errors="replace")
                    if request is not None
                    else None
                ),
                "target": (
                    request.target.decode("ascii", errors="replace")
                    if request is not None
                    else None
                ),
            },
            "response": {
                "status": response["status"],
                "terminal_kinds": terminals,
                "termination": response["termination"],
                "wire_sha256": sha256_bytes(response_wire),
            },
            "role": "mock_server",
            "schema_version": "0.1",
            "timing": {
                "finished_at_ns": time.monotonic_ns(),
                "started_at_ns": started_at_ns,
            },
        }
