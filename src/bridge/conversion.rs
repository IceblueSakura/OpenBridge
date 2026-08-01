//! 受限 Chat Completions 与 Responses 双向转换计划和 wire renderer。
//!
//! 本模块只转换两种 OpenAI-compatible 协议共同可表达的 text 与 function tool 语义。
//! Provider 私有扩展、hosted/custom tool、opaque continuation、reasoning、image、structured
//! output 与后台状态均在出站前拒绝，避免静默丢失字段。

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use bytes::Bytes;
use serde_json::{Map, Value, json};
use thiserror::Error;

use crate::{
    core::{ApiProtocol, ApiRequest},
    transport::sse::SseEvent,
};

use super::{BridgeStreamError, ChatStreamState, ResponsesStreamState};

/// 请求、响应或 stream 无法按受限 Bridge 契约转换时返回的错误。
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BridgeError {
    /// 输入不是符合方向要求的 JSON object。
    #[error("bridge input is not a valid protocol object")]
    InvalidShape,
    /// 输入使用了 Bridge 未声明支持的语义。
    #[error("bridge input uses unsupported semantics")]
    UnsupportedSemantics,
    /// function call/result identity 缺失、重复或无法关联。
    #[error("bridge tool identity is invalid")]
    InvalidToolIdentity,
    /// function arguments 不是闭合 JSON object。
    #[error("bridge function arguments are invalid")]
    InvalidToolArguments,
    /// 上游 stream 生命周期失败。
    #[error("bridge stream lifecycle is invalid")]
    InvalidStream,
}

impl From<BridgeStreamError> for BridgeError {
    fn from(_: BridgeStreamError) -> Self {
        Self::InvalidStream
    }
}

/// 一条已经固定转换方向、Public Model 和上游模型的执行计划。
#[derive(Clone, Debug)]
pub struct BridgePlan {
    downstream_protocol: ApiProtocol,
    upstream_protocol: ApiProtocol,
    public_model: String,
}

impl BridgePlan {
    /// 校验并转换下游请求，返回不可变计划与上游协议请求。
    pub fn prepare(
        downstream_protocol: ApiProtocol,
        upstream_protocol: ApiProtocol,
        public_model: &str,
        upstream_model: &str,
        body: Bytes,
    ) -> Result<(Self, ApiRequest), BridgeError> {
        // 拒绝同协议调用和不受支持的扩展，再执行方向专用转换。
        if downstream_protocol == upstream_protocol {
            return Err(BridgeError::UnsupportedSemantics);
        }
        let source = parse_value_object(&body)?;
        reject_unsupported_request(downstream_protocol, &source)?;
        let converted = match (downstream_protocol, upstream_protocol) {
            (ApiProtocol::ChatCompletions, ApiProtocol::Responses) => {
                chat_request_to_responses(&source, upstream_model)?
            }
            (ApiProtocol::Responses, ApiProtocol::ChatCompletions) => {
                responses_request_to_chat(&source, upstream_model)?
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        };

        // 固化响应转换需要的下游事实，并把紧凑 JSON 交给 Provider adapter。
        let request = ApiRequest::new(
            upstream_protocol,
            Bytes::from(serde_json::to_vec(&converted).map_err(|_| BridgeError::InvalidShape)?),
        );
        Ok((
            Self {
                downstream_protocol,
                upstream_protocol,
                public_model: public_model.to_owned(),
            },
            request,
        ))
    }

    /// 返回计划的下游协议。
    pub fn downstream_protocol(&self) -> ApiProtocol {
        self.downstream_protocol
    }

    /// 返回计划实际调用的上游协议。
    pub fn upstream_protocol(&self) -> ApiProtocol {
        self.upstream_protocol
    }

    /// 将一个完整成功上游 JSON response 转换为下游协议。
    pub fn render_non_stream(&self, body: Bytes) -> Result<Bytes, BridgeError> {
        // 解析上游对象并按固定方向生成下游 response。
        let source = parse_value_object(&body)?;
        let converted = match (self.downstream_protocol, self.upstream_protocol) {
            (ApiProtocol::ChatCompletions, ApiProtocol::Responses) => {
                responses_response_to_chat(&source, &self.public_model)?
            }
            (ApiProtocol::Responses, ApiProtocol::ChatCompletions) => {
                chat_response_to_responses(&source, &self.public_model)?
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        };
        serde_json::to_vec(&converted)
            .map(Bytes::from)
            .map_err(|_| BridgeError::InvalidShape)
    }

    /// 创建只服务于本次请求的增量 SSE renderer。
    pub fn stream_renderer(&self) -> BridgeStreamRenderer {
        BridgeStreamRenderer::new(self.clone())
    }
}

/// 将上游完整 SSE event 增量渲染成下游协议 event。
pub struct BridgeStreamRenderer {
    plan: BridgePlan,
    state: StreamState,
}

enum StreamState {
    ResponsesToChat(ResponsesToChatStream),
    ChatToResponses(ChatToResponsesStream),
}

impl BridgeStreamRenderer {
    fn new(plan: BridgePlan) -> Self {
        let state = match (plan.downstream_protocol, plan.upstream_protocol) {
            (ApiProtocol::ChatCompletions, ApiProtocol::Responses) => {
                StreamState::ResponsesToChat(ResponsesToChatStream::new())
            }
            (ApiProtocol::Responses, ApiProtocol::ChatCompletions) => {
                StreamState::ChatToResponses(ChatToResponsesStream::new())
            }
            _ => unreachable!("BridgePlan always has opposite protocols"),
        };
        Self { plan, state }
    }

    /// 消费一个已完成 framing 的上游 event，并返回零个或多个下游 SSE event bytes。
    pub fn render(&mut self, event: SseEvent) -> Result<Bytes, BridgeError> {
        match &mut self.state {
            StreamState::ResponsesToChat(state) => state.render(event, &self.plan.public_model),
            StreamState::ChatToResponses(state) => state.render(event, &self.plan.public_model),
        }
    }

    /// 结束上游输入并确认显式 terminal 已经到达。
    pub fn finish(&mut self) -> Result<Bytes, BridgeError> {
        match &mut self.state {
            StreamState::ResponsesToChat(state) => state.finish(),
            StreamState::ChatToResponses(state) => state.finish(),
        }
    }
}

fn parse_value_object(body: &[u8]) -> Result<Map<String, Value>, BridgeError> {
    serde_json::from_slice::<Value>(body)
        .map_err(|_| BridgeError::InvalidShape)?
        .as_object()
        .cloned()
        .ok_or(BridgeError::InvalidShape)
}

fn reject_unsupported_request(
    protocol: ApiProtocol,
    source: &Map<String, Value>,
) -> Result<(), BridgeError> {
    // Bridge 与 Native Path 不同：任何未建模顶层字段都必须显式拒绝，不能转换时静默丢失。
    let allowed: &[&str] = match protocol {
        ApiProtocol::ChatCompletions => &[
            "max_completion_tokens",
            "max_tokens",
            "messages",
            "model",
            "parallel_tool_calls",
            "stream",
            "temperature",
            "tool_choice",
            "tools",
            "top_p",
        ],
        ApiProtocol::Responses => &[
            "input",
            "max_output_tokens",
            "model",
            "parallel_tool_calls",
            "stream",
            "temperature",
            "tool_choice",
            "tools",
            "top_p",
        ],
    };
    if source
        .keys()
        .any(|field| !allowed.contains(&field.as_str()))
    {
        return Err(BridgeError::UnsupportedSemantics);
    }

    // 拒绝需要状态亲和、异构语义或尚未建模转换策略的顶层字段。
    let unsupported = [
        "background",
        "conversation",
        "include",
        "instructions",
        "metadata",
        "modalities",
        "previous_response_id",
        "reasoning",
        "reasoning_effort",
        "response_format",
        "store",
        "text",
        "truncation",
    ];
    if unsupported.iter().any(|field| {
        source
            .get(*field)
            .is_some_and(|value| !value.is_null() && value != &Value::Bool(false))
    }) {
        return Err(BridgeError::UnsupportedSemantics);
    }
    if protocol == ApiProtocol::ChatCompletions && source.contains_key("functions") {
        return Err(BridgeError::UnsupportedSemantics);
    }

    // 只允许标准 function tool，避免 hosted/custom tool 被降级为普通函数。
    if source
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| {
            tools
                .iter()
                .any(|tool| tool.get("type").and_then(Value::as_str) != Some("function"))
        })
    {
        return Err(BridgeError::UnsupportedSemantics);
    }
    Ok(())
}

fn chat_request_to_responses(
    source: &Map<String, Value>,
    upstream_model: &str,
) -> Result<Value, BridgeError> {
    // 转换 Chat messages，并验证 tool call/result 的局部 identity ledger。
    let messages = source
        .get("messages")
        .and_then(Value::as_array)
        .ok_or(BridgeError::InvalidShape)?;
    let stream = source.get("stream").and_then(Value::as_bool) == Some(true);
    let tools_present = source.get("tools").is_some();
    let input = chat_messages_to_responses(messages, stream, tools_present)?;

    // 复制两协议共同字段，并转换 function schema 与输出 token 字段。
    let mut result = Map::new();
    result.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
    result.insert("input".to_owned(), input);
    result.insert("stream".to_owned(), Value::Bool(stream));
    copy_fields(
        source,
        &mut result,
        &["parallel_tool_calls", "temperature", "top_p"],
    );
    if let Some(max_tokens) = source
        .get("max_completion_tokens")
        .or_else(|| source.get("max_tokens"))
    {
        result.insert("max_output_tokens".to_owned(), max_tokens.clone());
    }
    if let Some(tools) = source.get("tools").and_then(Value::as_array) {
        result.insert(
            "tools".to_owned(),
            Value::Array(
                tools
                    .iter()
                    .map(chat_tool_to_responses)
                    .collect::<Result<_, _>>()?,
            ),
        );
    }
    if let Some(tool_choice) = source.get("tool_choice") {
        result.insert(
            "tool_choice".to_owned(),
            chat_tool_choice_to_responses(tool_choice)?,
        );
    }
    Ok(Value::Object(result))
}

fn chat_messages_to_responses(
    messages: &[Value],
    stream: bool,
    tools_present: bool,
) -> Result<Value, BridgeError> {
    // 对最常见的单条流式文本保留 Responses 简写，其他 history 使用显式 input items。
    if stream && messages.len() == 1 {
        let message = messages[0].as_object().ok_or(BridgeError::InvalidShape)?;
        if message.get("role").and_then(Value::as_str) == Some("user")
            && let Some(text) = message.get("content").and_then(Value::as_str)
        {
            return Ok(Value::String(text.to_owned()));
        }
    }

    let mut input = Vec::new();
    let mut known_calls = BTreeMap::<String, (String, String)>::new();
    let mut item_ids = BTreeSet::new();
    for message in messages {
        let message = message.as_object().ok_or(BridgeError::InvalidShape)?;
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .ok_or(BridgeError::InvalidShape)?;
        match role {
            "assistant" if message.get("tool_calls").is_some() => {
                if message
                    .get("content")
                    .is_some_and(|content| !content.is_null() && content.as_str() != Some(""))
                {
                    return Err(BridgeError::UnsupportedSemantics);
                }
                let calls = message
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .ok_or(BridgeError::InvalidShape)?;
                for call in calls {
                    let call = call.as_object().ok_or(BridgeError::InvalidShape)?;
                    let id = required_string(call, "id")?;
                    if call.get("type").and_then(Value::as_str) != Some("function") {
                        return Err(BridgeError::UnsupportedSemantics);
                    }
                    let function = call
                        .get("function")
                        .and_then(Value::as_object)
                        .ok_or(BridgeError::InvalidShape)?;
                    let name = required_string(function, "name")?;
                    let arguments = required_string(function, "arguments")?;
                    validate_arguments(&arguments)?;
                    if known_calls
                        .insert(id.clone(), (name.clone(), arguments.clone()))
                        .is_some()
                    {
                        return Err(BridgeError::InvalidToolIdentity);
                    }
                    let item_id =
                        allocate_non_stream_item_id(&id, known_calls.len(), &mut item_ids);
                    input.push(json!({
                        "arguments": arguments,
                        "call_id": id,
                        "id": item_id,
                        "name": name,
                        "type": "function_call"
                    }));
                }
            }
            "tool" => {
                let call_id = required_string(message, "tool_call_id")?;
                if !known_calls.contains_key(&call_id) {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                let output = message
                    .get("content")
                    .and_then(Value::as_str)
                    .ok_or(BridgeError::InvalidShape)?;
                input.push(json!({
                    "call_id": call_id,
                    "output": output,
                    "type": "function_call_output"
                }));
            }
            "user" | "assistant" | "system" | "developer" => {
                let content = message.get("content").ok_or(BridgeError::InvalidShape)?;
                let converted = chat_content_to_responses(content, tools_present)?;
                input.push(json!({"content": converted, "role": role, "type": "message"}));
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        }
    }
    Ok(Value::Array(input))
}

fn chat_content_to_responses(content: &Value, preserve_string: bool) -> Result<Value, BridgeError> {
    match content {
        Value::String(text) if preserve_string => Ok(Value::String(text.clone())),
        Value::String(text) => Ok(json!([{"text": text, "type": "input_text"}])),
        Value::Array(parts) => {
            let converted = parts
                .iter()
                .map(|part| {
                    let part = part.as_object().ok_or(BridgeError::InvalidShape)?;
                    if part.get("type").and_then(Value::as_str) != Some("text") {
                        return Err(BridgeError::UnsupportedSemantics);
                    }
                    Ok(json!({
                        "text": required_string(part, "text")?,
                        "type": "input_text"
                    }))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Array(converted))
        }
        _ => Err(BridgeError::UnsupportedSemantics),
    }
}

fn chat_tool_to_responses(tool: &Value) -> Result<Value, BridgeError> {
    let tool = tool.as_object().ok_or(BridgeError::InvalidShape)?;
    let function = tool
        .get("function")
        .and_then(Value::as_object)
        .ok_or(BridgeError::InvalidShape)?;
    let mut result = function.clone();
    result.insert("type".to_owned(), Value::String("function".to_owned()));
    Ok(Value::Object(result))
}

fn chat_tool_choice_to_responses(choice: &Value) -> Result<Value, BridgeError> {
    if choice
        .as_str()
        .is_some_and(|choice| matches!(choice, "auto" | "none" | "required"))
    {
        return Ok(choice.clone());
    }
    let choice = choice
        .as_object()
        .ok_or(BridgeError::UnsupportedSemantics)?;
    if choice.get("type").and_then(Value::as_str) != Some("function") {
        return Err(BridgeError::UnsupportedSemantics);
    }
    let function = choice
        .get("function")
        .and_then(Value::as_object)
        .ok_or(BridgeError::UnsupportedSemantics)?;
    Ok(json!({"name": required_string(function, "name")?, "type": "function"}))
}

fn responses_request_to_chat(
    source: &Map<String, Value>,
    upstream_model: &str,
) -> Result<Value, BridgeError> {
    // 将 Responses input 展开为 Chat messages，并校验 call/output ledger。
    let input = source.get("input").ok_or(BridgeError::InvalidShape)?;
    let messages = responses_input_to_chat(input)?;
    let stream = source.get("stream").and_then(Value::as_bool) == Some(true);

    // 复制共同字段，并把 flat function schema 包装为 Chat function 对象。
    let mut result = Map::new();
    result.insert("model".to_owned(), Value::String(upstream_model.to_owned()));
    result.insert("messages".to_owned(), Value::Array(messages));
    result.insert("stream".to_owned(), Value::Bool(stream));
    copy_fields(
        source,
        &mut result,
        &["parallel_tool_calls", "temperature", "top_p"],
    );
    if let Some(max_tokens) = source.get("max_output_tokens") {
        result.insert("max_completion_tokens".to_owned(), max_tokens.clone());
    }
    if let Some(tools) = source.get("tools").and_then(Value::as_array) {
        result.insert(
            "tools".to_owned(),
            Value::Array(
                tools
                    .iter()
                    .map(responses_tool_to_chat)
                    .collect::<Result<_, _>>()?,
            ),
        );
    }
    if let Some(tool_choice) = source.get("tool_choice") {
        result.insert(
            "tool_choice".to_owned(),
            responses_tool_choice_to_chat(tool_choice)?,
        );
    }
    Ok(Value::Object(result))
}

fn responses_input_to_chat(input: &Value) -> Result<Vec<Value>, BridgeError> {
    if let Some(text) = input.as_str() {
        return Ok(vec![json!({"content": text, "role": "user"})]);
    }
    let items = input.as_array().ok_or(BridgeError::InvalidShape)?;
    let mut messages = Vec::new();
    let mut calls = BTreeMap::<String, Value>::new();
    let mut call_order = Vec::new();
    let mut emitted_calls = false;
    let mut seen_results = BTreeSet::new();
    let mut item_ids = BTreeSet::new();

    // 先按 wire 顺序建立 call ledger，message/output 转换时保持原顺序。
    for item in items {
        let item = item.as_object().ok_or(BridgeError::InvalidShape)?;
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                if !calls.is_empty() && !emitted_calls {
                    messages.push(chat_assistant_tool_message(&call_order, &calls));
                    emitted_calls = true;
                }
                messages.push(responses_message_to_chat(item)?);
            }
            Some("function_call") => {
                if emitted_calls {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                let call_id = required_string(item, "call_id")?;
                let item_id = required_string(item, "id")?;
                if !item_ids.insert(item_id) {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                let name = required_string(item, "name")?;
                let arguments = required_string(item, "arguments")?;
                validate_arguments(&arguments)?;
                if calls
                    .insert(
                        call_id.clone(),
                        json!({
                            "function": {"arguments": arguments, "name": name},
                            "id": call_id,
                            "type": "function"
                        }),
                    )
                    .is_some()
                {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                call_order.push(call_id);
            }
            Some("function_call_output") => {
                if !emitted_calls {
                    if calls.is_empty() {
                        return Err(BridgeError::InvalidToolIdentity);
                    }
                    messages.push(chat_assistant_tool_message(&call_order, &calls));
                    emitted_calls = true;
                }
                let call_id = required_string(item, "call_id")?;
                if !calls.contains_key(&call_id) || !seen_results.insert(call_id.clone()) {
                    return Err(BridgeError::InvalidToolIdentity);
                }
                let output = item.get("output").ok_or(BridgeError::InvalidShape)?;
                let output = output
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| output.to_string());
                messages.push(json!({
                    "content": output,
                    "role": "tool",
                    "tool_call_id": call_id
                }));
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        }
    }
    if !calls.is_empty() && !emitted_calls {
        messages.push(chat_assistant_tool_message(&call_order, &calls));
    }
    Ok(messages)
}

fn chat_assistant_tool_message(order: &[String], calls: &BTreeMap<String, Value>) -> Value {
    json!({
        "content": Value::Null,
        "role": "assistant",
        "tool_calls": order.iter().filter_map(|id| calls.get(id)).cloned().collect::<Vec<_>>()
    })
}

fn responses_message_to_chat(item: &Map<String, Value>) -> Result<Value, BridgeError> {
    let role = required_string(item, "role")?;
    let content = item.get("content").ok_or(BridgeError::InvalidShape)?;
    let content = match content {
        Value::String(text) => Value::String(text.clone()),
        Value::Array(parts) => {
            let mut text = String::new();
            for part in parts {
                let part = part.as_object().ok_or(BridgeError::InvalidShape)?;
                if part.get("type").and_then(Value::as_str) != Some("input_text") {
                    return Err(BridgeError::UnsupportedSemantics);
                }
                text.push_str(&required_string(part, "text")?);
            }
            Value::String(text)
        }
        _ => return Err(BridgeError::UnsupportedSemantics),
    };
    Ok(json!({"content": content, "role": role}))
}

fn responses_tool_to_chat(tool: &Value) -> Result<Value, BridgeError> {
    let tool = tool.as_object().ok_or(BridgeError::InvalidShape)?;
    let mut function = tool.clone();
    function.remove("type");
    Ok(json!({"function": function, "type": "function"}))
}

fn responses_tool_choice_to_chat(choice: &Value) -> Result<Value, BridgeError> {
    if choice
        .as_str()
        .is_some_and(|choice| matches!(choice, "auto" | "none" | "required"))
    {
        return Ok(choice.clone());
    }
    let choice = choice
        .as_object()
        .ok_or(BridgeError::UnsupportedSemantics)?;
    if choice.get("type").and_then(Value::as_str) != Some("function") {
        return Err(BridgeError::UnsupportedSemantics);
    }
    Ok(json!({
        "function": {"name": required_string(choice, "name")?},
        "type": "function"
    }))
}

fn responses_response_to_chat(
    source: &Map<String, Value>,
    public_model: &str,
) -> Result<Value, BridgeError> {
    // 只把显式 completed response 投影为 Chat 成功；其他终态不能伪造成 stop。
    if source.get("object").and_then(Value::as_str) != Some("response")
        || source.get("status").and_then(Value::as_str) != Some("completed")
    {
        return Err(BridgeError::InvalidShape);
    }
    let output = source
        .get("output")
        .and_then(Value::as_array)
        .ok_or(BridgeError::InvalidShape)?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for item in output {
        let item = item.as_object().ok_or(BridgeError::InvalidShape)?;
        match item.get("type").and_then(Value::as_str) {
            Some("message") => text.push_str(&responses_output_text(item)?),
            Some("function_call") => {
                let arguments = required_string(item, "arguments")?;
                validate_arguments(&arguments)?;
                tool_calls.push(json!({
                    "function": {
                        "arguments": arguments,
                        "name": required_string(item, "name")?
                    },
                    "id": required_string(item, "call_id")?,
                    "type": "function"
                }));
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        }
    }

    // 构造单 choice Chat response，并映射 usage 名称。
    let upstream_id = required_string(source, "id")?;
    let mut message = Map::new();
    message.insert("role".to_owned(), Value::String("assistant".to_owned()));
    let finish_reason = if tool_calls.is_empty() {
        message.insert("content".to_owned(), Value::String(text));
        "stop"
    } else {
        message.insert(
            "content".to_owned(),
            if text.is_empty() {
                Value::Null
            } else {
                Value::String(text)
            },
        );
        message.insert("tool_calls".to_owned(), Value::Array(tool_calls));
        "tool_calls"
    };
    let mut result = json!({
        "choices": [{"finish_reason": finish_reason, "index": 0, "message": message}],
        "id": map_id(&upstream_id, "resp_", "chatcmpl_"),
        "model": public_model,
        "object": "chat.completion"
    });
    if let Some(usage) = source.get("usage").and_then(Value::as_object) {
        result["usage"] = json!({
            "completion_tokens": usage.get("output_tokens").cloned().unwrap_or(Value::Null),
            "prompt_tokens": usage.get("input_tokens").cloned().unwrap_or(Value::Null),
            "total_tokens": usage.get("total_tokens").cloned().unwrap_or(Value::Null)
        });
    }
    Ok(result)
}

fn responses_output_text(item: &Map<String, Value>) -> Result<String, BridgeError> {
    let parts = item
        .get("content")
        .and_then(Value::as_array)
        .ok_or(BridgeError::InvalidShape)?;
    let mut text = String::new();
    for part in parts {
        let part = part.as_object().ok_or(BridgeError::InvalidShape)?;
        if part.get("type").and_then(Value::as_str) != Some("output_text") {
            return Err(BridgeError::UnsupportedSemantics);
        }
        text.push_str(&required_string(part, "text")?);
    }
    Ok(text)
}

fn chat_response_to_responses(
    source: &Map<String, Value>,
    public_model: &str,
) -> Result<Value, BridgeError> {
    // 只接受一个完成 choice，避免合并多 choice 时制造未定义顺序。
    let choices = source
        .get("choices")
        .and_then(Value::as_array)
        .ok_or(BridgeError::InvalidShape)?;
    if choices.len() != 1 {
        return Err(BridgeError::UnsupportedSemantics);
    }
    let choice = choices[0].as_object().ok_or(BridgeError::InvalidShape)?;
    let message = choice
        .get("message")
        .and_then(Value::as_object)
        .ok_or(BridgeError::InvalidShape)?;
    let upstream_id = required_string(source, "id")?;
    if !matches!(
        choice.get("finish_reason").and_then(Value::as_str),
        Some("stop" | "tool_calls")
    ) {
        return Err(BridgeError::UnsupportedSemantics);
    }
    let suffix = id_suffix(&upstream_id, "chatcmpl_");
    let mut output = Vec::new();
    let mut item_ids = BTreeSet::new();
    if let Some(content) = message.get("content").and_then(Value::as_str)
        && !content.is_empty()
    {
        output.push(json!({
            "content": [{"annotations": [], "text": content, "type": "output_text"}],
            "id": format!("msg_{suffix}"),
            "role": "assistant",
            "status": "completed",
            "type": "message"
        }));
    }
    if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
        for (ordinal, call) in calls.iter().enumerate() {
            let call = call.as_object().ok_or(BridgeError::InvalidShape)?;
            let function = call
                .get("function")
                .and_then(Value::as_object)
                .ok_or(BridgeError::InvalidShape)?;
            let arguments = required_string(function, "arguments")?;
            validate_arguments(&arguments)?;
            let call_id = required_string(call, "id")?;
            let item_id = allocate_non_stream_item_id(&call_id, ordinal + 1, &mut item_ids);
            output.push(json!({
                "arguments": arguments,
                "call_id": call_id,
                "id": item_id,
                "name": required_string(function, "name")?,
                "status": "completed",
                "type": "function_call"
            }));
        }
    }
    if output.is_empty() {
        return Err(BridgeError::InvalidShape);
    }
    let mut result = json!({
        "id": format!("resp_{suffix}"),
        "model": public_model,
        "object": "response",
        "output": output,
        "status": "completed"
    });
    if let Some(usage) = source.get("usage").and_then(Value::as_object) {
        result["usage"] = json!({
            "input_tokens": usage.get("prompt_tokens").cloned().unwrap_or(Value::Null),
            "output_tokens": usage.get("completion_tokens").cloned().unwrap_or(Value::Null),
            "total_tokens": usage.get("total_tokens").cloned().unwrap_or(Value::Null)
        });
    }
    Ok(result)
}

fn copy_fields(source: &Map<String, Value>, target: &mut Map<String, Value>, fields: &[&str]) {
    for field in fields {
        if let Some(value) = source.get(*field) {
            target.insert((*field).to_owned(), value.clone());
        }
    }
}

fn required_string(object: &Map<String, Value>, field: &str) -> Result<String, BridgeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or(BridgeError::InvalidShape)
}

fn validate_arguments(arguments: &str) -> Result<(), BridgeError> {
    if arguments.is_empty()
        || !serde_json::from_str::<Value>(arguments).is_ok_and(|value| value.is_object())
    {
        return Err(BridgeError::InvalidToolArguments);
    }
    Ok(())
}

fn bridge_item_id(call_id: &str) -> String {
    call_id
        .strip_prefix("call_")
        .map(|suffix| format!("fc_{suffix}"))
        .unwrap_or_else(|| format!("fc_{call_id}"))
}

fn non_stream_item_id(call_id: &str) -> String {
    call_id
        .rsplit_once('_')
        .map(|(_, suffix)| format!("fc_tool_{suffix}"))
        .unwrap_or_else(|| format!("fc_tool_{call_id}"))
}

fn allocate_non_stream_item_id(
    call_id: &str,
    ordinal: usize,
    used: &mut BTreeSet<String>,
) -> String {
    let preferred = non_stream_item_id(call_id);
    if used.insert(preferred.clone()) {
        return preferred;
    }
    let unique = format!("fc_tool_{ordinal}_{}", id_suffix(call_id, "call_"));
    used.insert(unique.clone());
    unique
}

fn id_suffix<'a>(id: &'a str, prefix: &str) -> &'a str {
    id.strip_prefix(prefix).unwrap_or(id)
}

fn map_id(id: &str, from: &str, to: &str) -> String {
    format!("{to}{}", id_suffix(id, from))
}

struct ResponsesToChatStream {
    state: ResponsesStreamState,
    chat_id: Option<String>,
    role_emitted: bool,
    tool_indices: BTreeMap<u64, u64>,
    has_text: bool,
    has_tools: bool,
    terminal_emitted: bool,
}

impl ResponsesToChatStream {
    fn new() -> Self {
        Self {
            state: ResponsesStreamState::new(),
            chat_id: None,
            role_emitted: false,
            tool_indices: BTreeMap::new(),
            has_text: false,
            has_tools: false,
            terminal_emitted: false,
        }
    }

    fn render(&mut self, event: SseEvent, public_model: &str) -> Result<Bytes, BridgeError> {
        // 先让严格状态机验证 event/type、identity 与 terminal，再生成目标 wire。
        self.state.ingest(&event)?;
        let value: Value =
            serde_json::from_str(event.data()).map_err(|_| BridgeError::InvalidStream)?;
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or(BridgeError::InvalidStream)?;
        let mut output = Vec::new();
        match kind {
            "response.created" => {
                let response = value
                    .get("response")
                    .and_then(Value::as_object)
                    .ok_or(BridgeError::InvalidStream)?;
                let id = required_string(response, "id")?;
                self.chat_id = Some(map_id(&id, "resp_", "chatcmpl_"));
            }
            "response.output_item.added" => {
                let index = value
                    .get("output_index")
                    .and_then(Value::as_u64)
                    .ok_or(BridgeError::InvalidStream)?;
                let item = value
                    .get("item")
                    .and_then(Value::as_object)
                    .ok_or(BridgeError::InvalidStream)?;
                if item.get("type").and_then(Value::as_str) == Some("function_call") {
                    self.has_tools = true;
                    let chat_index = self.tool_indices.len() as u64;
                    self.tool_indices.insert(index, chat_index);
                    let delta = json!({
                        "role": if self.role_emitted { Value::Null } else { Value::String("assistant".to_owned()) },
                        "tool_calls": [{
                            "function": {"arguments": "", "name": required_string(item, "name")?},
                            "id": required_string(item, "call_id")?,
                            "index": chat_index,
                            "type": "function"
                        }]
                    });
                    self.role_emitted = true;
                    output.extend(chat_chunk(
                        self.chat_id()?,
                        public_model,
                        strip_null_role(delta),
                        Value::Null,
                    )?);
                }
            }
            "response.output_text.delta" => {
                self.has_text = true;
                let mut delta = Map::new();
                if !self.role_emitted {
                    delta.insert("role".to_owned(), Value::String("assistant".to_owned()));
                    self.role_emitted = true;
                }
                delta.insert(
                    "content".to_owned(),
                    value
                        .get("delta")
                        .cloned()
                        .ok_or(BridgeError::InvalidStream)?,
                );
                output.extend(chat_chunk(
                    self.chat_id()?,
                    public_model,
                    Value::Object(delta),
                    Value::Null,
                )?);
            }
            "response.function_call_arguments.delta" => {
                let index = value
                    .get("output_index")
                    .and_then(Value::as_u64)
                    .ok_or(BridgeError::InvalidStream)?;
                let chat_index = self
                    .tool_indices
                    .get(&index)
                    .copied()
                    .ok_or(BridgeError::InvalidToolIdentity)?;
                output.extend(chat_chunk(
                    self.chat_id()?,
                    public_model,
                    json!({"tool_calls": [{
                        "function": {"arguments": value.get("delta").cloned().ok_or(BridgeError::InvalidStream)?},
                        "index": chat_index
                    }]}),
                    Value::Null,
                )?);
            }
            "response.completed" => {
                let finish = if self.has_tools { "tool_calls" } else { "stop" };
                output.extend(chat_chunk(
                    self.chat_id()?,
                    public_model,
                    json!({}),
                    Value::String(finish.to_owned()),
                )?);
                output.extend_from_slice(b"data: [DONE]\n\n");
                self.terminal_emitted = true;
            }
            "response.failed" | "response.incomplete" | "error" => {
                return Err(BridgeError::InvalidStream);
            }
            _ => {}
        }
        Ok(Bytes::from(output))
    }

    fn finish(&mut self) -> Result<Bytes, BridgeError> {
        self.state.finish()?;
        if !self.terminal_emitted || (!self.has_text && !self.has_tools) {
            return Err(BridgeError::InvalidStream);
        }
        Ok(Bytes::new())
    }

    fn chat_id(&self) -> Result<&str, BridgeError> {
        self.chat_id.as_deref().ok_or(BridgeError::InvalidStream)
    }
}

fn strip_null_role(mut value: Value) -> Value {
    if value.get("role").is_some_and(Value::is_null) {
        value
            .as_object_mut()
            .expect("delta is object")
            .remove("role");
    }
    value
}

fn chat_chunk(
    id: &str,
    model: &str,
    delta: Value,
    finish_reason: Value,
) -> Result<Vec<u8>, BridgeError> {
    sse_data(&json!({
        "choices": [{"delta": delta, "finish_reason": finish_reason, "index": 0}],
        "id": id,
        "model": model,
        "object": "chat.completion.chunk"
    }))
}

struct ChatToResponsesStream {
    state: ChatStreamState,
    response_id: Option<String>,
    message_id: Option<String>,
    created_emitted: bool,
    message_started: bool,
    text: String,
    calls: BTreeMap<u64, StreamCall>,
    finish_reason: Option<String>,
    terminal_emitted: bool,
}

struct StreamCall {
    item_id: String,
    call_id: String,
    name: String,
    arguments: String,
    completed: bool,
}

impl ChatToResponsesStream {
    fn new() -> Self {
        Self {
            state: ChatStreamState::new(),
            response_id: None,
            message_id: None,
            created_emitted: false,
            message_started: false,
            text: String::new(),
            calls: BTreeMap::new(),
            finish_reason: None,
            terminal_emitted: false,
        }
    }

    fn render(&mut self, event: SseEvent, public_model: &str) -> Result<Bytes, BridgeError> {
        // 先用严格 Chat 状态机固定 index/call identity 和 DONE 终态。
        self.state.ingest(&event)?;
        if event.data() == "[DONE]" {
            return self.render_done(public_model);
        }
        let value: Value =
            serde_json::from_str(event.data()).map_err(|_| BridgeError::InvalidStream)?;
        let upstream_id = value
            .get("id")
            .and_then(Value::as_str)
            .ok_or(BridgeError::InvalidStream)?;
        let response_id = self
            .response_id
            .get_or_insert_with(|| map_id(upstream_id, "chatcmpl_", "resp_"))
            .clone();
        self.message_id
            .get_or_insert_with(|| format!("msg_{}", id_suffix(upstream_id, "chatcmpl_")));
        let mut output = Vec::new();
        if !self.created_emitted {
            output.extend(response_event(
                "response.created",
                json!({
                    "response": {"id": response_id, "model": public_model, "object": "response", "output": [], "status": "in_progress"},
                    "type": "response.created"
                }),
            )?);
            self.created_emitted = true;
        }

        // 将单 choice Chat delta 展开为 typed Responses lifecycle events。
        let choice = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(Value::as_object)
            .ok_or(BridgeError::InvalidStream)?;
        let delta = choice
            .get("delta")
            .and_then(Value::as_object)
            .ok_or(BridgeError::InvalidStream)?;
        if let Some(content) = delta.get("content").and_then(Value::as_str) {
            if !self.calls.is_empty() {
                return Err(BridgeError::UnsupportedSemantics);
            }
            if !self.message_started {
                output.extend(response_event(
                    "response.output_item.added",
                    json!({
                        "item": {"content": [], "id": self.message_id(), "role": "assistant", "status": "in_progress", "type": "message"},
                        "output_index": 0,
                        "type": "response.output_item.added"
                    }),
                )?);
                self.message_started = true;
            }
            self.text.push_str(content);
            output.extend(response_event(
                "response.output_text.delta",
                json!({
                    "content_index": 0,
                    "delta": content,
                    "item_id": self.message_id(),
                    "output_index": 0,
                    "type": "response.output_text.delta"
                }),
            )?);
        }
        if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
            if self.message_started {
                return Err(BridgeError::UnsupportedSemantics);
            }
            for tool_call in tool_calls {
                output.extend(self.render_tool_delta(tool_call)?);
            }
        }
        if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish_reason = Some(reason.to_owned());
            output.extend(self.render_item_completion()?);
        }
        Ok(Bytes::from(output))
    }

    fn render_tool_delta(&mut self, tool_call: &Value) -> Result<Vec<u8>, BridgeError> {
        let tool_call = tool_call.as_object().ok_or(BridgeError::InvalidStream)?;
        let index = tool_call
            .get("index")
            .and_then(Value::as_u64)
            .ok_or(BridgeError::InvalidStream)?;
        let function = tool_call
            .get("function")
            .and_then(Value::as_object)
            .ok_or(BridgeError::InvalidStream)?;
        let mut output = Vec::new();
        if let Entry::Vacant(entry) = self.calls.entry(index) {
            let call_id = required_string(tool_call, "id")?;
            let name = required_string(function, "name")?;
            let item_id = bridge_item_id(&call_id);
            entry.insert(StreamCall {
                item_id: item_id.clone(),
                call_id: call_id.clone(),
                name: name.clone(),
                arguments: String::new(),
                completed: false,
            });
            output.extend(response_event(
                "response.output_item.added",
                json!({
                    "item": {"arguments": "", "call_id": call_id, "id": item_id, "name": name, "status": "in_progress", "type": "function_call"},
                    "output_index": index,
                    "type": "response.output_item.added"
                }),
            )?);
        }
        let arguments = function
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !arguments.is_empty() {
            let call = self.calls.get_mut(&index).expect("call inserted above");
            call.arguments.push_str(arguments);
            output.extend(response_event(
                "response.function_call_arguments.delta",
                json!({
                    "delta": arguments,
                    "item_id": call.item_id,
                    "output_index": index,
                    "type": "response.function_call_arguments.delta"
                }),
            )?);
        }
        Ok(output)
    }

    fn render_item_completion(&mut self) -> Result<Vec<u8>, BridgeError> {
        let mut output = Vec::new();
        if self.finish_reason.as_deref() == Some("tool_calls") {
            for (index, call) in &mut self.calls {
                validate_arguments(&call.arguments)?;
                output.extend(response_event(
                    "response.function_call_arguments.done",
                    json!({
                        "arguments": call.arguments,
                        "item_id": call.item_id,
                        "output_index": index,
                        "type": "response.function_call_arguments.done"
                    }),
                )?);
                output.extend(response_event(
                    "response.output_item.done",
                    json!({
                        "item": call_value(call),
                        "output_index": index,
                        "type": "response.output_item.done"
                    }),
                )?);
                call.completed = true;
            }
        } else if self.message_started {
            output.extend(response_event(
                "response.output_item.done",
                json!({
                    "item": self.message_value(),
                    "output_index": 0,
                    "type": "response.output_item.done"
                }),
            )?);
        }
        Ok(output)
    }

    fn render_done(&mut self, public_model: &str) -> Result<Bytes, BridgeError> {
        if !self.created_emitted || self.finish_reason.is_none() {
            return Err(BridgeError::InvalidStream);
        }
        let output_items = if self.calls.is_empty() {
            vec![self.message_value()]
        } else {
            self.calls.values().map(call_value).collect()
        };
        let bytes = response_event(
            "response.completed",
            json!({
                "response": {
                    "id": self.response_id.as_deref().ok_or(BridgeError::InvalidStream)?,
                    "model": public_model,
                    "object": "response",
                    "output": output_items,
                    "status": "completed"
                },
                "type": "response.completed"
            }),
        )?;
        self.terminal_emitted = true;
        Ok(Bytes::from(bytes))
    }

    fn finish(&mut self) -> Result<Bytes, BridgeError> {
        self.state.finish()?;
        if !self.terminal_emitted {
            return Err(BridgeError::InvalidStream);
        }
        Ok(Bytes::new())
    }

    fn message_id(&self) -> &str {
        self.message_id
            .as_deref()
            .expect("message id follows response id")
    }

    fn message_value(&self) -> Value {
        json!({
            "content": [{"annotations": [], "text": self.text, "type": "output_text"}],
            "id": self.message_id(),
            "role": "assistant",
            "status": "completed",
            "type": "message"
        })
    }
}

fn call_value(call: &StreamCall) -> Value {
    json!({
        "arguments": call.arguments,
        "call_id": call.call_id,
        "id": call.item_id,
        "name": call.name,
        "status": "completed",
        "type": "function_call"
    })
}

fn response_event(event: &str, value: Value) -> Result<Vec<u8>, BridgeError> {
    let mut output = format!("event: {event}\n").into_bytes();
    output.extend(sse_data(&value)?);
    Ok(output)
}

fn sse_data(value: &Value) -> Result<Vec<u8>, BridgeError> {
    let data = serde_json::to_vec(value).map_err(|_| BridgeError::InvalidShape)?;
    let mut output = b"data: ".to_vec();
    output.extend(data);
    output.extend_from_slice(b"\n\n");
    Ok(output)
}
