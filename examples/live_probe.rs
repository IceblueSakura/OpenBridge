//! Authorized live acceptance probe for the DeepSeek and Xiaomi MiMo provider slice.
//!
//! Explicitly gated: refuses to run unless `OPENBRIDGE_PROBE=1`.
//!
//!     OPENBRIDGE_PROBE=1 cargo run --locked --offline --example live_probe
//!
//! Boundaries honored here:
//! - credentials are read from the private local config at run time and used
//!   in-process only for auth headers; nothing secret is logged or reported;
//! - the agreed matrix is fixed: 2 models × chat/responses × {text, json_object,
//!   tool} × {JSON, SSE}, tool cells continue one round with the tool output;
//! - one request per call, 120s timeout, 2s spacing, bounded `max_output_tokens`;
//! - model ids are verified through the free Models listing before any paid call;
//! - raw per-call reports go to the gitignored `testdata/runtime/` tree and hold
//!   no keys, auth headers or credential locators.

use openbridge::{
    execution::{Attempt, AttemptError, admit, prepare_chat, prepare_responses},
    protocol::openai::{
        chat_envelope,
        chat_sse::ChatSseDecoder,
        envelope,
        sse::{Obfuscation, ResponsesSseDecoder, SseLimits},
    },
    provider::{ErrorClass, ProviderDefinition, SecretMaterial, catalog as provider_catalog},
    semantic::task::generation::{Completion, ContentPart, Item, Outcome, Usage},
    topology::{Endpoint, EndpointId, ProtocolProfile, PublicModel, catalog as topology_catalog},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fmt::Write as _, time::Instant};
use tokio::time::{Duration, sleep};

const CALL_TIMEOUT: Duration = Duration::from_secs(120);
const CALL_INTERVAL: Duration = Duration::from_secs(2);
const EXCERPT_CHARS: usize = 120;

const TEXT_PROMPT: &str = "Reply with exactly the word pong.";
const JSON_PROMPT: &str = "Return a JSON object with keys pong (boolean) and note (string).";
const TOOL_PROMPT: &str = "Use the lookup tool to find the value for key \"alpha\", then report it in one short sentence.";
const TOOL_OUTPUT: &str = "{\"value\": 42}";

struct ModelSpec {
    label: &'static str,
    pool: &'static str,
    provider: &'static str,
    models_path: &'static str,
    chat_endpoint: &'static str,
    responses_endpoint: &'static str,
}

const MODELS: [ModelSpec; 2] = [
    ModelSpec {
        label: "deepseek-flash",
        pool: "deepseek-primary",
        provider: "deepseek",
        models_path: "/models",
        chat_endpoint: "deepseek-chat",
        responses_endpoint: "deepseek-responses",
    },
    ModelSpec {
        label: "mimo-v2.6-pro",
        pool: "mimo-primary",
        provider: "xiaomi",
        models_path: "/v1/models",
        chat_endpoint: "xiaomi-chat",
        responses_endpoint: "xiaomi-responses",
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    Text,
    JsonObject,
    Tool,
}

impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::JsonObject => "json_object",
            Self::Tool => "tool",
        }
    }

    fn cap(self) -> u64 {
        match self {
            Self::Text => 64,
            Self::JsonObject | Self::Tool => 192,
        }
    }
}

const CASES: [Case; 3] = [Case::Text, Case::JsonObject, Case::Tool];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Delivery {
    Json,
    Stream,
}

impl Delivery {
    fn name(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Stream => "sse",
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
struct CallReport {
    model: String,
    provider: String,
    protocol: String,
    case: String,
    delivery: String,
    round: u8,
    stage: String,
    ok: bool,
    http_status: Option<u16>,
    error_class: Option<String>,
    outcome: Option<String>,
    usage: Option<Value>,
    text_chars: Option<usize>,
    excerpt: Option<String>,
    tool_call: Option<String>,
    error: Option<String>,
    delivered_bytes: usize,
    latency_ms: u64,
}

#[derive(Clone, Debug)]
struct ToolCallInfo {
    call_id: String,
    name: String,
    arguments: String,
}

struct Credentials {
    pools: BTreeMap<String, String>,
}

/// Minimal private-config reader: pool id -> first API key. Values never leave
/// this process except as auth headers.
#[derive(serde::Deserialize)]
struct CredentialFile {
    #[serde(default)]
    credential_pools: Vec<CredentialPool>,
}

#[derive(serde::Deserialize)]
struct CredentialPool {
    id: String,
    #[serde(default)]
    api_keys: Vec<String>,
}

fn load_credentials(path: &str) -> Result<Credentials, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    let parsed: CredentialFile = toml::from_str(&raw).map_err(|e| format!("parse {path}: {e}"))?;
    let mut pools = BTreeMap::new();
    for pool in parsed.credential_pools {
        if let Some(key) = pool.api_keys.first() {
            let secret = SecretMaterial::new(key)
                .map_err(|_| format!("pool {} holds invalid credential material", pool.id))?;
            pools.insert(pool.id, secret.expose().to_string());
        }
    }
    if pools.is_empty() {
        return Err(format!("{path} contains no credential pools"));
    }
    Ok(Credentials { pools })
}

fn scenario_request(
    protocol: ProtocolProfile,
    label: &str,
    case: Case,
    delivery: Delivery,
    round: u8,
    call: Option<&ToolCallInfo>,
) -> Value {
    let streaming = delivery == Delivery::Stream;
    let mut value = match protocol {
        ProtocolProfile::OpenAiChat => {
            let mut messages = vec![json!({"role":"user","content": match case {
                Case::Text => TEXT_PROMPT,
                Case::JsonObject => JSON_PROMPT,
                Case::Tool => TOOL_PROMPT,
            }})];
            if round == 2 {
                let call = call.expect("round two carries a tool call");
                messages.push(json!({"role":"assistant","content":null,"tool_calls":[
                    {"id":call.call_id,"type":"function","function":{"name":call.name,"arguments":call.arguments}}
                ]}));
                messages
                    .push(json!({"role":"tool","tool_call_id":call.call_id,"content":TOOL_OUTPUT}));
            }
            let mut value =
                json!({"model":label,"messages":messages,"max_completion_tokens":case.cap()});
            if case == Case::JsonObject {
                value["response_format"] = json!({"type":"json_object"});
            }
            if case == Case::Tool {
                value["tools"] = json!([chat_tool()]);
                value["tool_choice"] = json!("auto");
            }
            value
        }
        ProtocolProfile::OpenAiResponses => {
            let prompt = match case {
                Case::Text => TEXT_PROMPT,
                Case::JsonObject => JSON_PROMPT,
                Case::Tool => TOOL_PROMPT,
            };
            let mut input =
                vec![json!({"role":"user","content":[{"type":"input_text","text":prompt}]})];
            if round == 2 {
                let call = call.expect("round two carries a tool call");
                input.push(json!({"type":"function_call","call_id":call.call_id,"name":call.name,"arguments":call.arguments}));
                input.push(json!({"type":"function_call_output","call_id":call.call_id,"output":TOOL_OUTPUT}));
            }
            let mut value = json!({"model":label,"input":input,"max_output_tokens":case.cap()});
            if case == Case::JsonObject {
                value["text"] = json!({"format":{"type":"json_object"}});
            }
            if case == Case::Tool {
                value["tools"] = json!([responses_tool()]);
                value["tool_choice"] = json!("auto");
            }
            value
        }
    };
    if streaming {
        value["stream"] = json!(true);
        if protocol == ProtocolProfile::OpenAiChat {
            value["stream_options"] = json!({"include_usage":true,"include_obfuscation":false});
        }
    }
    value
}

fn chat_tool() -> Value {
    json!({"type":"function","function":{
        "name":"lookup",
        "description":"Look up the stored value for a key.",
        "parameters":{"type":"object","properties":{"key":{"type":"string"}},"required":["key"],"additionalProperties":false},
        "strict":false
    }})
}

fn responses_tool() -> Value {
    json!({"type":"function",
        "name":"lookup",
        "description":"Look up the stored value for a key.",
        "parameters":{"type":"object","properties":{"key":{"type":"string"}},"required":["key"],"additionalProperties":false},
        "strict":false
    })
}

fn chat_stream_options() -> chat_envelope::StreamOptions {
    chat_envelope::StreamOptions {
        include_usage: openbridge::semantic::value::Presence::Value(true),
        include_obfuscation: openbridge::semantic::value::Presence::Value(false),
    }
}

fn outcome_label(outcome: Outcome) -> String {
    match outcome {
        Outcome::Completed(Completion::Stop) => "completed:stop".into(),
        Outcome::Completed(Completion::ToolCalls) => "completed:tool_calls".into(),
        Outcome::Incomplete => "incomplete".into(),
        Outcome::Failed => "failed".into(),
        Outcome::Cancelled => "cancelled".into(),
    }
}

fn usage_value(usage: &Usage) -> Value {
    json!({
        "input_tokens": usage.input_tokens,
        "output_tokens": usage.output_tokens,
        "total_tokens": usage.total_tokens,
        "reasoning_tokens": usage.reasoning_tokens,
        "cached_input_tokens": usage.cached_input_tokens,
        "input_cache_write_tokens": usage.input_cache_write_tokens,
    })
}

fn error_class(error: &AttemptError) -> Option<ErrorClass> {
    match error {
        AttemptError::Status { class, .. } | AttemptError::Transport(class) => Some(*class),
        _ => None,
    }
}

fn excerpt(text: &str) -> String {
    text.chars().take(EXCERPT_CHARS).collect()
}

struct CallOutcome {
    report: CallReport,
    tool_call: Option<ToolCallInfo>,
}

struct CallContext<'a> {
    out_dir: &'a str,
    label: &'a str,
    provider_name: &'a str,
    provider_def: &'a ProviderDefinition,
    secret: &'a SecretMaterial,
    endpoint: &'a Endpoint,
    public: &'a PublicModel,
    downstream: ProtocolProfile,
    delivery: Delivery,
    case: Case,
}

async fn run_call(
    client: &reqwest::Client,
    ctx: &CallContext<'_>,
    request_json: Value,
    round: u8,
) -> CallOutcome {
    let started = Instant::now();
    let base = CallReport {
        model: ctx.label.into(),
        provider: ctx.provider_name.into(),
        protocol: match ctx.downstream {
            ProtocolProfile::OpenAiChat => "chat",
            ProtocolProfile::OpenAiResponses => "responses",
        }
        .into(),
        case: ctx.case.name().into(),
        delivery: ctx.delivery.name().into(),
        round,
        stage: "start".into(),
        ok: false,
        http_status: None,
        error_class: None,
        outcome: None,
        usage: None,
        text_chars: None,
        excerpt: None,
        tool_call: None,
        error: None,
        delivered_bytes: 0,
        latency_ms: 0,
    };
    let fail = |stage: &str,
                error_class: Option<String>,
                error: Option<String>,
                report: &mut CallReport| {
        report.stage = stage.into();
        report.error_class = error_class;
        report.error = error;
        report.latency_ms = started.elapsed().as_millis() as u64;
    };

    // 1. Downstream request bytes must decode through the real admission codecs.
    let bytes = serde_json::to_vec(&request_json).expect("request serializes");
    enum Downstream {
        Chat(chat_envelope::DecodedChatRequest),
        Responses(envelope::DecodedResponsesRequest),
    }
    let decoded = match ctx.downstream {
        ProtocolProfile::OpenAiChat => match chat_envelope::decode_request_bytes(&bytes) {
            Ok(request) => Downstream::Chat(request),
            Err(error) => {
                let mut report = base;
                fail(
                    "request-decode",
                    None,
                    Some(format!("{error}")),
                    &mut report,
                );
                return CallOutcome {
                    report,
                    tool_call: None,
                };
            }
        },
        ProtocolProfile::OpenAiResponses => match envelope::decode_request_bytes(&bytes) {
            Ok(request) => Downstream::Responses(request),
            Err(error) => {
                let mut report = base;
                fail(
                    "request-decode",
                    None,
                    Some(format!("{error}")),
                    &mut report,
                );
                return CallOutcome {
                    report,
                    tool_call: None,
                };
            }
        },
    };

    // 2. Entry admission against the public contract, then candidate preparation.
    let task = match &decoded {
        Downstream::Chat(request) => &request.task,
        Downstream::Responses(request) => &request.task,
    };
    if let Err(error) = admit(ctx.public, task) {
        let mut report = base;
        fail(
            "admission",
            error_class(&error).map(|c| format!("{c:?}")),
            Some(format!("{error}")),
            &mut report,
        );
        return CallOutcome {
            report,
            tool_call: None,
        };
    }
    let upstream = match &decoded {
        Downstream::Chat(request) => {
            prepare_chat(ctx.endpoint, ctx.provider_def, ctx.secret, request)
        }
        Downstream::Responses(request) => {
            prepare_responses(ctx.endpoint, ctx.provider_def, ctx.secret, request)
        }
    };
    let upstream = match upstream {
        Ok(upstream) => upstream,
        Err(error) => {
            let mut report = base;
            fail(
                "prepare",
                error_class(&error).map(|c| format!("{c:?}")),
                Some(format!("{error}")),
                &mut report,
            );
            return CallOutcome {
                report,
                tool_call: None,
            };
        }
    };

    // Raw capture for wire diagnosis: bodies only, never headers or credentials.
    let raw_stem = format!(
        "{}/raw/{}-{}-{}-{}-r{}",
        ctx.out_dir,
        ctx.label,
        match ctx.downstream {
            ProtocolProfile::OpenAiChat => "chat",
            ProtocolProfile::OpenAiResponses => "responses",
        },
        ctx.case.name(),
        ctx.delivery.name(),
        round
    );
    let _ = std::fs::create_dir_all(format!("{}/raw", ctx.out_dir));
    let _ = std::fs::write(format!("{raw_stem}.req.json"), &upstream.body);

    // 3. Transport attempt. The request carries no target, credential or profile.
    let mut call = client.post(format!("{}{}", upstream.origin, upstream.path));
    for (name, value) in &upstream.safe_headers {
        call = call.header(name, value);
    }
    call = call.header(&upstream.auth_header.0, &upstream.auth_header.1);
    let mut response = match call.body(upstream.body).send().await {
        Ok(response) => response,
        Err(error) => {
            let mut report = base;
            let class = if error.is_timeout() {
                ErrorClass::Timeout
            } else {
                ErrorClass::Upstream
            };
            fail(
                "transport",
                Some(format!("{class:?}")),
                Some(format!("{error}")),
                &mut report,
            );
            return CallOutcome {
                report,
                tool_call: None,
            };
        }
    };
    let mut report = base;
    report.http_status = Some(response.status().as_u16());
    let content_type = response
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap_or_default().to_string())
        .unwrap_or_default();
    let mut attempt = Attempt::new(
        ctx.endpoint.protocol,
        ctx.endpoint.execution.response_body_limit,
        SseLimits::default(),
        ctx.endpoint.representation.replay_origin.clone(),
    );
    if let Err(error) = attempt.begin(report.http_status.unwrap_or(0), &content_type) {
        fail(
            "response-head",
            error_class(&error).map(|c| format!("{c:?}")),
            Some(format!("{error}")),
            &mut report,
        );
        return CallOutcome {
            report,
            tool_call: None,
        };
    }
    let mut raw_body: Vec<u8> = vec![];
    let mut intake_error: Option<AttemptError> = None;
    while let Some(chunk) = match response.chunk().await {
        Ok(chunk) => chunk,
        Err(error) => {
            let class = if error.is_timeout() {
                ErrorClass::Timeout
            } else {
                ErrorClass::Upstream
            };
            let _ = std::fs::write(format!("{raw_stem}.resp.txt"), &raw_body);
            fail(
                "transport",
                Some(format!("{class:?}")),
                Some(format!("{error}")),
                &mut report,
            );
            return CallOutcome {
                report,
                tool_call: None,
            };
        }
    } {
        raw_body.extend_from_slice(&chunk);
        if intake_error.is_none() {
            let mut rest = &chunk[..];
            while !rest.is_empty() {
                match attempt.push(rest) {
                    Ok((used, _)) if used > 0 => rest = &rest[used..],
                    Ok(_) => break,
                    Err(error) => {
                        intake_error = Some(error);
                        break;
                    }
                }
            }
        }
    }

    let _ = std::fs::write(format!("{raw_stem}.resp.txt"), &raw_body);
    if let Some(error) = intake_error {
        fail(
            "intake",
            error_class(&error).map(|c| format!("{c:?}")),
            Some(format!("{error}")),
            &mut report,
        );
        return CallOutcome {
            report,
            tool_call: None,
        };
    }

    // 4. Semantic terminal validation, then downstream delivery.
    let finished = match attempt.finish() {
        Ok(finished) => finished,
        Err(error) => {
            fail(
                "terminal",
                error_class(&error).map(|c| format!("{c:?}")),
                Some(format!("{error}")),
                &mut report,
            );
            return CallOutcome {
                report,
                tool_call: None,
            };
        }
    };
    let (text, tool_call) = extract(&finished.semantic);
    report.outcome = Some(outcome_label(finished.semantic.outcome()));
    report.usage = finished.semantic.usage().map(|u| usage_value(&u));
    report.text_chars = Some(text.chars().count());
    report.excerpt = Some(excerpt(&text));
    report.tool_call = tool_call
        .as_ref()
        .map(|c| format!("{} {}", c.name, c.call_id));

    let delivered = match ctx.delivery {
        Delivery::Json => {
            attempt.render_json(ctx.downstream, &ctx.endpoint.representation, ctx.label)
        }
        Delivery::Stream => attempt
            .render_stream(
                ctx.downstream,
                &ctx.endpoint.representation,
                ctx.label,
                SseLimits::default(),
                chat_stream_options(),
                Obfuscation::Disabled,
            )
            .map(|frames| frames.iter().flat_map(|f| f.to_vec()).collect()),
    };
    let delivered = match delivered {
        Ok(delivered) => delivered,
        Err(error) => {
            fail(
                "render",
                error_class(&error).map(|c| format!("{c:?}")),
                Some(format!("{error}")),
                &mut report,
            );
            return CallOutcome { report, tool_call };
        }
    };
    report.delivered_bytes = delivered.len();

    // 5. Client consumption of the delivered bytes through the same profile codec.
    let consumed = match ctx.delivery {
        Delivery::Json => match ctx.downstream {
            ProtocolProfile::OpenAiChat => chat_envelope::decode_response_bytes(&delivered).is_ok(),
            ProtocolProfile::OpenAiResponses => envelope::decode_response_bytes(&delivered).is_ok(),
        },
        Delivery::Stream => match ctx.downstream {
            ProtocolProfile::OpenAiChat => consume_chat_stream(&delivered).is_ok(),
            ProtocolProfile::OpenAiResponses => consume_responses_stream(&delivered).is_ok(),
        },
    };
    if !consumed {
        fail(
            "consumption",
            Some("codec".into()),
            Some("delivered bytes failed client decode".into()),
            &mut report,
        );
        return CallOutcome { report, tool_call };
    }
    report.stage = "consumed".into();
    report.ok = true;
    report.latency_ms = started.elapsed().as_millis() as u64;
    CallOutcome { report, tool_call }
}

fn extract(
    response: &openbridge::semantic::task::generation::GenerationResponse,
) -> (String, Option<ToolCallInfo>) {
    let mut text = String::new();
    let mut call = None;
    for (_, item) in response.items() {
        match item {
            Item::Message(message) => {
                for part in &message.parts {
                    if let ContentPart::Text(content) = &part.content {
                        text.push_str(content.as_str());
                    }
                }
            }
            Item::ToolCall(tool) if call.is_none() => {
                call = Some(ToolCallInfo {
                    call_id: tool.call_id.as_str().into(),
                    name: tool.name.as_str().into(),
                    arguments: tool.arguments.clone(),
                });
            }
            _ => {}
        }
    }
    (text, call)
}

fn consume_chat_stream(delivered: &[u8]) -> Result<(), String> {
    let mut decoder = ChatSseDecoder::new(
        200,
        "text/event-stream; charset=utf-8",
        SseLimits::default(),
    )
    .map_err(|e| e.to_string())?;
    feed(
        &mut |chunk| {
            decoder
                .consume(chunk)
                .map(|(used, _)| used)
                .map_err(|e| e.to_string())
        },
        delivered,
    )?;
    decoder.finish().map_err(|e| e.to_string())
}

fn consume_responses_stream(delivered: &[u8]) -> Result<(), String> {
    let mut decoder = ResponsesSseDecoder::new(
        200,
        "text/event-stream; charset=utf-8",
        SseLimits::default(),
        None,
    )
    .map_err(|e| e.to_string())?;
    feed(
        &mut |chunk| {
            decoder
                .consume(chunk)
                .map(|(used, _)| used)
                .map_err(|e| e.to_string())
        },
        delivered,
    )?;
    decoder.finish().map_err(|e| e.to_string())?;
    decoder.materialize().map(|_| ()).map_err(|e| e.to_string())
}

fn feed(
    consume: &mut dyn FnMut(&[u8]) -> Result<usize, String>,
    delivered: &[u8],
) -> Result<(), String> {
    let mut rest = delivered;
    while !rest.is_empty() {
        let used = consume(rest)?;
        if used == 0 {
            break;
        }
        rest = &rest[used..];
    }
    Ok(())
}

async fn model_listing(
    client: &reqwest::Client,
    origin: &str,
    path: &str,
    secret: &SecretMaterial,
) -> Result<Vec<String>, String> {
    let response = client
        .get(format!("{origin}{path}"))
        .header("authorization", format!("Bearer {}", secret.expose()))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let body = response.bytes().await.map_err(|e| e.to_string())?;
    if status != 200 {
        let excerpt: String = String::from_utf8_lossy(&body).chars().take(300).collect();
        return Err(format!("HTTP {status}: {excerpt}"));
    }
    let value: Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    Ok(value["data"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

fn report_dir() -> String {
    let date = std::process::Command::new("date")
        .arg("+%F")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "undated".into());
    format!("testdata/runtime/probe-{date}")
}

#[tokio::main]
async fn main() {
    if std::env::var("OPENBRIDGE_PROBE").as_deref() != Ok("1") {
        eprintln!("live probe is an explicit paid gate; set OPENBRIDGE_PROBE=1 to run");
        std::process::exit(2);
    }
    let credentials_path = std::env::var("OPENBRIDGE_CREDENTIALS")
        .unwrap_or_else(|_| "config/upstream-credentials.toml".into());
    let credentials = match load_credentials(&credentials_path) {
        Ok(credentials) => credentials,
        Err(error) => {
            eprintln!("credential setup failed: {error}");
            std::process::exit(1);
        }
    };
    let secret_for = |pool: &str| -> SecretMaterial {
        let raw = credentials
            .pools
            .get(pool)
            .unwrap_or_else(|| panic!("missing credential pool {pool}"));
        SecretMaterial::new(raw).expect("validated at load")
    };

    let topology = topology_catalog::default_topology().expect("fixed topology compiles");
    // Egress goes through the environment's outbound proxy when one is set:
    // this is trusted tooling, not business request routing. Loopback tests
    // build their own `no_proxy` clients and are unaffected.
    let mut client_builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(CALL_TIMEOUT);
    for variable in [
        "https_proxy",
        "HTTPS_PROXY",
        "http_proxy",
        "HTTP_PROXY",
        "all_proxy",
        "ALL_PROXY",
    ] {
        if let Ok(url) = std::env::var(variable) {
            match reqwest::Proxy::all(&url) {
                Ok(proxy) => {
                    client_builder = client_builder.proxy(proxy);
                    break;
                }
                Err(_) => continue,
            }
        }
    }
    let client = client_builder.build().expect("http client");
    let out_dir = report_dir();
    std::fs::create_dir_all(&out_dir).expect("report directory");

    let mut reports: Vec<CallReport> = vec![];
    let mut precheck = String::new();
    let mut eligible: Vec<&ModelSpec> = vec![];

    // Free model-listing precheck: never spend a paid call on a wrong model id.
    for spec in &MODELS {
        let definition = match spec.provider {
            "deepseek" => provider_catalog::deepseek(),
            _ => provider_catalog::xiaomi(),
        };
        let secret = secret_for(spec.pool);
        match model_listing(
            &client,
            definition.origin.as_str(),
            spec.models_path,
            &secret,
        )
        .await
        {
            Ok(ids) => {
                let found = ids.iter().any(|id| id == spec.label);
                let mut shown = ids.clone();
                shown.truncate(24);
                let _ = writeln!(
                    precheck,
                    "- {} (`{}`): listing ok ({} ids), exact id {} — ids: {:?}",
                    spec.label,
                    spec.models_path,
                    ids.len(),
                    if found { "found" } else { "MISSING" },
                    shown
                );
                if found {
                    eligible.push(spec);
                }
            }
            Err(error) => {
                let _ = writeln!(
                    precheck,
                    "- {} (`{}`): listing failed: {} — paid calls skipped",
                    spec.label, spec.models_path, error
                );
            }
        }
    }

    // Agreed matrix: 2 models × 2 protocols × 3 cases × 2 deliveries.
    // `OPENBRIDGE_PROBE_MODEL=<label>` narrows paid calls to one model.
    let only = std::env::var("OPENBRIDGE_PROBE_MODEL").ok();
    for spec in &eligible {
        if only.as_ref().is_some_and(|label| label != spec.label) {
            continue;
        }
        let definition = match spec.provider {
            "deepseek" => provider_catalog::deepseek(),
            _ => provider_catalog::xiaomi(),
        };
        let secret = secret_for(spec.pool);
        let public = topology.model(spec.label).expect("public model binding");
        for protocol in [
            ProtocolProfile::OpenAiChat,
            ProtocolProfile::OpenAiResponses,
        ] {
            let endpoint_id = match protocol {
                ProtocolProfile::OpenAiChat => spec.chat_endpoint,
                ProtocolProfile::OpenAiResponses => spec.responses_endpoint,
            };
            let endpoint = topology
                .endpoint(&EndpointId::new(endpoint_id).expect("endpoint id"))
                .expect("compiled endpoint");
            for case in CASES {
                for delivery in [Delivery::Json, Delivery::Stream] {
                    let ctx = CallContext {
                        out_dir: &out_dir,
                        label: spec.label,
                        provider_name: spec.provider,
                        provider_def: &definition,
                        secret: &secret,
                        endpoint,
                        public,
                        downstream: protocol,
                        delivery,
                        case,
                    };
                    let request = scenario_request(protocol, spec.label, case, delivery, 1, None);
                    let first = run_call(&client, &ctx, request, 1).await;
                    let mut tool_call = first.tool_call.clone();
                    reports.push(first.report);
                    if case == Case::Tool
                        && let Some(call) = tool_call.take()
                    {
                        sleep(CALL_INTERVAL).await;
                        let request =
                            scenario_request(protocol, spec.label, case, delivery, 2, Some(&call));
                        let second = run_call(&client, &ctx, request, 2).await;
                        reports.push(second.report);
                    }
                    sleep(CALL_INTERVAL).await;
                }
            }
        }
    }

    // Sanitized report: matrix facts only, no keys, auth headers or locators.
    let jsonl = reports
        .iter()
        .map(|report| serde_json::to_string(report).expect("serializable"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(format!("{out_dir}/calls.jsonl"), format!("{jsonl}\n")).expect("write calls");

    let mut summary = String::new();
    let _ = writeln!(summary, "# Live probe summary\n");
    let _ = writeln!(summary, "## Model precheck\n{precheck}");
    let _ = writeln!(
        summary,
        "\n## Matrix ({} calls, {}/{} scenarios consumed)\n",
        reports.len(),
        reports.iter().filter(|r| r.ok).count(),
        reports.len()
    );
    let _ = writeln!(
        summary,
        "| model | protocol | case | delivery | round | stage | ok | http | class | outcome | text | usage | error | ms |"
    );
    let _ = writeln!(
        summary,
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"
    );
    for report in &reports {
        let error = report
            .error
            .as_deref()
            .map(|e| e.replace('|', "/").chars().take(80).collect::<String>())
            .unwrap_or_else(|| "-".into());
        let _ = writeln!(
            summary,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            report.model,
            report.protocol,
            report.case,
            report.delivery,
            report.round,
            report.stage,
            if report.ok { "yes" } else { "no" },
            report
                .http_status
                .map(|status| status.to_string())
                .unwrap_or_else(|| "-".into()),
            report.error_class.as_deref().unwrap_or("-"),
            report.outcome.as_deref().unwrap_or("-"),
            report
                .text_chars
                .map(|n| format!("{n} chars"))
                .unwrap_or_else(|| "-".into()),
            report
                .usage
                .as_ref()
                .map(|u| format!("in {} / out {}", u["input_tokens"], u["output_tokens"]))
                .unwrap_or_else(|| "-".into()),
            error,
            report.latency_ms,
        );
    }
    std::fs::write(format!("{out_dir}/summary.md"), &summary).expect("write summary");
    println!("{summary}");
    println!("reports written to {out_dir}/ (gitignored)");
}
