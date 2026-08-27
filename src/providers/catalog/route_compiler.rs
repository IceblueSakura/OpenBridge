//! Compiles declarative generation surfaces into ordered registry Route definitions.
//!
//! The compiler owns typed Native-first/Source-first ordering, Route construction, and
//! missing-protocol Bridge supplementation. It does not own checked-in model identities or
//! request-time candidate selection.

use crate::{
    core::{ApiProtocol, OperationKind},
    registry::{ModelLifecycle, PublicModelConfig, RouteConfig},
};

use super::public_models::{
    ProviderRouteRegistration, PublicModelRegistration, PublicModelRoutingStrategy,
    PublicModelSurface,
};

/// Compiles generation Public Model registrations with their ordered typed Route candidates.
pub(super) fn compile_generation_routing(
    registrations: &[PublicModelRegistration],
) -> Vec<PublicModelConfig> {
    registrations
        .iter()
        .copied()
        .map(PublicModelRegistration::compile)
        .collect()
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
    fn compile(self) -> PublicModelConfig {
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

        PublicModelConfig {
            id: self.public_name.to_owned(),
            created: 1_785_715_200,
            display_name: self.public_name.to_owned(),
            description: None,
            lifecycle: ModelLifecycle::active(),
            reasoning_level_policy: self.reasoning_level_policy,
            routes,
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
        // Select the fixed operation pair for this surface and phase.
        let (upstream_operation, downstream_protocol) = match phase {
            RoutePhase::ChatNative
                if !matches!(
                    self.surface,
                    PublicModelSurface::ResponsesNativeOnly
                        | PublicModelSurface::ResponsesNativeWithChatBridge
                ) =>
            {
                (OperationKind::ChatCompletions, ApiProtocol::ChatCompletions)
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
                (OperationKind::Responses, ApiProtocol::ChatCompletions)
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
                (OperationKind::Responses, ApiProtocol::Responses)
            }
            RoutePhase::ResponsesBridge
                if matches!(self.surface, PublicModelSurface::DualProtocolWithBridges)
                    || (matches!(self.surface, PublicModelSurface::ChatNativeOnly)
                        && supplement_missing_protocol) =>
            {
                (OperationKind::ChatCompletions, ApiProtocol::Responses)
            }
            _ => return None,
        };

        // Bind the phase-specific typed Route directly to this Provider Target.
        Some(route(
            self.upstream_target,
            upstream_operation,
            downstream_protocol,
        ))
    }
}

/// Builds a Route definition bound to a Target and one typed operation pair.
fn route(
    upstream_target: &str,
    upstream_operation: OperationKind,
    downstream_protocol: ApiProtocol,
) -> RouteConfig {
    // Freeze the call site's operation pair; registry compilation derives the only valid mode.
    RouteConfig {
        upstream_target: upstream_target.to_owned(),
        upstream_operation,
        downstream_operation: downstream_protocol.operation(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::catalog::public_models::generation_registrations;

    #[test]
    fn muse_spark_uses_chat_native_and_responses_bridge() {
        let models = compile_generation_routing(generation_registrations());
        let muse = models
            .iter()
            .find(|model| model.id == "muse-spark-1.2-contributor")
            .expect("Muse Spark must remain in the public model catalog");
        let operations = muse
            .routes
            .iter()
            .map(|route| (route.upstream_operation, route.downstream_operation))
            .collect::<Vec<_>>();

        assert_eq!(
            operations,
            [
                (
                    OperationKind::ChatCompletions,
                    OperationKind::ChatCompletions,
                ),
                (OperationKind::ChatCompletions, OperationKind::Responses),
            ]
        );
    }

    #[test]
    fn glm_5_3_flash_keeps_responses_native_with_two_native_chat_candidates() {
        let models = compile_generation_routing(generation_registrations());
        let glm = models
            .iter()
            .find(|model| model.id == "glm-5.3-flash")
            .expect("GLM-5.3-Flash must be published through the production routing catalog");
        let operations = glm
            .routes
            .iter()
            .map(|route| (route.upstream_operation, route.downstream_operation))
            .collect::<Vec<_>>();

        assert_eq!(
            operations,
            [
                (
                    OperationKind::ChatCompletions,
                    OperationKind::ChatCompletions,
                ),
                (
                    OperationKind::ChatCompletions,
                    OperationKind::ChatCompletions,
                ),
                (OperationKind::Responses, OperationKind::Responses),
            ]
        );
    }
}
