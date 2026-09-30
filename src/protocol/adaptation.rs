//! Explicit wire rules selected by trusted adapters, never by business JSON.
use crate::{
    protocol::openai::{CodecError, Profile},
    semantic::value::ReplayOrigin,
};
use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WireRules {
    /// Assistant reasoning text has its own typed owner before its Chat carrier.
    pub readable_reasoning: bool,
    /// Cache hit/miss aliases must agree with reported totals and cached counts.
    pub usage_aliases: bool,
    /// Default absent/null cache-write to zero only inside valid usage. Never
    /// replace a reported/invalid count or invent whole usage; record provenance.
    /// This compatibility value is not evidence of cache activity or billing.
    pub default_cache_write: bool,
    /// Classified, bounded response facts bind final semantics and trusted scope.
    pub response_extras: bool,
    /// Validate the derived text view, then discard it; never restore edited text.
    pub derived_output_text: bool,
    /// Bind first timestamp/fingerprint values while keeping identity/model strict.
    pub chunk_metadata_drift: bool,
    /// Normalize timestamp drift only, without permitting fingerprint changes.
    pub chunk_created_drift: bool,
    /// Bounded, source-bound router and billing facts, never task semantics.
    pub routing_extras: bool,
    /// A readable alias must not conflict with the canonical reasoning carrier.
    pub reasoning_alias: bool,
    /// Scoped OpenAI Responses-format reasoning details on a Chat carrier.
    /// Final encrypted replay requires a completed owner and matching origin;
    /// it does not make the carrier response completed or permit partial replay.
    pub structured_chat_reasoning: bool,
    /// Accept only the checked redundant format marker, not arbitrary formats.
    pub responses_reasoning_format: bool,
    /// One data-only DONE may follow a validated terminal; never replace it.
    pub responses_done_marker: bool,
    /// Content-free usage tail may repeat the same finish, not add new output.
    pub repeated_finish_usage: bool,
    /// Trusted request policy; business JSON cannot override parameter admission.
    pub require_parameters: bool,
    /// Provider request spelling; downstream still uses max_completion_tokens.
    pub legacy_max_tokens: bool,
    /// Advisory lastOne marker and source-bound numeric matched_stop diagnostics.
    pub chat_stop_diagnostics: bool,
    /// Source-bound diagnostic; never public response identity or task semantics.
    pub reported_request_id: bool,
    /// Normalize only explicit integer-zero unsupported usage details.
    pub zero_usage_details: bool,
    /// Validate an exact duplicate Chat-named input detail view on Responses.
    pub responses_usage_detail_view: bool,
    /// Text-only profile: text counts must equal the corresponding total.
    /// Output text includes reasoning; do not add reasoning again. Non-equivalent
    /// modality breakdowns require typed ownership instead of silently dropping.
    /// Source: <https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-chat-completions>.
    pub text_usage_total_view: bool,
    /// Documented null placeholders do not enable active media or legacy calls.
    pub inactive_chat_fields: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Adaptation {
    pub rules: WireRules,
    /// Stable adapter contract identity, not a network or credential locator.
    pub profile_id: &'static str,
    pub scope: Option<ReplayOrigin>,
}
impl Adaptation {
    pub(crate) fn validate_response(
        &self,
        protocol: Profile,
        o: &Map<String, Value>,
    ) -> Result<(), CodecError> {
        if protocol == Profile::Responses {
            for name in crate::protocol::fidelity::RESPONSE_EXTRA_FIELDS {
                if o.contains_key(*name) && !self.rules.response_extras {
                    return Err(CodecError::Unsupported((*name).into()));
                }
            }
            if o.contains_key("output_text") && !self.rules.derived_output_text {
                return Err(CodecError::Unsupported("output_text".into()));
            }
        }
        if let Some(usage) = o.get("usage").and_then(Value::as_object) {
            for name in ["prompt_cache_hit_tokens", "prompt_cache_miss_tokens"] {
                if usage.contains_key(name)
                    && !(protocol == Profile::Chat && self.rules.usage_aliases)
                {
                    return Err(CodecError::Unsupported(name.into()));
                }
            }
        }
        if protocol == Profile::Chat
            && let Some(choices) = o.get("choices").and_then(Value::as_array)
        {
            for choice in choices {
                for field in ["message", "delta"] {
                    if let Some(message) = choice.get(field).and_then(Value::as_object) {
                        self.validate_message(message)?;
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn validate_message(&self, message: &Map<String, Value>) -> Result<(), CodecError> {
        if message.contains_key("reasoning_content") && !self.rules.readable_reasoning {
            return Err(CodecError::Unsupported("reasoning_content".into()));
        }
        Ok(())
    }
}
