//! Dual-run contracts for R2 Static Generation IR request/response codecs.

use bytes::Bytes;
use openbridge::{
    bridge::{BridgeLimits, BridgePlan, StaticBridgePlan, StaticCodecError, StaticCodecLimits},
    core::{ApiProtocol, ReasoningOutput},
};
use serde_json::{Value, json};

fn body(value: Value) -> Bytes {
    Bytes::from(serde_json::to_vec(&value).expect("test JSON must serialize"))
}

fn limits() -> StaticCodecLimits {
    StaticCodecLimits::new(256 * 1024, 256 * 1024).expect("test limits must be valid")
}

fn bridge_limits() -> BridgeLimits {
    BridgeLimits::new(256 * 1024, 256 * 1024, 64 * 1024).expect("test Bridge limits must be valid")
}

#[test]
fn static_codecs_enforce_request_and_response_limits_before_decode() {
    let request = body(json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": "hello"}],
        "stream": false
    }));
    let request_limit = request.len().saturating_sub(1);
    let result = StaticBridgePlan::prepare(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        request.clone(),
        StaticCodecLimits::new(request_limit, 1024).expect("limits must be non-zero"),
    );
    assert!(matches!(result, Err(StaticCodecError::LimitExceeded)));

    let expanded_model = "m".repeat(request.len());
    let result = StaticBridgePlan::prepare(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        &expanded_model,
        request.clone(),
        StaticCodecLimits::new(request.len(), 1024).expect("limits must be non-zero"),
    );
    assert!(matches!(result, Err(StaticCodecError::LimitExceeded)));

    let response = body(json!({
        "id": "resp_limit",
        "object": "response",
        "status": "completed",
        "output": [{
            "id": "msg_limit",
            "type": "message",
            "role": "assistant",
            "status": "completed",
            "content": [{"type": "output_text", "text": "answer", "annotations": []}]
        }]
    }));
    let expanded_public_model = "p".repeat(response.len());
    let (plan, _) = StaticBridgePlan::prepare(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        &expanded_public_model,
        "upstream-model",
        request,
        StaticCodecLimits::new(1024, response.len()).expect("limits must be non-zero"),
    )
    .expect("request must fit its independent limit");
    assert_eq!(
        plan.render_non_stream(response),
        Err(StaticCodecError::LimitExceeded)
    );
}

fn assert_request_parity(
    source: ApiProtocol,
    target: ApiProtocol,
    request: Value,
    reasoning: ReasoningOutput,
) {
    let request = body(request);
    let (_, established) = BridgePlan::prepare_with_reasoning_output(
        source,
        target,
        "public-model",
        "upstream-model",
        request.clone(),
        reasoning,
        bridge_limits(),
    )
    .expect("established Bridge must accept the characterized request");
    let (static_plan, static_ir) = StaticBridgePlan::prepare_with_reasoning_output(
        source,
        target,
        "public-model",
        "upstream-model",
        request,
        reasoning,
        limits(),
    )
    .expect("Static IR codec must accept the characterized request");
    assert!(!static_plan.request_changes().is_empty());
    let established: Value = serde_json::from_slice(established.body()).unwrap();
    let static_ir: Value = serde_json::from_slice(static_ir.body()).unwrap();
    assert_eq!(static_ir, established);
}

#[test]
fn static_codecs_preserve_structured_reasoning_instruction_and_tool_semantics() {
    let schema = json!({
        "additionalProperties": false,
        "properties": {"answer": {"type": "string"}},
        "required": ["answer"],
        "type": "object"
    });
    assert_request_parity(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        json!({
            "messages": [
                {"content": "follow policy", "role": "developer"},
                {"content": "return JSON", "role": "user"}
            ],
            "model": "public-model",
            "response_format": {
                "json_schema": {
                    "description": "A short answer",
                    "name": "answer",
                    "schema": schema,
                    "strict": true
                },
                "type": "json_schema"
            },
            "stream": false
        }),
        ReasoningOutput::Unsupported,
    );
    assert_request_parity(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        json!({
            "input": [
                {"content": "lookup weather", "role": "user", "type": "message"},
                {
                    "content": [{"text": "decide lookup", "type": "reasoning_text"}],
                    "id": "rs_previous",
                    "status": "completed",
                    "summary": [],
                    "type": "reasoning"
                },
                {
                    "arguments": "{\"city\":\"Hangzhou\"}",
                    "call_id": "call_lookup",
                    "id": "fc_previous",
                    "name": "lookup",
                    "type": "function_call"
                }
            ],
            "model": "public-model",
            "reasoning": {"effort": "high", "summary": "auto"},
            "stream": false,
            "tools": [{"name": "lookup", "parameters": {"type": "object"}, "type": "function"}]
        }),
        ReasoningOutput::PlainText,
    );
    assert_request_parity(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        json!({
            "input": [
                {"content": "look up value", "role": "user"},
                {
                    "arguments": "{\"key\":\"value\"}",
                    "call_id": "call_lookup",
                    "id": "fc_lookup",
                    "name": "lookup",
                    "type": "function_call"
                },
                {"call_id": "call_lookup", "output": "{\"value\":42}", "type": "function_call_output"},
                {"content": "return DONE", "role": "user"}
            ],
            "model": "public-model",
            "stream": false,
            "tool_choice": "none",
            "tools": [{"name": "lookup", "parameters": {"type": "object"}, "type": "function"}]
        }),
        ReasoningOutput::Unsupported,
    );
}

#[test]
fn static_codecs_preserve_readable_non_stream_reasoning_and_usage() {
    let request = body(json!({
        "input": "hello",
        "model": "public-model",
        "reasoning": {"effort": "high", "summary": "auto"},
        "stream": false
    }));
    let (established, _) = BridgePlan::prepare_with_reasoning_output(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        request.clone(),
        ReasoningOutput::PlainText,
        bridge_limits(),
    )
    .unwrap();
    let (static_ir, _) = StaticBridgePlan::prepare_with_reasoning_output(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        request,
        ReasoningOutput::PlainText,
        limits(),
    )
    .unwrap();
    let upstream = Bytes::from_static(
        br#"{"choices":[{"finish_reason":"stop","index":0,"message":{"content":"answer","reasoning_content":"analysis","role":"assistant"}}],"id":"chatcmpl_reasoning","model":"upstream-model","object":"chat.completion","usage":{"completion_tokens":5,"completion_tokens_details":{"reasoning_tokens":2},"prompt_tokens":3,"prompt_tokens_details":{"cached_tokens":1},"total_tokens":8}}"#,
    );
    let established = established.render_non_stream(upstream.clone()).unwrap();
    let static_ir = static_ir.render_non_stream(upstream).unwrap();
    assert!(!static_ir.changes().is_empty());
    let established: Value = serde_json::from_slice(&established).unwrap();
    let static_ir: Value = serde_json::from_slice(static_ir.body()).unwrap();
    for field in ["input_tokens", "output_tokens", "total_tokens"] {
        assert_eq!(static_ir["usage"][field], established["usage"][field]);
    }
    assert_eq!(
        static_ir["usage"]["output_tokens_details"]["reasoning_tokens"],
        2
    );
    assert_eq!(
        static_ir["usage"]["input_tokens_details"]["cached_tokens"],
        1
    );

    let (chat_plan, _) = StaticBridgePlan::prepare(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        body(json!({
            "model": "public-model",
            "messages": [{"role": "user", "content": "hello"}],
            "stream": false
        })),
        limits(),
    )
    .expect("Chat request must lower to Responses");
    let chat = chat_plan
        .render_non_stream(body(json!({
            "id": "resp_usage",
            "object": "response",
            "status": "completed",
            "output": [{
                "id": "msg_usage",
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "content": [{"type": "output_text", "text": "answer", "annotations": []}]
            }],
            "usage": {
                "input_tokens": 3,
                "output_tokens": 5,
                "total_tokens": 8,
                "input_tokens_details": {"cached_tokens": 1},
                "output_tokens_details": {"reasoning_tokens": 2}
            }
        })))
        .expect("Responses usage details must lower to Chat");
    assert!(!chat.changes().is_empty());
    let chat: Value = serde_json::from_slice(chat.body()).unwrap();
    assert_eq!(
        chat["usage"]["completion_tokens_details"]["reasoning_tokens"],
        2
    );
    assert_eq!(chat["usage"]["prompt_tokens_details"]["cached_tokens"], 1);
}

#[test]
fn static_codecs_fail_closed_on_unmodeled_or_unresolved_semantics() {
    let cases = [
        (
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            json!({
                "include": ["reasoning.encrypted_content"],
                "input": "hello",
                "model": "public-model"
            }),
        ),
        (
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            json!({
                "input": [{"call_id": "call_missing", "output": "result", "type": "function_call_output"}],
                "model": "public-model"
            }),
        ),
        (
            ApiProtocol::ChatCompletions,
            ApiProtocol::Responses,
            json!({
                "messages": [{"content": "return JSON", "role": "user"}],
                "model": "public-model",
                "response_format": {"future": true, "type": "json_object"}
            }),
        ),
        (
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            json!({
                "input": [
                    {"type": "function_call", "id": "fc_1", "call_id": "call_1", "name": "lookup", "arguments": "{}"},
                    {"type": "function_call_output", "call_id": "call_1", "output": "first"},
                    {"type": "function_call_output", "call_id": "call_1", "output": "duplicate"}
                ],
                "model": "public-model"
            }),
        ),
        (
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            json!({
                "input": "hello",
                "model": "public-model",
                "text": {"verbosity": "high"}
            }),
        ),
        (
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            json!({
                "input": "hello",
                "model": "public-model",
                "reasoning": {"effort": "high", "future": true}
            }),
        ),
    ];
    for (source, target, request) in cases {
        let request = body(request);
        assert!(
            BridgePlan::prepare(
                source,
                target,
                "public-model",
                "upstream-model",
                request.clone(),
                bridge_limits(),
            )
            .is_err(),
            "established Bridge must reject the characterized loss"
        );
        assert!(
            StaticBridgePlan::prepare(
                source,
                target,
                "public-model",
                "upstream-model",
                request,
                limits(),
            )
            .is_err(),
            "Static IR codec must reject the characterized loss"
        );
    }

    // Static IR closes the same duplicate-result hole in the Chat request direction.
    let duplicate_chat_result = body(json!({
        "messages": [
            {"role": "assistant", "content": null, "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "lookup", "arguments": "{}"}}]},
            {"role": "tool", "tool_call_id": "call_1", "content": "first"},
            {"role": "tool", "tool_call_id": "call_1", "content": "duplicate"}
        ],
        "model": "public-model"
    }));
    assert!(
        StaticBridgePlan::prepare(
            ApiProtocol::ChatCompletions,
            ApiProtocol::Responses,
            "public-model",
            "upstream-model",
            duplicate_chat_result,
            limits(),
        )
        .is_err()
    );

    // Citations cannot disappear while source-item lowering remains unimplemented.
    let (plan, _) = StaticBridgePlan::prepare(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        body(json!({
            "model": "public-model",
            "messages": [{"role": "user", "content": "hello"}],
            "stream": false
        })),
        limits(),
    )
    .expect("request itself must be bridgeable");
    let annotated = body(json!({
        "id": "resp_annotated",
        "object": "response",
        "status": "completed",
        "output": [{
            "id": "msg_annotated",
            "type": "message",
            "role": "assistant",
            "status": "completed",
            "content": [{
                "type": "output_text",
                "text": "answer",
                "annotations": [{"type": "url_citation", "url": "https://example.com"}]
            }]
        }]
    }));
    assert!(plan.render_non_stream(annotated).is_err());

    let duplicate_responses_calls = body(json!({
        "id": "resp_duplicate_calls",
        "object": "response",
        "status": "completed",
        "output": [
            {"id": "fc_1", "type": "function_call", "status": "completed", "call_id": "call_same", "name": "lookup", "arguments": "{}"},
            {"id": "fc_2", "type": "function_call", "status": "completed", "call_id": "call_same", "name": "lookup", "arguments": "{}"}
        ]
    }));
    assert!(plan.render_non_stream(duplicate_responses_calls).is_err());

    let (chat_source_plan, _) = StaticBridgePlan::prepare(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        body(json!({"model": "public-model", "input": "hello", "stream": false})),
        limits(),
    )
    .expect("Responses request must lower to Chat");
    let duplicate_chat_calls = body(json!({
        "id": "chatcmpl_duplicate_calls",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "finish_reason": "tool_calls",
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [
                    {"id": "call_same", "type": "function", "function": {"name": "lookup", "arguments": "{}"}},
                    {"id": "call_same", "type": "function", "function": {"name": "lookup", "arguments": "{}"}}
                ]
            }
        }]
    }));
    assert!(
        chat_source_plan
            .render_non_stream(duplicate_chat_calls)
            .is_err()
    );

    let incomplete_item = body(json!({
        "id": "resp_incomplete",
        "object": "response",
        "status": "completed",
        "output": [{
            "id": "msg_incomplete",
            "type": "message",
            "role": "assistant",
            "status": "in_progress",
            "content": [{"type": "output_text", "text": "partial", "annotations": []}]
        }]
    }));
    assert!(plan.render_non_stream(incomplete_item).is_err());
}

#[test]
fn static_codecs_support_native_same_protocol_round_trips() {
    let cases = [
        (
            ApiProtocol::ChatCompletions,
            json!({
                "model": "public-model",
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            }),
            json!({
                "id": "chat-native",
                "object": "chat.completion",
                "model": "upstream-model",
                "choices": [{
                    "index": 0,
                    "message": {"role": "assistant", "content": "answer"},
                    "finish_reason": "stop"
                }]
            }),
        ),
        (
            ApiProtocol::Responses,
            json!({
                "model": "public-model",
                "input": [{"type": "message", "role": "user", "content": "hello"}],
                "stream": false
            }),
            json!({
                "id": "response-native",
                "object": "response",
                "status": "completed",
                "model": "upstream-model",
                "output": [{
                    "id": "message-native",
                    "type": "message",
                    "role": "assistant",
                    "status": "completed",
                    "content": [{"type": "output_text", "text": "answer", "annotations": []}]
                }]
            }),
        ),
    ];

    for (protocol, request, response) in cases {
        let (plan, upstream) = StaticBridgePlan::prepare(
            protocol,
            protocol,
            "public-model",
            "upstream-model",
            body(request),
            limits(),
        )
        .expect("Native request must pass through canonical Static IR");
        let upstream: Value = serde_json::from_slice(upstream.body()).unwrap();
        assert_eq!(upstream["model"], "upstream-model");

        let rendered = plan
            .render_non_stream(body(response))
            .expect("Native response must pass through canonical Static IR");
        let rendered: Value = serde_json::from_slice(rendered.body()).unwrap();
        assert_eq!(rendered["model"], "upstream-model");
    }
}

#[test]
fn cross_protocol_request_rejects_unrepresented_nested_fields() {
    let chat_cases = [
        (
            json!({"role": "user", "content": "hello", "name": "alice"}),
            "name",
            json!("alice"),
        ),
        (
            json!({"role": "assistant", "content": null, "audio": {"id": "audio_previous"}}),
            "audio",
            json!({"id": "audio_previous"}),
        ),
        (
            json!({"role": "assistant", "content": null, "refusal": "declined"}),
            "refusal",
            json!("declined"),
        ),
    ];
    for (message, field, expected) in chat_cases {
        let chat = json!({"model": "public-model", "messages": [message]});
        assert!(
            StaticBridgePlan::prepare(
                ApiProtocol::ChatCompletions,
                ApiProtocol::Responses,
                "public-model",
                "upstream-model",
                body(chat.clone()),
                limits(),
            )
            .is_err()
        );
        let (_, native) = StaticBridgePlan::prepare(
            ApiProtocol::ChatCompletions,
            ApiProtocol::ChatCompletions,
            "public-model",
            "upstream-model",
            body(chat),
            limits(),
        )
        .expect("Native PreserveSource may retain a recognized nested field");
        let native: Value = serde_json::from_slice(native.body()).unwrap();
        assert_eq!(native["messages"][0][field], expected);
    }

    let chat_tool_extension = json!({
        "model": "public-model",
        "messages": [{"role": "user", "content": "hello"}],
        "tools": [{
            "type": "function",
            "function": {
                "name": "lookup",
                "parameters": {"type": "object"},
                "cache_control": {"type": "ephemeral"}
            }
        }]
    });
    assert!(
        StaticBridgePlan::prepare(
            ApiProtocol::ChatCompletions,
            ApiProtocol::Responses,
            "public-model",
            "upstream-model",
            body(chat_tool_extension),
            limits(),
        )
        .is_err()
    );

    let responses = json!({
        "model": "public-model",
        "input": [{
            "type": "message",
            "role": "user",
            "status": "completed",
            "content": [{"type": "input_text", "text": "hello"}]
        }]
    });
    assert!(
        StaticBridgePlan::prepare(
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "public-model",
            "upstream-model",
            body(responses.clone()),
            limits(),
        )
        .is_err()
    );
    let (_, native) = StaticBridgePlan::prepare(
        ApiProtocol::Responses,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        body(responses),
        limits(),
    )
    .expect("Native PreserveSource may retain a recognized nested field");
    let native: Value = serde_json::from_slice(native.body()).unwrap();
    assert_eq!(native["input"][0]["status"], "completed");

    let responses_shorthand_extension = json!({
        "model": "public-model",
        "input": [{
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "hello",
                "unrepresented": true
            }]
        }]
    });
    assert!(
        StaticBridgePlan::prepare(
            ApiProtocol::Responses,
            ApiProtocol::ChatCompletions,
            "public-model",
            "upstream-model",
            body(responses_shorthand_extension),
            limits(),
        )
        .is_err()
    );
}

#[test]
fn static_response_usage_rejects_malformed_detail_containers() {
    let (responses_plan, _) = StaticBridgePlan::prepare(
        ApiProtocol::ChatCompletions,
        ApiProtocol::Responses,
        "public-model",
        "upstream-model",
        body(json!({"model": "public-model", "messages": [{"role": "user", "content": "hello"}]})),
        limits(),
    )
    .unwrap();
    for field in ["output_tokens_details", "input_tokens_details"] {
        for malformed in [json!([]), Value::Null] {
            let mut usage = json!({"input_tokens": 1, "output_tokens": 1, "total_tokens": 2});
            usage[field] = malformed;
            let response = body(json!({
                "id": "resp_usage",
                "object": "response",
                "status": "completed",
                "output": [{
                    "id": "msg_usage",
                    "type": "message",
                    "role": "assistant",
                    "status": "completed",
                    "content": [{"type": "output_text", "text": "ok", "annotations": []}]
                }],
                "usage": usage
            }));
            assert!(
                responses_plan.render_non_stream(response).is_err(),
                "{field}"
            );
        }
    }

    let (chat_plan, _) = StaticBridgePlan::prepare(
        ApiProtocol::Responses,
        ApiProtocol::ChatCompletions,
        "public-model",
        "upstream-model",
        body(json!({"model": "public-model", "input": "hello"})),
        limits(),
    )
    .unwrap();
    for field in ["completion_tokens_details", "prompt_tokens_details"] {
        for malformed in [json!(false), Value::Null] {
            let mut usage = json!({"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2});
            usage[field] = malformed;
            let response = body(json!({
                "id": "chatcmpl-usage",
                "object": "chat.completion",
                "choices": [{
                    "index": 0,
                    "message": {"role": "assistant", "content": "ok"},
                    "finish_reason": "stop"
                }],
                "usage": usage
            }));
            assert!(chat_plan.render_non_stream(response).is_err(), "{field}");
        }
    }
}
