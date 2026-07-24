import sys

from openai import OpenAI

base_url = sys.argv[1].rstrip("/")
client = OpenAI(api_key="downstream-token", base_url=f"{base_url}/v1", max_retries=0)

chat = client.chat.completions.create(
    model="public-model",
    messages=[{"role": "user", "content": "hello"}],
)
assert chat.choices[0].message.content == "hello"

chat_events = list(
    client.chat.completions.create(
        model="public-model",
        messages=[{"role": "user", "content": "hello"}],
        stream=True,
    )
)
assert [event.choices[0].delta.content for event in chat_events[:-1]] == ["hé", "llo"]
assert chat_events[-1].choices[0].finish_reason == "stop"

response = client.responses.create(model="public-model", input="hello")
assert response.id == "resp_nonstream"

response_events = list(
    client.responses.create(model="public-model", input="hello", stream=True)
)
assert [event.type for event in response_events] == [
    "response.output_text.delta",
    "response.completed",
]
assert response_events[0].delta == "héllo"
assert response_events[-1].response.id == "resp_stream"

tools = [
    {
        "type": "function",
        "function": {
            "name": "get_weather",
            "description": "Return a deterministic weather fixture.",
            "parameters": {
                "type": "object",
                "properties": {"city": {"type": "string"}},
                "required": ["city"],
                "additionalProperties": False,
            },
        },
    }
]

chat_tool_call = client.chat.completions.create(
    model="public-model",
    messages=[{"role": "user", "content": "weather"}],
    tools=tools,
)
chat_call = chat_tool_call.choices[0].message.tool_calls[0]
assert chat_tool_call.choices[0].finish_reason == "tool_calls"
assert chat_call.id == "call_sdk_chat_1"
assert chat_call.function.name == "get_weather"
assert chat_call.function.arguments == '{"city":"Shanghai"}'

chat_tool_result = client.chat.completions.create(
    model="public-model",
    messages=[
        {"role": "user", "content": "weather"},
        {
            "role": "assistant",
            "content": None,
            "tool_calls": [
                {
                    "id": chat_call.id,
                    "type": "function",
                    "function": {
                        "name": chat_call.function.name,
                        "arguments": chat_call.function.arguments,
                    },
                }
            ],
        },
        {"role": "tool", "tool_call_id": chat_call.id, "content": "sunny"},
    ],
    tools=tools,
)
assert chat_tool_result.choices[0].message.content == "hello"

parallel_chat_tools = [
    *tools,
    {
        "type": "function",
        "function": {
            "name": "get_time",
            "description": "Return a deterministic time fixture.",
            "parameters": {
                "type": "object",
                "properties": {"zone": {"type": "string"}},
                "required": ["zone"],
                "additionalProperties": False,
            },
        },
    },
]
parallel_chat_tool_call = client.chat.completions.create(
    model="public-model",
    messages=[{"role": "user", "content": "weather and time"}],
    tools=parallel_chat_tools,
)
parallel_chat_calls = parallel_chat_tool_call.choices[0].message.tool_calls
assert [(call.id, call.function.name, call.function.arguments) for call in parallel_chat_calls] == [
    ("call_sdk_chat_1", "get_weather", '{"city":"Shanghai"}'),
    ("call_sdk_chat_2", "get_time", '{"zone":"Asia/Shanghai"}'),
]
parallel_chat_tool_result = client.chat.completions.create(
    model="public-model",
    messages=[
        {"role": "user", "content": "weather and time"},
        {
            "role": "assistant",
            "content": None,
            "tool_calls": [
                {
                    "id": call.id,
                    "type": "function",
                    "function": {
                        "name": call.function.name,
                        "arguments": call.function.arguments,
                    },
                }
                for call in parallel_chat_calls
            ],
        },
        {"role": "tool", "tool_call_id": parallel_chat_calls[0].id, "content": "sunny"},
        {"role": "tool", "tool_call_id": parallel_chat_calls[1].id, "content": "12:00"},
    ],
    tools=parallel_chat_tools,
)
assert parallel_chat_tool_result.choices[0].message.content == "hello"

chat_tool_events = list(
    client.chat.completions.create(
        model="public-model",
        messages=[{"role": "user", "content": "weather"}],
        tools=tools,
        stream=True,
    )
)
tool_deltas = [
    delta
    for event in chat_tool_events
    for delta in (event.choices[0].delta.tool_calls or [])
]
assert tool_deltas[0].id == "call_sdk_chat_1"
assert tool_deltas[0].function.name == "get_weather"
assert "".join(delta.function.arguments or "" for delta in tool_deltas) == '{"city":"Shanghai"}'
assert chat_tool_events[-1].choices[0].finish_reason == "tool_calls"

response_tools = [
    {
        "type": "function",
        "name": "get_weather",
        "description": "Return a deterministic weather fixture.",
        "parameters": {
            "type": "object",
            "properties": {"city": {"type": "string"}},
            "required": ["city"],
            "additionalProperties": False,
        },
    }
]

response_tool_call = client.responses.create(
    model="public-model", input="weather", tools=response_tools
)
response_call = next(item for item in response_tool_call.output if item.type == "function_call")
assert response_call.call_id == "call_sdk_response_1"
assert response_call.name == "get_weather"
assert response_call.arguments == '{"city":"Shanghai"}'

response_tool_result = client.responses.create(
    model="public-model",
    input=[{"type": "function_call_output", "call_id": response_call.call_id, "output": "sunny"}],
    tools=response_tools,
)
assert response_tool_result.id == "resp_tool_result"

parallel_response_tools = [
    *response_tools,
    {
        "type": "function",
        "name": "get_time",
        "description": "Return a deterministic time fixture.",
        "parameters": {
            "type": "object",
            "properties": {"zone": {"type": "string"}},
            "required": ["zone"],
            "additionalProperties": False,
        },
    },
]
parallel_response_tool_call = client.responses.create(
    model="public-model", input="weather and time", tools=parallel_response_tools
)
parallel_response_calls = [
    item for item in parallel_response_tool_call.output if item.type == "function_call"
]
assert [(call.call_id, call.name, call.arguments) for call in parallel_response_calls] == [
    ("call_sdk_response_1", "get_weather", '{"city":"Shanghai"}'),
    ("call_sdk_response_2", "get_time", '{"zone":"Asia/Shanghai"}'),
]
parallel_response_tool_result = client.responses.create(
    model="public-model",
    input=[
        {"type": "function_call_output", "call_id": parallel_response_calls[0].call_id, "output": "sunny"},
        {"type": "function_call_output", "call_id": parallel_response_calls[1].call_id, "output": "12:00"},
    ],
    tools=parallel_response_tools,
)
assert parallel_response_tool_result.id == "resp_tool_result"

response_tool_events = list(
    client.responses.create(
        model="public-model", input="weather", tools=response_tools, stream=True
    )
)
assert [event.type for event in response_tool_events] == [
    "response.output_item.added",
    "response.function_call_arguments.delta",
    "response.function_call_arguments.delta",
    "response.function_call_arguments.done",
    "response.output_item.done",
    "response.completed",
]
assert "".join(
    event.delta
    for event in response_tool_events
    if event.type == "response.function_call_arguments.delta"
) == '{"city":"Shanghai"}'
assert response_tool_events[-1].response.id == "resp_tool_stream"

try:
    client.responses.create(model="public-model", input="trigger-upstream-error")
except Exception as error:
    assert getattr(error, "status_code", None) == 429
    assert getattr(error, "code", None) == "sdk_fixture_rate_limited"
else:
    raise AssertionError("fixture 429 was not decoded as an SDK error")
