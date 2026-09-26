//! Offline semantic ownership, codec projection and failure contracts, grouped by domain.
#[path = "support/semantic_events.rs"]
mod events_support;
#[path = "support/responses_profile.rs"]
mod wire;

#[path = "semantic/function_events.rs"]
mod function_events;
#[path = "semantic/instructions.rs"]
mod instructions;
#[path = "semantic/parsed_replay.rs"]
mod parsed_replay;
#[path = "semantic/phase.rs"]
mod phase;
#[path = "semantic/reasoning.rs"]
mod reasoning;
#[path = "semantic/response.rs"]
mod response;
#[path = "semantic/schema.rs"]
mod schema;
#[path = "semantic/text_events.rs"]
mod text_events;
#[path = "semantic/text_profile.rs"]
mod text_profile;
#[path = "semantic/tools.rs"]
mod tools;
