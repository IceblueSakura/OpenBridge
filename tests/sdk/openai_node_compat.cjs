const OpenAI = require("openai");

const baseURL = `${process.argv[2].replace(/\/$/, "")}/v1`;
const client = new OpenAI({ apiKey: "downstream-token", baseURL, maxRetries: 0 });

(async () => {
  const chat = await client.chat.completions.create({
    model: "public-model",
    messages: [{ role: "user", content: "hello" }],
  });
  if (chat.choices[0].message.content !== "hello") {
    throw new Error("non-stream Chat completion was not decoded");
  }

  const chatStream = await client.chat.completions.create({
    model: "public-model",
    messages: [{ role: "user", content: "hello" }],
    stream: true,
  });
  const chatEvents = [];
  for await (const event of chatStream) {
    chatEvents.push(event);
  }
  if (chatEvents.map((event) => event.choices[0].delta.content ?? "").join("") !== "héllo") {
    throw new Error("stream Chat completion deltas were not decoded");
  }
  if (chatEvents.at(-1).choices[0].finish_reason !== "stop") {
    throw new Error("stream Chat completion terminal event was not decoded");
  }

  const response = await client.responses.create({ model: "public-model", input: "hello" });
  if (response.id !== "resp_nonstream") {
    throw new Error("non-stream Response was not decoded");
  }

  const responseStream = await client.responses.create({
    model: "public-model",
    input: "hello",
    stream: true,
  });
  const responseEvents = [];
  for await (const event of responseStream) {
    responseEvents.push(event);
  }
  if (responseEvents.map((event) => event.type).join(",") !== "response.output_text.delta,response.completed") {
    throw new Error("stream Response events were not decoded");
  }
  if (responseEvents[0].delta !== "héllo" || responseEvents.at(-1).response.id !== "resp_stream") {
    throw new Error("stream Response payload was not decoded");
  }

  const chatTools = [
    {
      type: "function",
      function: {
        name: "get_weather",
        description: "Return a deterministic weather fixture.",
        parameters: {
          type: "object",
          properties: { city: { type: "string" } },
          required: ["city"],
          additionalProperties: false,
        },
      },
    },
  ];
  const chatToolCall = await client.chat.completions.create({
    model: "public-model",
    messages: [{ role: "user", content: "weather" }],
    tools: chatTools,
  });
  const chatCall = chatToolCall.choices[0].message.tool_calls[0];
  if (
    chatToolCall.choices[0].finish_reason !== "tool_calls" ||
    chatCall.id !== "call_sdk_chat_1" ||
    chatCall.function.name !== "get_weather" ||
    chatCall.function.arguments !== '{"city":"Shanghai"}'
  ) {
    throw new Error("non-stream Chat tool call was not decoded");
  }
  const chatToolResult = await client.chat.completions.create({
    model: "public-model",
    messages: [
      { role: "user", content: "weather" },
      {
        role: "assistant",
        content: null,
        tool_calls: [
          {
            id: chatCall.id,
            type: "function",
            function: {
              name: chatCall.function.name,
              arguments: chatCall.function.arguments,
            },
          },
        ],
      },
      { role: "tool", tool_call_id: chatCall.id, content: "sunny" },
    ],
    tools: chatTools,
  });
  if (chatToolResult.choices[0].message.content !== "hello") {
    throw new Error("Chat tool result replay was not decoded");
  }
  const parallelChatTools = [
    ...chatTools,
    {
      type: "function",
      function: {
        name: "get_time",
        description: "Return a deterministic time fixture.",
        parameters: {
          type: "object",
          properties: { zone: { type: "string" } },
          required: ["zone"],
          additionalProperties: false,
        },
      },
    },
  ];
  const parallelChatToolCall = await client.chat.completions.create({
    model: "public-model",
    messages: [{ role: "user", content: "weather and time" }],
    tools: parallelChatTools,
  });
  const parallelChatCalls = parallelChatToolCall.choices[0].message.tool_calls ?? [];
  if (
    JSON.stringify(parallelChatCalls.map((call) => [call.id, call.function.name, call.function.arguments])) !==
      JSON.stringify([
        ["call_sdk_chat_1", "get_weather", '{"city":"Shanghai"}'],
        ["call_sdk_chat_2", "get_time", '{"zone":"Asia/Shanghai"}'],
      ])
  ) {
    throw new Error("parallel Chat tool calls were not decoded");
  }
  const parallelChatToolResult = await client.chat.completions.create({
    model: "public-model",
    messages: [
      { role: "user", content: "weather and time" },
      {
        role: "assistant",
        content: null,
        tool_calls: parallelChatCalls.map((call) => ({
          id: call.id,
          type: "function",
          function: { name: call.function.name, arguments: call.function.arguments },
        })),
      },
      { role: "tool", tool_call_id: parallelChatCalls[0].id, content: "sunny" },
      { role: "tool", tool_call_id: parallelChatCalls[1].id, content: "12:00" },
    ],
    tools: parallelChatTools,
  });
  if (parallelChatToolResult.choices[0].message.content !== "hello") {
    throw new Error("parallel Chat tool result replay was not decoded");
  }
  const chatToolStream = await client.chat.completions.create({
    model: "public-model",
    messages: [{ role: "user", content: "weather" }],
    tools: chatTools,
    stream: true,
  });
  const chatToolEvents = [];
  for await (const event of chatToolStream) {
    chatToolEvents.push(event);
  }
  const chatToolDeltas = chatToolEvents.flatMap(
    (event) => event.choices[0].delta.tool_calls ?? [],
  );
  if (
    chatToolDeltas[0].id !== "call_sdk_chat_1" ||
    chatToolDeltas[0].function.name !== "get_weather" ||
    chatToolDeltas.map((delta) => delta.function.arguments ?? "").join("") !==
      '{"city":"Shanghai"}' ||
    chatToolEvents.at(-1).choices[0].finish_reason !== "tool_calls"
  ) {
    throw new Error("stream Chat tool call fragments were not decoded");
  }

  const responseTools = [
    {
      type: "function",
      name: "get_weather",
      description: "Return a deterministic weather fixture.",
      parameters: {
        type: "object",
        properties: { city: { type: "string" } },
        required: ["city"],
        additionalProperties: false,
      },
    },
  ];
  const responseToolCall = await client.responses.create({
    model: "public-model",
    input: "weather",
    tools: responseTools,
  });
  const responseCall = responseToolCall.output.find((item) => item.type === "function_call");
  if (
    responseCall?.call_id !== "call_sdk_response_1" ||
    responseCall.name !== "get_weather" ||
    responseCall.arguments !== '{"city":"Shanghai"}'
  ) {
    throw new Error("non-stream Response tool call was not decoded");
  }
  const responseToolResult = await client.responses.create({
    model: "public-model",
    input: [{ type: "function_call_output", call_id: responseCall.call_id, output: "sunny" }],
    tools: responseTools,
  });
  if (responseToolResult.id !== "resp_tool_result") {
    throw new Error("Response tool result replay was not decoded");
  }
  const parallelResponseTools = [
    ...responseTools,
    {
      type: "function",
      name: "get_time",
      description: "Return a deterministic time fixture.",
      parameters: {
        type: "object",
        properties: { zone: { type: "string" } },
        required: ["zone"],
        additionalProperties: false,
      },
    },
  ];
  const parallelResponseToolCall = await client.responses.create({
    model: "public-model",
    input: "weather and time",
    tools: parallelResponseTools,
  });
  const parallelResponseCalls = parallelResponseToolCall.output.filter(
    (item) => item.type === "function_call",
  );
  if (
    JSON.stringify(parallelResponseCalls.map((call) => [call.call_id, call.name, call.arguments])) !==
      JSON.stringify([
        ["call_sdk_response_1", "get_weather", '{"city":"Shanghai"}'],
        ["call_sdk_response_2", "get_time", '{"zone":"Asia/Shanghai"}'],
      ])
  ) {
    throw new Error("parallel Response tool calls were not decoded");
  }
  const parallelResponseToolResult = await client.responses.create({
    model: "public-model",
    input: [
      { type: "function_call_output", call_id: parallelResponseCalls[0].call_id, output: "sunny" },
      { type: "function_call_output", call_id: parallelResponseCalls[1].call_id, output: "12:00" },
    ],
    tools: parallelResponseTools,
  });
  if (parallelResponseToolResult.id !== "resp_tool_result") {
    throw new Error("parallel Response tool result replay was not decoded");
  }
  const responseToolStream = await client.responses.create({
    model: "public-model",
    input: "weather",
    tools: responseTools,
    stream: true,
  });
  const responseToolEvents = [];
  for await (const event of responseToolStream) {
    responseToolEvents.push(event);
  }
  if (
    responseToolEvents.map((event) => event.type).join(",") !==
      "response.output_item.added,response.function_call_arguments.delta,response.function_call_arguments.delta,response.function_call_arguments.done,response.output_item.done,response.completed" ||
    responseToolEvents
      .filter((event) => event.type === "response.function_call_arguments.delta")
      .map((event) => event.delta)
      .join("") !== '{"city":"Shanghai"}' ||
    responseToolEvents.at(-1).response.id !== "resp_tool_stream"
  ) {
    throw new Error("stream Response tool call fragments were not decoded");
  }
  try {
    await client.responses.create({ model: "public-model", input: "trigger-upstream-error" });
    throw new Error("fixture 429 was not decoded as an SDK error");
  } catch (error) {
    if (error.status !== 429 || error.code !== "sdk_fixture_rate_limited") {
      throw new Error("fixture 429 did not preserve SDK error details");
    }
  }
})();
