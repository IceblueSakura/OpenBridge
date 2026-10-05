# Standard Speech audio profile

This profile owns the independent `POST /v1/audio/speech` audio branch, not Chat audio, Responses audio or Realtime. Field shapes and bounds belong to the [codec](../../src/protocol/openai/speech.rs), [task](../../src/semantic/task/speech_synthesis.rs) and [OpenAPI](../openapi.json); HTTP activation belongs to the [Gateway contract](../http-gateway.md#独立语音生成).

## Sources and ownership

- Official [Create speech](https://developers.openai.com/api/reference/resources/audio/subresources/speech/methods/create) and [Text to speech](https://developers.openai.com/api/docs/guides/text-to-speech) define the operation and encoding vocabulary.
- [Fixed SDK sources](../references/upstream-sync.md) apply: [request parameters](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/types/audio/speech_create_params.py) and [binary consumer](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/resources/audio/speech.py). SDK permissiveness does not expand this profile.
- `SpeechRequest` owns text, named voice and synthesis intent. `AudioArtifact` owns bounded encoded bytes and optional reported encoding. The adapter envelope owns public model and explicit audio delivery; target/auth/HTTP state stays outside task data.

## Values and target admission

Speech is a separate task, not a Generation message with fabricated transcript, reference or expiry. The existing Chat audio value and closure checks remain unchanged. Shared encoding values do not imply equivalent Chat controls or replay.

Absent controls, explicit defaults and empty instructions remain distinct. Null controls, unknown fields, custom voice ID objects and SSE are rejected. Speed retains exact decimal value; admission cannot round an out-of-range value across a boundary. Typed construction and edits are validated again at projection. No source JSON can restore removed instructions or controls.

[Target capabilities](../../src/lowering/speech.rs) explicitly select voices, formats, instructions and speed. Even an empty explicit instruction or default-valued explicit speed requires support. A missing requested format means MP3, not an arbitrary target default. A single target starts from immutable input; unsupported controls fail before I/O rather than being dropped.

## OpenRouter MP3 target mapping

The trusted `OpenRouterMp3` profile follows the [OpenRouter Speech operation](https://openrouter.ai/docs/api/api-reference/tts/create-speech.md) and [TTS guide](https://openrouter.ai/docs/guides/overview/multimodal/tts), not an assumption that every OpenAI field is accepted. The [codec](../../src/protocol/openrouter_speech.rs) and [compiled binding](../../src/topology/catalog/speech.rs) own wire and product details respectively; the public client remains standard Speech.

OpenRouter defaults to PCM, so this profile always encodes effective MP3 explicitly, including when the standard client omitted its format. Explicit `stream_format: audio` selects the same binary operation and needs no upstream field. These are bounded equivalent mappings from final typed values, not request mutation or semantic loss.

Only MP3 and explicitly bound named voices are admitted. Instructions and speed, including empty instructions and explicit default speed, fail before I/O; model-native support or a provider-specific passthrough example does not establish an admitted carrier. No arbitrary `provider`, voice references, account context or other private client fields are accepted. PCM is deferred rather than guessing its sample layout; WAV and the other standard formats remain available only to independently supporting targets.

Local execution uses one fixed target and credential source, without retry/fallback. This does not attest the aggregator's internal routing, retries, account qualification or remote completion.

## Binary reports and closure

Only the selected profile interprets response media types. `application/octet-stream` does not report an encoding; request format must not fill it. Admitted format-specific MIME reports must agree with effective request format. The bounded `audio/x-wav` alias projects to `audio/wav`; PCM layout is supplied by the standard profile, not a universal interpretation of `audio/pcm`. Parameterized, absent, duplicate or conflicting media types and content encoding are rejected.

The I/O owner bounds cumulative bytes, checks declared lengths and read errors, and waits for strict transport EOF before publication. HTTP chunking is accepted but does not become semantic audio events. Handoff, cancellation, deadlines and completion reuse the [execution contract](execution-model.md); no retries or post-publication replacement occur.

Encoding labels and EOF do not prove valid frames, decodability, exact spoken text or audio quality. Codec and Gateway do not sniff, decode, transcode, play, retain or fetch audio. Binary output carries no standard usage report here; no count or cost is fabricated, and no accounting-loss rule is inherited from Images.

## Deferred boundaries

Low-latency binary delivery, SSE events, transcription, custom voices/consent, resource IDs/history and Realtime require separate contracts. A registered standard carrier does not activate a model, establish account permissions or bound remote fees. Consumer verification uses synthetic bytes; real Provider and quality checks require their own evidence and authorization.
