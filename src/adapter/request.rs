//! Unified request representation and pure target-context projection.
use super::{Adapter, AdapterError};
use crate::{
    lowering::generation::{GenerationRepresentationContract, RepresentationError, lower_request},
    protocol::{
        extensions::CustomSections,
        openai::{self, CodecError, DecodedRequest, Profile, chat_envelope, envelope},
    },
    semantic::{
        context::{CacheHints, DeliveryIntent, ExecutionHints, StreamOptions},
        value::Presence,
    },
};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub task: DecodedRequest,
    pub model: String,
    pub context: ExecutionHints,
    pub delivery: DeliveryIntent,
    pub extensions: CustomSections,
    source: Profile,
    n: Presence<u64>,
}
impl Request {
    fn check_context(
        &self,
        contract: &GenerationRepresentationContract,
    ) -> Result<(), AdapterError> {
        self.context.validate().map_err(CodecError::from)?;
        self.delivery.validate().map_err(CodecError::from)?;
        if self.extensions != CustomSections::default()
            || !contract.standard_context
                && (!self.context.metadata.is_absent()
                    || !self.context.service_tier.is_absent()
                    || !self.context.max_tool_calls.is_absent())
        {
            return Err(RepresentationError::UnmigratedSemantic.into());
        }
        Ok(())
    }
    pub fn check(
        &self,
        contract: &GenerationRepresentationContract,
    ) -> Result<crate::semantic::task::generation::GenerationRequirements, AdapterError> {
        self.check_context(contract)?;
        Ok(crate::lowering::generation::check(
            &self.task.semantic,
            contract.clone(),
        )?)
    }
}
impl Adapter {
    pub fn decode_request(&self, bytes: &[u8]) -> Result<Request, CodecError> {
        let value = openai::json::decode(bytes)?;
        let mut request = match self.protocol {
            Profile::Chat => {
                let decoded = chat_envelope::decode_request_with(&value, &self.adaptation)?;
                Request {
                    task: decoded.task,
                    model: decoded.context.model,
                    context: ExecutionHints {
                        cache: decoded.context.cache,
                        ..Default::default()
                    },
                    delivery: DeliveryIntent {
                        stream: decoded.context.stream,
                        options: decoded.context.stream_options,
                    },
                    extensions: CustomSections::default(),
                    source: self.protocol,
                    n: decoded.context.n,
                }
            }
            Profile::Responses => {
                let decoded = envelope::decode_request(&value)?;
                let options = match decoded.context.delivery.options {
                    Presence::Absent => Presence::Absent,
                    Presence::Null => Presence::Null,
                    Presence::Value(value) => Presence::Value(StreamOptions {
                        include_obfuscation: value.include_obfuscation,
                        ..Default::default()
                    }),
                };
                Request {
                    task: decoded.task,
                    model: decoded.context.model,
                    context: decoded.context.execution,
                    delivery: DeliveryIntent {
                        stream: decoded.context.delivery.stream,
                        options,
                    },
                    extensions: decoded.context.extensions,
                    source: self.protocol,
                    n: Presence::Absent,
                }
            }
        };
        request.delivery.validate()?;
        if let Some(scope) = &self.adaptation.scope {
            request.task.fidelity.bind_replay_origin(scope)?;
        }
        Ok(request)
    }
    pub fn encode_request(
        &self,
        request: &Request,
        model: &str,
        contract: &GenerationRepresentationContract,
    ) -> Result<Value, AdapterError> {
        request.check_context(contract)?;
        let mut context = request.context.clone();
        if !contract.cache_hints {
            context.cache = CacheHints::default();
        }
        if self.protocol == Profile::Chat
            && (!context.metadata.is_absent()
                || !context.service_tier.is_absent()
                || !context.max_tool_calls.is_absent())
        {
            return Err(RepresentationError::UnmigratedSemantic.into());
        }
        let target = lower_request(
            &request.task.semantic,
            &request.task.fidelity,
            self.protocol,
            self.contract(contract),
        )?;
        Ok(match self.protocol {
            Profile::Chat => {
                let mut options = request.delivery.options.clone();
                // Cross-protocol delivery still needs actual upstream usage; request
                // its wire report rather than synthesizing any counters.
                if request.source != self.protocol && request.delivery.streaming() {
                    let mut common = options.value().cloned().unwrap_or_default();
                    common.include_usage = Presence::Value(true);
                    options = Presence::Value(common);
                }
                chat_envelope::encode_request(
                    &target,
                    &chat_envelope::RequestContext {
                        model: model.into(),
                        n: if request.source == self.protocol {
                            request.n.clone()
                        } else {
                            Presence::Absent
                        },
                        stream: request.delivery.stream.clone(),
                        stream_options: options,
                        cache: context.cache,
                    },
                )?
            }
            Profile::Responses => {
                let options = match &request.delivery.options {
                    Presence::Absent => Presence::Absent,
                    Presence::Null => Presence::Null,
                    Presence::Value(value) => Presence::Value(envelope::StreamOptions {
                        include_obfuscation: value.include_obfuscation.clone(),
                    }),
                };
                envelope::encode_request(
                    &target,
                    &envelope::RequestContext {
                        model: model.into(),
                        execution: context,
                        delivery: envelope::Delivery {
                            stream: request.delivery.stream.clone(),
                            options,
                        },
                        extensions: CustomSections::default(),
                    },
                )?
            }
        })
    }
}
