import sys

from openai import OpenAI

base_url = sys.argv[1].rstrip("/")
client = OpenAI(api_key="downstream-token", base_url=f"{base_url}/v1")

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
