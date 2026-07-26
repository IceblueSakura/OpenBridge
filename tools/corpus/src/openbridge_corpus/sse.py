from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any


TERMINAL_TYPES = {
    "response.completed": "response_completed",
    "response.failed": "response_failed",
    "response.incomplete": "response_incomplete",
    "error": "error",
}


@dataclass(frozen=True)
class SseEvent:
    event_field: str | None
    data_text: str
    data_json: Any
    json_error: str | None
    payload_type: str | None
    type_conflict: bool
    terminal: str | None

    def as_dict(self) -> dict[str, Any]:
        return {
            "data_json": self.data_json,
            "data_text": self.data_text,
            "event_field": self.event_field,
            "json_error": self.json_error,
            "payload_type": self.payload_type,
            "terminal": self.terminal,
            "type_conflict": self.type_conflict,
        }


class IncrementalSseParser:
    """Incrementally parse an SSE byte stream without assuming read boundaries."""

    def __init__(self) -> None:
        self._line = bytearray()
        self._skip_lf = False
        self._event_field: str | None = None
        self._data_lines: list[str] = []
        self._closed = False

    def feed(self, data: bytes) -> list[SseEvent]:
        if self._closed:
            raise RuntimeError("cannot feed a closed SSE parser")
        events: list[SseEvent] = []
        for value in data:
            if self._skip_lf:
                self._skip_lf = False
                if value == 0x0A:
                    continue
            if value == 0x0D:
                events.extend(self._finish_line())
                self._skip_lf = True
            elif value == 0x0A:
                events.extend(self._finish_line())
            else:
                self._line.append(value)
        return events

    def close(self) -> None:
        """Close the stream and discard an event not terminated by a blank line."""

        self._closed = True
        self._line.clear()
        self._event_field = None
        self._data_lines.clear()

    def _finish_line(self) -> list[SseEvent]:
        raw = bytes(self._line)
        self._line.clear()
        if not raw:
            event = self._dispatch()
            return [event] if event is not None else []
        line = raw.decode("utf-8")
        if line.startswith(":"):
            return []
        field, separator, value = line.partition(":")
        if separator and value.startswith(" "):
            value = value[1:]
        if field == "event":
            self._event_field = value
        elif field == "data":
            self._data_lines.append(value)
        return []

    def _dispatch(self) -> SseEvent | None:
        if not self._data_lines:
            self._event_field = None
            return None
        data_text = "\n".join(self._data_lines)
        data_json: Any = None
        json_error: str | None = None
        payload_type: str | None = None
        terminal: str | None = None
        if data_text == "[DONE]":
            terminal = "chat_done"
        else:
            try:
                data_json = json.loads(data_text)
            except json.JSONDecodeError as error:
                json_error = str(error)
            if isinstance(data_json, dict):
                value = data_json.get("type")
                if isinstance(value, str):
                    payload_type = value
            effective_type = payload_type or self._event_field
            terminal = TERMINAL_TYPES.get(effective_type)
        event = SseEvent(
            event_field=self._event_field,
            data_text=data_text,
            data_json=data_json,
            json_error=json_error,
            payload_type=payload_type,
            type_conflict=bool(
                self._event_field
                and payload_type
                and self._event_field != payload_type
            ),
            terminal=terminal,
        )
        self._event_field = None
        self._data_lines.clear()
        return event


def parse_sse(data: bytes) -> list[SseEvent]:
    parser = IncrementalSseParser()
    events = parser.feed(data)
    parser.close()
    return events
