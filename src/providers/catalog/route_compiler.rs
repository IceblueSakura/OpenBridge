//! Compiles declarative generation surfaces into ordered registry Route definitions.
//!
//! The compiler owns typed Native-first/Source-first ordering, Route construction, and
//! missing-protocol Bridge supplementation. It does not own checked-in model identities or
//! request-time candidate selection.

use crate::{
    core::{ApiProtocol, OperationKind},
    registry::{ModelLifecycle, PublicModelConfig, RouteConfig, RouteMode},
};

use super::{
    public_models::{
        ProviderRouteRegistration, PublicModelRegistration, PublicModelRoutingStrategy,
        PublicModelSurface,
    },
    routing::{CompiledPublicModel, CompiledRouting},
};

/// Compiles generation Public Model registrations into ordered Route and Public Model entries.
pub(super) fn compile_generation_routing(
    registrations: &[PublicModelRegistration],
) -> CompiledRouting {
    // Build Route and Public Model candidates in explicit registration order.
    let mut routes = Vec::with_capacity(registrations.len() * 4);
    let mut public_models = Vec::with_capacity(registrations.len());
    for &registration in registrations {
        let compiled = registration.compile();
        routes.extend(compiled.routes);
        public_models.push(compiled.public_model);
    }

    CompiledRouting {
        routes,
        public_models,
    }
}

/// Global Route phase used to keep each downstream protocol's Native candidates ahead of its Bridges.
#[derive(Clone, Copy)]
enum RoutePhase {
    /// Native Chat Completions candidate.
    ChatNative,
    /// Chat Completions candidate bridged through Responses.
    ChatBridge,
    /// Native Responses candidate.
    ResponsesNative,
    /// Responses candidate bridged through Chat Completions.
    ResponsesBridge,
}

const ROUTE_PHASES: [RoutePhase; 4] = [
    RoutePhase::ChatNative,
    RoutePhase::ChatBridge,
    RoutePhase::ResponsesNative,
    RoutePhase::ResponsesBridge,
];

const CHAT_ROUTE_PHASES: [RoutePhase; 2] = [RoutePhase::ChatNative, RoutePhase::ChatBridge];
const RESPONSES_ROUTE_PHASES: [RoutePhase; 2] =
    [RoutePhase::ResponsesNative, RoutePhase::ResponsesBridge];

impl PublicModelRegistration {
    /// Builds one complete Public Model while preserving Provider priority inside each Route phase.
    fn compile(self) -> CompiledPublicModel {
        // Detect whether the Public Model already has a Native candidate for each downstream protocol.
        let has_chat_native = self
            .providers
            .iter()
            .any(|provider| provider.route_for(RoutePhase::ChatNative, false).is_some());
        let has_responses_native = self.providers.iter().any(|provider| {
            provider
                .route_for(RoutePhase::ResponsesNative, false)
                .is_some()
        });

        // Generate candidates under the Public Model's typed policy without changing provider priority.
        let mut routes = Vec::with_capacity(self.providers.len() * ROUTE_PHASES.len());
        match self.routing_strategy {
            PublicModelRoutingStrategy::NativeFirst => {
                for phase in ROUTE_PHASES {
                    for provider in self.providers {
                        if let Some(route) = provider.route_for(
                            phase,
                            supplement_missing_protocol(
                                phase,
                                has_chat_native,
                                has_responses_native,
                            ),
                        ) {
                            routes.push(route);
                        }
                    }
                }
            }
            PublicModelRoutingStrategy::SourceFirst => {
                for phases in [CHAT_ROUTE_PHASES, RESPONSES_ROUTE_PHASES] {
                    for provider in self.providers {
                        for phase in phases {
                            if let Some(route) = provider.route_for(
                                phase,
                                supplement_missing_protocol(
                                    phase,
                                    has_chat_native,
                                    has_responses_native,
                                ),
                            ) {
                                routes.push(route);
                            }
                        }
                    }
                }
            }
        }

        // Reuse the generated IDs as the private immutable execution order.
        let route_ids = routes.iter().map(|route| route.id.clone()).collect();
        let public_model = PublicModelConfig {
            id: self.public_name.to_owned(),
            created: 1_785_715_200,
            display_name: self.public_name.to_owned(),
            description: None,
            lifecycle: ModelLifecycle::active(),
            reasoning_level_policy: self.reasoning_level_policy,
            routes: route_ids,
        };
        CompiledPublicModel {
            routes,
            public_model,
        }
    }
}

/// Returns whether one Bridge phase may supplement a globally missing downstream protocol.
const fn supplement_missing_protocol(
    phase: RoutePhase,
    has_chat_native: bool,
    has_responses_native: bool,
) -> bool {
    match phase {
        RoutePhase::ChatBridge => !has_chat_native,
        RoutePhase::ResponsesBridge => !has_responses_native,
        RoutePhase::ChatNative | RoutePhase::ResponsesNative => false,
    }
}

impl ProviderRouteRegistration {
    /// Builds this Provider's Route for one phase when its surface and compiler policy allow it.
    fn route_for(
        self,
        phase: RoutePhase,
        supplement_missing_protocol: bool,
    ) -> Option<RouteConfig> {
        // Select the fixed protocol direction and handling mode for this surface and phase.
        let (suffix, upstream_operation, downstream_protocol, mode) = match phase {
            RoutePhase::ChatNative
                if !matches!(
                    self.surface,
                    PublicModelSurface::ResponsesNativeOnly
                        | PublicModelSurface::ResponsesNativeWithChatBridge
                ) =>
            {
                (
                    "chat",
                    OperationKind::ChatCompletions,
                    ApiProtocol::ChatCompletions,
                    RouteMode::Native,
                )
            }
            RoutePhase::ChatBridge
                if matches!(self.surface, PublicModelSurface::DualProtocolWithBridges)
                    || matches!(
                        self.surface,
                        PublicModelSurface::ResponsesNativeWithChatBridge
                    )
                    || (matches!(self.surface, PublicModelSurface::ResponsesNativeOnly)
                        && supplement_missing_protocol) =>
            {
                (
                    "chat-via-responses",
                    OperationKind::Responses,
                    ApiProtocol::ChatCompletions,
                    RouteMode::Bridged,
                )
            }
            RoutePhase::ResponsesNative
                if matches!(
                    self.surface,
                    PublicModelSurface::DualProtocolWithBridges
                        | PublicModelSurface::DualProtocolNativeOnly
                        | PublicModelSurface::ResponsesNativeOnly
                        | PublicModelSurface::ResponsesNativeWithChatBridge
                ) =>
            {
                (
                    "responses",
                    OperationKind::Responses,
                    ApiProtocol::Responses,
                    RouteMode::Native,
                )
            }
            RoutePhase::ResponsesBridge
                if matches!(self.surface, PublicModelSurface::DualProtocolWithBridges)
                    || (matches!(self.surface, PublicModelSurface::ChatNativeOnly)
                        && supplement_missing_protocol) =>
            {
                (
                    "responses-via-chat",
                    OperationKind::ChatCompletions,
                    ApiProtocol::Responses,
                    RouteMode::Bridged,
                )
            }
            _ => return None,
        };

        // Bind the phase-specific Route to this Provider Target with a stable ID.
        let id = format!("{}-{suffix}", self.route_prefix);
        Some(route(
            &id,
            self.upstream_target,
            upstream_operation,
            downstream_protocol,
            mode,
        ))
    }
}

/// Builds a Route definition bound to a Target, upstream operation, downstream protocol, and mode.
fn route(
    id: &str,
    upstream_target: &str,
    upstream_operation: OperationKind,
    downstream_protocol: ApiProtocol,
    mode: RouteMode,
) -> RouteConfig {
    // Freeze the call site's protocol direction and mode into an immutable Route definition.
    RouteConfig {
        id: id.to_owned(),
        upstream_target: upstream_target.to_owned(),
        upstream_operation,
        downstream_operation: downstream_protocol.operation(),
        mode,
    }
}
