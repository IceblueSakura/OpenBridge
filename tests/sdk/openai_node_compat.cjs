const OpenAI = require("openai");

const baseURL = `${process.argv[2].replace(/\/$/, "")}/v1`;
const client = new OpenAI({ apiKey: "downstream-token", baseURL });

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
})();
