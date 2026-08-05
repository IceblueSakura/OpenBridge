"""Discover and call the checked-in Embeddings interface over loopback HTTP."""

from __future__ import annotations

import json
import sys
import urllib.request
from typing import Any


def _json_request(
    base_url: str,
    api_key: str,
    path: str,
    body: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Send one authenticated JSON request and return its decoded object response."""
    payload = None if body is None else json.dumps(body).encode("utf-8")
    request = urllib.request.Request(
        f"{base_url.rstrip('/')}{path}",
        data=payload,
        method="GET" if body is None else "POST",
        headers={
            "Authorization": f"Bearer {api_key}",
            **({"Content-Type": "application/json"} if body is not None else {}),
        },
    )
    with urllib.request.urlopen(request, timeout=5) as response:
        assert response.status == 200
        assert response.headers.get_content_type() == "application/json"
        document = json.load(response)
    assert isinstance(document, dict)
    return document


def main() -> None:
    """Discover the fixed model contract, execute one request, and emit a safe summary."""
    base_url, api_key = sys.argv[1:3]

    # Discover the public contract without relying on Provider or Route topology.
    models = _json_request(base_url, api_key, "/openbridge/v1/models")
    model = next(item for item in models["data"] if item["id"] == "embedding-primary")
    interface = model["interfaces"]["embeddings"]
    assert model["interfaces"]["chat_completions"] is None
    assert model["interfaces"]["responses"] is None
    assert interface["input_forms"] == [
        "string",
        "string_array",
        "token_array",
        "token_array_array",
    ]
    assert interface["encoding"] == {
        "default": "float",
        "allowed": ["float", "base64"],
    }
    assert interface["dimensions"] == {"default": 1536, "allowed": None}
    assert 0 < interface["limits"]["max_inputs"] < 2048
    assert interface["supported_parameters"] == ["encoding_format", "user"]

    # Call only fields published by that same interface and validate the projected result.
    result = _json_request(
        base_url,
        api_key,
        "/v1/embeddings",
        {
            "model": "embedding-primary",
            "input": ["alpha", "beta"],
            "encoding_format": "float",
            "user": "synthetic-loopback-user",
        },
    )
    assert result["object"] == "list"
    assert result["model"] == "embedding-primary"
    assert result["usage"] == {"prompt_tokens": 2, "total_tokens": 2}
    assert [item["index"] for item in result["data"]] == [0, 1]
    assert all(item["object"] == "embedding" for item in result["data"])
    assert all(len(item["embedding"]) == 1536 for item in result["data"])
    assert result["data"][0]["embedding"][0] == 0.25
    assert result["data"][1]["embedding"][-1] == -0.5

    # Emit only low-sensitivity contract facts, never request inputs or vectors.
    print(
        json.dumps(
            {
                "default_dimensions": interface["dimensions"]["default"],
                "discovered_model": model["id"],
                "encoding": interface["encoding"]["default"],
                "vectors": len(result["data"]),
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
