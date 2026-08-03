//! Built-in Public Models and their complete Route sets.
//!
//! Each Public Model and its referenced Routes are created in one registration unit so that
//! Route IDs and candidate order cannot drift apart.

use crate::{
    core::ApiProtocol,
    registry::{ModelLifecycle, PublicModelConfig, RouteConfig, RouteMode},
};

/// Aggregated Route and Public Model definitions used by the compiled catalog.
pub(super) struct CompiledRouting {
    pub(super) routes: Vec<RouteConfig>,
    pub(super) public_models: Vec<PublicModelConfig>,
}

/// Returns all Public Models and their Routes compiled into the binary.
pub(super) fn compiled_routing() -> CompiledRouting {
    let registrations = [
        PublicModelRegistration {
            public_name: "code-primary",
            route_prefix: "code-primary-openai",
            upstream_target: "openai-main",
            surface: PublicModelSurface::DualProtocolWithBridges,
        },
        PublicModelRegistration {
            public_name: "LongCat-2.0",
            route_prefix: "longcat-2",
            upstream_target: "longcat-2",
            surface: PublicModelSurface::DualProtocolWithBridges,
        },
        PublicModelRegistration {
            public_name: "nemotron-3-ultra",
            route_prefix: "nemotron-3-ultra-openrouter",
            upstream_target: "openrouter-nemotron-3-ultra",
            surface: PublicModelSurface::DualProtocolNativeOnly,
        },
        PublicModelRegistration {
            public_name: "deepseek-v4-pro",
            route_prefix: "deepseek-v4-pro-deepseek",
            upstream_target: "deepseek-v4-pro",
            surface: PublicModelSurface::ChatNativeWithResponsesBridge,
        },
        PublicModelRegistration {
            public_name: "deepseek-v4-flash",
            route_prefix: "deepseek-v4-flash-deepseek",
            upstream_target: "deepseek-v4-flash",
            surface: PublicModelSurface::ChatNativeWithResponsesBridge,
        },
        PublicModelRegistration {
            public_name: "mimo-v2.5-pro",
            route_prefix: "mimo-v2-5-pro-mimo",
            upstream_target: "mimo-v2-5-pro",
            surface: PublicModelSurface::DualProtocolWithBridges,
        },
        PublicModelRegistration {
            public_name: "mimo-v2.5",
            route_prefix: "mimo-v2-5-mimo",
            upstream_target: "mimo-v2-5",
            surface: PublicModelSurface::DualProtocolWithBridges,
        },
    ];

    // Build Route and Public Model candidates in explicit registration order.
    let mut routes = Vec::with_capacity(registrations.len() * 4);
    let mut public_models = Vec::with_capacity(registrations.len());
    for registration in registrations {
        let compiled = registration.compile();
        routes.extend(compiled.routes);
        public_models.push(compiled.public_model);
    }

    CompiledRouting {
        routes,
        public_models,
    }
}

struct PublicModelRegistration {
    public_name: &'static str,
    route_prefix: &'static str,
    upstream_target: &'static str,
    surface: PublicModelSurface,
}

#[derive(Clone, Copy)]
enum PublicModelSurface {
    DualProtocolWithBridges,
    DualProtocolNativeOnly,
    ChatNativeWithResponsesBridge,
}

impl PublicModelRegistration {
    /// Builds the complete Route and Public Model candidates for a registered surface.
    fn compile(self) -> CompiledPublicModel {
        match self.surface {
            PublicModelSurface::DualProtocolWithBridges => self.compile_dual_protocol(),
            PublicModelSurface::DualProtocolNativeOnly => self.compile_dual_protocol_native_only(),
            PublicModelSurface::ChatNativeWithResponsesBridge => {
                self.compile_chat_native_with_responses_bridge()
            }
        }
    }

    /// Builds the complete Chat/Responses surface with Native-first ordering and reverse Bridge candidates.
    fn compile_dual_protocol(self) -> CompiledPublicModel {
        // Assign stable IDs to the two Native and two reverse Bridged Routes.
        let chat = format!("{}-chat", self.route_prefix);
        let chat_via_responses = format!("{}-chat-via-responses", self.route_prefix);
        let responses = format!("{}-responses", self.route_prefix);
        let responses_via_chat = format!("{}-responses-via-chat", self.route_prefix);

        // Build the complete Route set in downstream-protocol Native-first order.
        let routes = vec![
            route(
                &chat,
                self.upstream_target,
                "chat",
                ApiProtocol::ChatCompletions,
                RouteMode::Native,
            ),
            route(
                &chat_via_responses,
                self.upstream_target,
                "responses",
                ApiProtocol::ChatCompletions,
                RouteMode::Bridged,
            ),
            route(
                &responses,
                self.upstream_target,
                "responses",
                ApiProtocol::Responses,
                RouteMode::Native,
            ),
            route(
                &responses_via_chat,
                self.upstream_target,
                "chat",
                ApiProtocol::Responses,
                RouteMode::Bridged,
            ),
        ];

        // Reuse the same IDs to build the stable Public Model candidate order.
        let public_model = PublicModelConfig {
            id: self.public_name.to_owned(),
            created: 1_785_715_200,
            display_name: self.public_name.to_owned(),
            description: None,
            lifecycle: ModelLifecycle::active(),
            routes: vec![chat, chat_via_responses, responses, responses_via_chat],
        };
        CompiledPublicModel {
            routes,
            public_model,
        }
    }

    /// Builds only the two Native surfaces without Bridge or fallback candidates.
    fn compile_dual_protocol_native_only(self) -> CompiledPublicModel {
        // Build Chat and Responses Native Routes without implying Bridge support.
        let chat = format!("{}-chat", self.route_prefix);
        let responses = format!("{}-responses", self.route_prefix);
        let routes = vec![
            route(
                &chat,
                self.upstream_target,
                "chat",
                ApiProtocol::ChatCompletions,
                RouteMode::Native,
            ),
            route(
                &responses,
                self.upstream_target,
                "responses",
                ApiProtocol::Responses,
                RouteMode::Native,
            ),
        ];

        // Make each Public Model reference its only complete candidate for the protocol.
        let public_model = PublicModelConfig {
            id: self.public_name.to_owned(),
            created: 1_785_715_200,
            display_name: self.public_name.to_owned(),
            description: None,
            lifecycle: ModelLifecycle::active(),
            routes: vec![chat, responses],
        };
        CompiledPublicModel {
            routes,
            public_model,
        }
    }

    /// Builds a Chat Native surface and a Responses-to-Chat Bridge surface.
    fn compile_chat_native_with_responses_bridge(self) -> CompiledPublicModel {
        // Assign stable IDs to the Chat Native and Responses Bridge Routes.
        let chat = format!("{}-chat", self.route_prefix);
        let responses_via_chat = format!("{}-responses-via-chat", self.route_prefix);
        let routes = vec![
            route(
                &chat,
                self.upstream_target,
                "chat",
                ApiProtocol::ChatCompletions,
                RouteMode::Native,
            ),
            route(
                &responses_via_chat,
                self.upstream_target,
                "chat",
                ApiProtocol::Responses,
                RouteMode::Bridged,
            ),
        ];

        // Make each downstream protocol reference its only complete candidate.
        let public_model = PublicModelConfig {
            id: self.public_name.to_owned(),
            created: 1_785_715_200,
            display_name: self.public_name.to_owned(),
            description: None,
            lifecycle: ModelLifecycle::active(),
            routes: vec![chat, responses_via_chat],
        };
        CompiledPublicModel {
            routes,
            public_model,
        }
    }
}

struct CompiledPublicModel {
    routes: Vec<RouteConfig>,
    public_model: PublicModelConfig,
}

/// Builds a Route definition bound to a target, Upstream API, downstream protocol, and handling mode.
fn route(
    id: &str,
    upstream_target: &str,
    upstream_api: &str,
    downstream_protocol: ApiProtocol,
    mode: RouteMode,
) -> RouteConfig {
    // Freeze the call site's protocol direction and mode into an immutable Route definition.
    RouteConfig {
        id: id.to_owned(),
        upstream_target: upstream_target.to_owned(),
        upstream_api: upstream_api.to_owned(),
        downstream_protocol,
        mode,
    }
}
