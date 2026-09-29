//! Explicit wire rules selected by trusted adapters, never by business JSON.
use crate::{
    protocol::openai::{CodecError, Profile},
    semantic::value::ReplayOrigin,
};
use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WireRules {
    pub readable_reasoning: bool,
    pub usage_aliases: bool,
    pub default_cache_write: bool,
    pub response_extras: bool,
    pub derived_output_text: bool,
    pub chunk_metadata_drift: bool,
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
