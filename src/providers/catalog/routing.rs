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
            public_name: "gpt-5.6-sol",
            providers: &[ProviderRouteRegistration {
                route_prefix: "gpt-5.6-sol-openai",
                upstream_target: "openai-main",
                surface: PublicModelSurface::DualProtocolWithBridges,
            }],
        },
        PublicModelRegistration {
            public_name: "LongCat-2.0",
            providers: &[ProviderRouteRegistration {
                route_prefix: "longcat-2",
                upstream_target: "longcat-2",
                surface: PublicModelSurface::DualProtocolWithBridges,
            }],
        },
        PublicModelRegistration {
            public_name: "deepseek-v4-pro",
            providers: &[ProviderRouteRegistration {
                route_prefix: "deepseek-v4-pro-deepseek",
                upstream_target: "deepseek-v4-pro",
                surface: PublicModelSurface::ChatNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "deepseek-v4-flash",
            providers: &[
                ProviderRouteRegistration {
                    route_prefix: "deepseek-v4-flash-deepseek",
                    upstream_target: "deepseek-v4-flash",
                    surface: PublicModelSurface::ChatNativeOnly,
                },
                ProviderRouteRegistration {
                    route_prefix: "deepseek-v4-flash-openrouter",
                    upstream_target: "openrouter-deepseek-v4-flash",
                    surface: PublicModelSurface::DualProtocolNativeOnly,
                },
            ],
        },
        PublicModelRegistration {
            public_name: "mimo-v2.5-pro",
            providers: &[ProviderRouteRegistration {
                route_prefix: "mimo-v2-5-pro-mimo",
                upstream_target: "mimo-v2-5-pro",
                surface: PublicModelSurface::DualProtocolWithBridges,
            }],
        },
        PublicModelRegistration {
            public_name: "mimo-v2.5",
            providers: &[ProviderRouteRegistration {
                route_prefix: "mimo-v2-5-mimo",
                upstream_target: "mimo-v2-5",
                surface: PublicModelSurface::DualProtocolWithBridges,
            }],
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

    // Add the independent Embeddings-only registration after all generation Public Models.
    let embedding = embedding_registration();
    routes.extend(embedding.routes);
    public_models.push(embedding.public_model);

    CompiledRouting {
        routes,
        public_models,
    }
}

/// Builds the single checked-in Embeddings Native Route and its Public Model.
fn embedding_registration() -> CompiledPublicModel {
    // Bind the downstream operation directly to the dedicated trusted target and API.
    let route = RouteConfig {
        id: "text-embedding-3-small-openai-embeddings".to_owned(),
        upstream_target: "openai-text-embedding-3-small".to_owned(),
        upstream_api: "embeddings".to_owned(),
        downstream_operation: crate::core::OperationKind::EmbeddingsCreate,
        mode: RouteMode::Native,
    };

    // Publish exactly that Route without adding Bridge or fallback candidates.
    let public_model = PublicModelConfig {
        id: "text-embedding-3-small".to_owned(),
        created: 1_785_715_200,
        display_name: "text-embedding-3-small".to_owned(),
        description: Some(
            "OpenAI text embedding model with a fixed Native execution path.".to_owned(),
        ),
        lifecycle: ModelLifecycle::active(),
        routes: vec![route.id.clone()],
    };
    CompiledPublicModel {
        routes: vec![route],
        public_model,
    }
}

/// One downstream model identity with Provider route sources ordered by fallback priority.
struct PublicModelRegistration {
    public_name: &'static str,
    providers: &'static [ProviderRouteRegistration],
}

/// One Provider target's executable protocol surface within a Public Model.
#[derive(Clone, Copy)]
struct ProviderRouteRegistration {
    route_prefix: &'static str,
    upstream_target: &'static str,
    surface: PublicModelSurface,
}

/// Native and Bridge surfaces that a Provider target contributes to one Public Model.
#[derive(Clone, Copy)]
enum PublicModelSurface {
    /// Provides both Native protocols plus both reverse Bridge paths.
    DualProtocolWithBridges,
    /// Provides both Native protocols without Bridge paths.
    DualProtocolNativeOnly,
    /// Provides only a Chat Completions Native path.
    ChatNativeOnly,
}

/// Global Route phase used to keep every Provider's Native candidate ahead of Bridge candidates.
#[derive(Clone, Copy)]
enum RoutePhase {
    ChatNative,
    ChatBridge,
    ResponsesNative,
    ResponsesBridge,
}

const ROUTE_PHASES: [RoutePhase; 4] = [
    RoutePhase::ChatNative,
    RoutePhase::ChatBridge,
    RoutePhase::ResponsesNative,
    RoutePhase::ResponsesBridge,
];

impl PublicModelRegistration {
    /// Builds one complete Public Model while preserving Provider priority inside each Route phase.
    fn compile(self) -> CompiledPublicModel {
        // Generate all Native candidates before Bridge candidates for each downstream protocol.
        let mut routes = Vec::with_capacity(self.providers.len() * ROUTE_PHASES.len());
        for phase in ROUTE_PHASES {
            for provider in self.providers {
                if let Some(route) = provider.route_for(phase) {
                    routes.push(route);
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
            routes: route_ids,
        };
        CompiledPublicModel {
            routes,
            public_model,
        }
    }
}

impl ProviderRouteRegistration {
    /// Builds this Provider's Route for one global phase when its surface supports that phase.
    fn route_for(self, phase: RoutePhase) -> Option<RouteConfig> {
        // Select the fixed protocol direction and handling mode for this surface and phase.
        let (suffix, upstream_api, downstream_protocol, mode) = match phase {
            RoutePhase::ChatNative => (
                "chat",
                "chat",
                ApiProtocol::ChatCompletions,
                RouteMode::Native,
            ),
            RoutePhase::ChatBridge
            if matches!(self.surface, PublicModelSurface::DualProtocolWithBridges) =>
                {
                    (
                        "chat-via-responses",
                        "responses",
                        ApiProtocol::ChatCompletions,
                        RouteMode::Bridged,
                    )
                }
            RoutePhase::ResponsesNative
            if matches!(
                    self.surface,
                    PublicModelSurface::DualProtocolWithBridges
                        | PublicModelSurface::DualProtocolNativeOnly
                ) =>
                {
                    (
                        "responses",
                        "responses",
                        ApiProtocol::Responses,
                        RouteMode::Native,
                    )
                }
            RoutePhase::ResponsesBridge
            if matches!(self.surface, PublicModelSurface::DualProtocolWithBridges) =>
                {
                    (
                        "responses-via-chat",
                        "chat",
                        ApiProtocol::Responses,
                        RouteMode::Bridged,
                    )
                }
            _ => return None,
        };

        // Bind the phase-specific Route to this Provider target with a stable ID.
        let id = format!("{}-{suffix}", self.route_prefix);
        Some(route(
            &id,
            self.upstream_target,
            upstream_api,
            downstream_protocol,
            mode,
        ))
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
        downstream_operation: downstream_protocol.operation(),
        mode,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProviderRouteRegistration, PublicModelRegistration, PublicModelSurface, RouteMode,
    };

    #[test]
    fn multiple_providers_are_compiled_native_first_for_each_protocol() {
        // Register two equivalent Provider targets in their explicit fallback priority.
        let registration = PublicModelRegistration {
            public_name: "shared-model",
            providers: &[
                ProviderRouteRegistration {
                    route_prefix: "shared-primary",
                    upstream_target: "primary-target",
                    surface: PublicModelSurface::DualProtocolWithBridges,
                },
                ProviderRouteRegistration {
                    route_prefix: "shared-secondary",
                    upstream_target: "secondary-target",
                    surface: PublicModelSurface::DualProtocolWithBridges,
                },
            ],
        };

        // Compile one Public Model and verify global Native-first order for both protocols.
        let compiled = registration.compile();
        let expected = [
            "shared-primary-chat",
            "shared-secondary-chat",
            "shared-primary-chat-via-responses",
            "shared-secondary-chat-via-responses",
            "shared-primary-responses",
            "shared-secondary-responses",
            "shared-primary-responses-via-chat",
            "shared-secondary-responses-via-chat",
        ];
        assert_eq!(compiled.public_model.routes, expected);
        assert_eq!(
            compiled
                .routes
                .iter()
                .map(|route| route.id.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(compiled.routes[0].upstream_target, "primary-target");
        assert_eq!(compiled.routes[1].upstream_target, "secondary-target");
        assert_eq!(compiled.routes[0].mode, RouteMode::Native);
        assert_eq!(compiled.routes[2].mode, RouteMode::Bridged);
    }
}
