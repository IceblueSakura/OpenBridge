//! Offline semantic ownership, codec projection and failure contracts, grouped by domain.
#[path = "support/semantic_events.rs"]
mod events_support;
#[path = "support/responses_profile.rs"]
mod wire;

#[path = "semantic/adapters.rs"]
mod adapters;
#[path = "semantic/billing_modal_usage.rs"]
mod billing_modal_usage;
#[path = "semantic/chat_logprobs.rs"]
mod chat_logprobs;
#[path = "semantic/chat_wire.rs"]
mod chat_wire;
#[path = "semantic/extensions.rs"]
mod extensions;
#[path = "semantic/function_events.rs"]
mod function_events;
#[path = "semantic/image_usage.rs"]
mod image_usage;
#[path = "semantic/images.rs"]
mod images;
#[path = "semantic/instructions.rs"]
mod instructions;
#[path = "semantic/parsed_replay.rs"]
mod parsed_replay;
#[path = "semantic/phase.rs"]
mod phase;
#[path = "semantic/provider_profiles.rs"]
mod provider_profiles;
#[path = "semantic/reasoning.rs"]
mod reasoning;
#[path = "semantic/reasoning_boundary.rs"]
mod reasoning_boundary;
#[path = "semantic/response.rs"]
mod response;
#[path = "semantic/router_adapter.rs"]
mod router_adapter;
#[path = "semantic/schema.rs"]
mod schema;
#[path = "semantic/string_enums.rs"]
mod string_enums;
#[path = "semantic/text_admission.rs"]
mod text_admission;
#[path = "semantic/text_events.rs"]
mod text_events;
#[path = "semantic/text_profile.rs"]
mod text_profile;
#[path = "semantic/tools.rs"]
mod tools;
#[path = "semantic/unreported_event_probabilities.rs"]
mod unreported_event_probabilities;
#[path = "semantic/usage.rs"]
mod usage;
#[path = "semantic/vendor_shapes.rs"]
mod vendor_shapes;
