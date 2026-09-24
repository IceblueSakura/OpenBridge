//! Framing, Responses HTTP/SSE and body I/O are separate contracts, not interchangeable evidence.
#[path = "support/responses_profile.rs"]
mod wire;

#[path = "transport/body_lifecycle.rs"]
mod body_lifecycle;
#[path = "transport/chat.rs"]
mod chat;
#[path = "transport/framing.rs"]
mod framing;
#[path = "transport/responses_sse.rs"]
mod responses_sse;
