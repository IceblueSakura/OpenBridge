//! Trusted bidirectional boundary adapters composed from shared protocol codecs.
//! No registry lookup, credentials, network or arbitrary JSON body hooks live here.
use crate::{
    lowering::generation::{GenerationRepresentationContract, RepresentationError, lower_response},
    protocol::{
        adaptation::{Adaptation, WireRules},
        openai::{self, CodecError, DecodedResponse, Profile, events::EventDecoder},
    },
    semantic::value::ReplayOrigin,
};
use serde_json::Value;
mod request;
pub use request::Request;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Dialect {
    #[default]
    Standard,
    OpenBridge,
    DeepSeek,
    Xiaomi,
    OpenRouter,
    LongCat,
    Nvidia,
    Bailian,
    Kimi,
    Zhipu,
}
impl Dialect {
    fn adaptation(self, scope: Option<ReplayOrigin>) -> Adaptation {
        let (profile_id, rules) = match self {
            Self::Standard => ("standard-v1", WireRules::default()),
            Self::OpenBridge => (
                "openbridge-v1",
                WireRules {
                    readable_reasoning: true,
                    structured_chat_reasoning: true,
                    ..Default::default()
                },
            ),
            Self::DeepSeek => (
                "deepseek-v1",
                WireRules {
                    readable_reasoning: true,
                    usage_aliases: true,
                    default_cache_write: true,
                    response_extras: true,
                    chunk_metadata_drift: true,
                    ..Default::default()
                },
            ),
            Self::OpenRouter => (
                "openrouter-v1",
                WireRules {
                    readable_reasoning: true,
                    routing_extras: true,
                    response_extras: true,
                    responses_reasoning_format: true,
                    responses_done_marker: true,
                    reasoning_alias: true,
                    structured_chat_reasoning: true,
                    repeated_finish_usage: true,
                    require_parameters: true,
                    ..Default::default()
                },
            ),
            Self::LongCat => (
                "longcat-v1",
                WireRules {
                    readable_reasoning: true,
                    legacy_max_tokens: true,
                    chat_stop_diagnostics: true,
                    zero_usage_details: true,
                    chunk_created_drift: true,
                    ..Default::default()
                },
            ),
            Self::Nvidia => (
                "nvidia-v1",
                WireRules {
                    readable_reasoning: true,
                    legacy_max_tokens: true,
                    ..Default::default()
                },
            ),
            Self::Bailian => (
                "bailian-v1",
                WireRules {
                    readable_reasoning: true,
                    legacy_max_tokens: true,
                    inactive_chat_fields: true,
                    ..Default::default()
                },
            ),
            Self::Kimi => (
                "kimi-v1",
                WireRules {
                    readable_reasoning: true,
                    legacy_max_tokens: true,
                    ..Default::default()
                },
            ),
            Self::Zhipu => (
                "zhipu-v1",
                WireRules {
                    readable_reasoning: true,
                    legacy_max_tokens: true,
                    reported_request_id: true,
                    ..Default::default()
                },
            ),
            Self::Xiaomi => (
                "xiaomi-v1",
                WireRules {
                    readable_reasoning: true,
                    derived_output_text: true,
                    chunk_metadata_drift: true,
                    ..Default::default()
                },
            ),
        };
        Adaptation {
            rules,
            profile_id,
            scope,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Adapter {
    pub protocol: Profile,
    pub adaptation: Adaptation,
}
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error(transparent)]
    Codec(#[from] CodecError),
    #[error(transparent)]
    Representation(#[from] RepresentationError),
}
impl Adapter {
    pub fn new(protocol: Profile, dialect: Dialect, scope: Option<ReplayOrigin>) -> Self {
        Self {
            protocol,
            adaptation: dialect.adaptation(scope),
        }
    }
    pub fn contract(
        &self,
        contract: &GenerationRepresentationContract,
    ) -> GenerationRepresentationContract {
        GenerationRepresentationContract {
            adaptation: self.adaptation.clone(),
            ..contract.clone()
        }
    }
    pub fn decode_response(&self, bytes: &[u8]) -> Result<DecodedResponse, CodecError> {
        let value = openai::json::decode(bytes)?;
        let mut decoded = match self.protocol {
            Profile::Chat => {
                openai::chat_envelope::validate_response(&value)?;
                openai::static_response::decode_chat_with(&value, &self.adaptation)?
            }
            Profile::Responses => {
                openai::envelope::validate_response_snapshot(&value)?;
                openai::static_response::decode_responses_with(&value, &self.adaptation)?
            }
        };
        if let Some(scope) = &self.adaptation.scope {
            decoded.fidelity.bind_replay_origin(scope)?;
        }
        Ok(decoded)
    }
    pub fn encode_response(
        &self,
        response: &DecodedResponse,
        contract: &GenerationRepresentationContract,
    ) -> Result<Value, AdapterError> {
        let target = lower_response(
            &response.semantic,
            &response.fidelity,
            &response.metadata,
            self.protocol,
            self.contract(contract),
        )?;
        Ok(match self.protocol {
            Profile::Chat => openai::chat_envelope::encode_response(&target)?,
            Profile::Responses => openai::envelope::encode_response(&target)?,
        })
    }
    pub fn event_decoder(&self) -> EventDecoder {
        let decoder = EventDecoder::new(self.protocol).with_adaptation(self.adaptation.clone());
        match &self.adaptation.scope {
            Some(scope) => decoder.with_replay_origin(scope.clone()),
            None => decoder,
        }
    }
}
