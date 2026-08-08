//! Trusted Provider dimensions for request observation.
//!
//! This facade binds each actual upstream call to compile-time Route, target, upstream model,
//! operation, Provider, and execution-mode attributes. Attempt timing/state and transparent body
//! observation live in child modules so this file remains limited to dimension construction and
//! stable Provider-name mappings. No module retains business bodies, credentials, endpoint URLs,
//! or downstream identities.

use opentelemetry::KeyValue;

use crate::{core::OperationKind, provider::ProviderKind};

mod attempt;
mod body;

pub(super) use attempt::{AttemptOutcome, AttemptSummary, ProviderAttemptObservation};
pub(super) use body::observe_json_body;

/// Borrowed compile-time facts for one actual Provider attempt.
pub(crate) struct ProviderAttemptContext<'a> {
    /// One-based attempt index within the downstream request.
    pub(crate) attempt: u64,
    /// Compiled Route identifier.
    pub(crate) route_id: &'a str,
    /// Compiled Upstream Target identifier.
    pub(crate) upstream_target: &'a str,
    /// Operation selected on the Upstream Target.
    pub(crate) upstream_operation: OperationKind,
    /// Provider-visible model selected by the compiled API.
    pub(crate) upstream_model: &'a str,
    /// Provider family owning the selected Target.
    pub(crate) provider: ProviderKind,
    /// Whether this attempt crosses the protocol bridge.
    pub(crate) bridged: bool,
}

/// Bounded, non-sensitive dimensions used by Provider OpenTelemetry measurements.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProviderMetricAttributes {
    pub(super) provider: String,
    gen_ai_provider: String,
    pub(super) route_id: String,
    pub(super) upstream_target: String,
    pub(super) upstream_operation: String,
    pub(super) upstream_model: String,
    pub(super) public_model: String,
    pub(super) operation: String,
    pub(super) gen_ai_operation: String,
    pub(super) route_mode: String,
    pub(super) streaming: bool,
}

impl ProviderMetricAttributes {
    /// Builds Provider attributes from trusted compile-time identifiers.
    pub(super) fn new(
        context: &ProviderAttemptContext<'_>,
        public_model: &str,
        operation: Option<OperationKind>,
        streaming: bool,
    ) -> Self {
        Self {
            provider: provider_name(context.provider).to_owned(),
            gen_ai_provider: gen_ai_provider_name(context.provider).to_owned(),
            route_id: context.route_id.to_owned(),
            upstream_target: context.upstream_target.to_owned(),
            upstream_operation: context.upstream_operation.as_str().to_owned(),
            upstream_model: context.upstream_model.to_owned(),
            public_model: public_model.to_owned(),
            operation: operation
                .map(OperationKind::as_str)
                .unwrap_or("unknown")
                .to_owned(),
            gen_ai_operation: operation
                .map(gen_ai_operation_name)
                .unwrap_or("unknown")
                .to_owned(),
            route_mode: if context.bridged { "bridged" } else { "native" }.to_owned(),
            streaming,
        }
    }

    /// Returns the full trusted attributes for OpenBridge-specific instruments.
    pub(super) fn openbridge_attributes(&self) -> Vec<KeyValue> {
        vec![
            KeyValue::new("gen_ai.provider.name", self.gen_ai_provider.clone()),
            KeyValue::new("gen_ai.operation.name", self.gen_ai_operation.clone()),
            KeyValue::new("gen_ai.request.model", self.upstream_model.clone()),
            KeyValue::new("gen_ai.request.stream", self.streaming),
            KeyValue::new("openbridge.provider.name", self.provider.clone()),
            KeyValue::new("openbridge.route.id", self.route_id.clone()),
            KeyValue::new("openbridge.upstream.target", self.upstream_target.clone()),
            KeyValue::new(
                "openbridge.upstream.operation",
                self.upstream_operation.clone(),
            ),
            KeyValue::new("openbridge.downstream.operation", self.operation.clone()),
            KeyValue::new("openbridge.public_model", self.public_model.clone()),
            KeyValue::new("openbridge.route.mode", self.route_mode.clone()),
        ]
    }

    /// Returns the standard GenAI attributes and a bounded error category when applicable.
    pub(super) fn gen_ai_attributes(&self, outcome: Option<AttemptOutcome>) -> Vec<KeyValue> {
        let mut attributes = self.openbridge_attributes();
        if let Some(outcome) = outcome.filter(|outcome| *outcome != AttemptOutcome::Completed) {
            attributes.push(KeyValue::new("error.type", outcome.as_str()));
        }
        attributes
    }
}

fn provider_name(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::ChatGpt => "chatgpt",
        ProviderKind::OpenAi => "openai",
        ProviderKind::LongCat => "longcat",
        ProviderKind::DeepSeek => "deepseek",
        ProviderKind::MiMo => "mimo",
        ProviderKind::OpenRouter => "openrouter",
        ProviderKind::Nvidia => "nvidia",
        ProviderKind::Bailian => "bailian",
    }
}

/// Maps concrete Provider adapters to the closest stable GenAI provider namespace.
fn gen_ai_provider_name(provider: ProviderKind) -> &'static str {
    match provider {
        ProviderKind::ChatGpt | ProviderKind::OpenAi => "openai",
        ProviderKind::LongCat => "longcat",
        ProviderKind::DeepSeek => "deepseek",
        ProviderKind::MiMo => "mimo",
        ProviderKind::OpenRouter => "openrouter",
        ProviderKind::Nvidia => "nvidia",
        ProviderKind::Bailian => "bailian",
    }
}

/// Maps concrete downstream protocols to the stable GenAI operation vocabulary.
fn gen_ai_operation_name(operation: OperationKind) -> &'static str {
    match operation {
        OperationKind::ChatCompletions | OperationKind::Responses => "chat",
        OperationKind::EmbeddingsCreate => "embeddings",
    }
}
