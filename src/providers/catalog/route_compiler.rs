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

#[cfg(test)]
mod tests {
    use crate::registry::{ReasoningLevelPolicy, RouteMode};

    use super::super::public_models::{
        ProviderRouteRegistration, PublicModelRegistration, PublicModelRoutingStrategy,
        PublicModelSurface, generation_registrations,
    };

    #[test]
    fn gpt_sol_prefers_its_chatgpt_source_for_both_protocols() {
        // Compile the checked-in GPT registration whose first source is expected to own both protocol priorities.
        let registration = generation_registrations()
            .iter()
            .find(|registration| registration.public_name == "gpt-5.6-sol")
            .copied()
            .expect("gpt-5.6-sol registration must exist");

        // Keep ChatGPT ahead of the fallback source even when it requires a Chat protocol Bridge.
        let compiled = registration.compile();
        assert_eq!(
            compiled.public_model.routes,
            [
                "gpt-5.6-sol-chatgpt-chat-via-responses",
                "gpt-5.6-sol-openai-chat",
                "gpt-5.6-sol-openai-chat-via-responses",
                "gpt-5.6-sol-chatgpt-responses",
                "gpt-5.6-sol-openai-responses",
                "gpt-5.6-sol-openai-responses-via-chat",
            ]
        );
    }

    #[test]
    fn checked_in_public_models_choose_a_reasoning_input_policy() {
        // Keep task-specific audio surfaces strict while making general generation Agent-friendly.
        for registration in generation_registrations() {
            let expected = if registration.public_name.starts_with("mimo-v2.5-asr")
                || registration.public_name.starts_with("mimo-v2.5-tts")
            {
                ReasoningLevelPolicy::Strict
            } else {
                ReasoningLevelPolicy::ClampPositiveFloor
            };
            assert_eq!(
                registration.reasoning_level_policy, expected,
                "{}",
                registration.public_name
            );
        }
    }

    #[test]
    fn deepseek_flash_uses_source_first_with_bailian_before_openrouter() {
        // Load the checked-in DeepSeek Flash registration and require its explicit source-first policy.
        let registration = generation_registrations()
            .iter()
            .find(|registration| registration.public_name == "deepseek-v4-flash")
            .copied()
            .expect("deepseek-v4-flash registration must exist");
        assert!(matches!(
            registration.routing_strategy,
            PublicModelRoutingStrategy::SourceFirst
        ));

        // Preserve Bailian ahead of OpenRouter for Chat while retaining the two Native Responses sources.
        let compiled = registration.compile();
        assert_eq!(
            compiled.public_model.routes,
            [
                "deepseek-v4-flash-deepseek-chat",
                "deepseek-v4-flash-bailian-chat",
                "deepseek-v4-flash-openrouter-chat",
                "deepseek-v4-flash-deepseek-responses",
                "deepseek-v4-flash-openrouter-responses",
            ]
        );
    }

    #[test]
    fn multiple_providers_are_compiled_native_first_for_each_protocol() {
        // Register two equivalent Provider targets in their explicit fallback priority.
        let registration = PublicModelRegistration {
            public_name: "shared-model",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::Strict,
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

    #[test]
    fn chat_only_surface_auto_supplements_missing_responses_coverage() {
        // Register a Chat-only source and require the compiler to fill the missing downstream protocol.
        let registration = PublicModelRegistration {
            public_name: "chat-only-model",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::Strict,
            providers: &[ProviderRouteRegistration {
                route_prefix: "chat-only-provider",
                upstream_target: "chat-only-target",
                surface: PublicModelSurface::ChatNativeOnly,
            }],
        };

        // Keep the Native Chat candidate first and append the reverse Bridge for Responses.
        let compiled = registration.compile();
        let expected = [
            "chat-only-provider-chat",
            "chat-only-provider-responses-via-chat",
        ];
        assert_eq!(compiled.public_model.routes, expected);
        assert_eq!(compiled.routes[0].mode, RouteMode::Native);
        assert_eq!(compiled.routes[1].mode, RouteMode::Bridged);
    }

    #[test]
    fn responses_only_surface_auto_supplements_missing_chat_coverage() {
        // Register a Responses-only source and require the compiler to fill the missing Chat protocol.
        let registration = PublicModelRegistration {
            public_name: "responses-only-model",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::Strict,
            providers: &[ProviderRouteRegistration {
                route_prefix: "responses-only-provider",
                upstream_target: "responses-only-target",
                surface: PublicModelSurface::ResponsesNativeOnly,
            }],
        };

        // Keep the reverse Chat Bridge and Responses Native candidates in their protocol phases.
        let compiled = registration.compile();
        let expected = [
            "responses-only-provider-chat-via-responses",
            "responses-only-provider-responses",
        ];
        assert_eq!(compiled.public_model.routes, expected);
        assert_eq!(compiled.routes[0].mode, RouteMode::Bridged);
        assert_eq!(compiled.routes[1].mode, RouteMode::Native);
    }

    #[test]
    fn responses_only_surface_can_explicitly_keep_its_chat_bridge() {
        // Combine another source's Native Chat path with one Responses-only explicit fallback Bridge.
        let registration = PublicModelRegistration {
            public_name: "mixed-model",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::Strict,
            providers: &[
                ProviderRouteRegistration {
                    route_prefix: "native-primary",
                    upstream_target: "native-primary-target",
                    surface: PublicModelSurface::DualProtocolNativeOnly,
                },
                ProviderRouteRegistration {
                    route_prefix: "responses-fallback",
                    upstream_target: "responses-fallback-target",
                    surface: PublicModelSurface::ResponsesNativeWithChatBridge,
                },
            ],
        };

        // Preserve Native-first ordering while retaining the explicitly declared fallback Bridge.
        let compiled = registration.compile();
        let expected = [
            "native-primary-chat",
            "responses-fallback-chat-via-responses",
            "native-primary-responses",
            "responses-fallback-responses",
        ];
        assert_eq!(compiled.public_model.routes, expected);
        assert_eq!(compiled.routes[0].mode, RouteMode::Native);
        assert_eq!(compiled.routes[1].mode, RouteMode::Bridged);
    }

    #[test]
    fn source_first_keeps_a_primary_bridge_ahead_of_a_fallback_native_route() {
        // Put one Responses-native source before a complete Native fallback source.
        let registration = PublicModelRegistration {
            public_name: "source-first-model",
            routing_strategy: PublicModelRoutingStrategy::SourceFirst,
            reasoning_level_policy: ReasoningLevelPolicy::Strict,
            providers: &[
                ProviderRouteRegistration {
                    route_prefix: "responses-primary",
                    upstream_target: "responses-primary-target",
                    surface: PublicModelSurface::ResponsesNativeWithChatBridge,
                },
                ProviderRouteRegistration {
                    route_prefix: "native-fallback",
                    upstream_target: "native-fallback-target",
                    surface: PublicModelSurface::DualProtocolNativeOnly,
                },
            ],
        };

        // Preserve source priority independently for Chat and Responses candidate groups.
        let compiled = registration.compile();
        assert_eq!(
            compiled.public_model.routes,
            [
                "responses-primary-chat-via-responses",
                "native-fallback-chat",
                "responses-primary-responses",
                "native-fallback-responses",
            ]
        );
        assert_eq!(compiled.routes[0].mode, RouteMode::Bridged);
        assert_eq!(compiled.routes[1].mode, RouteMode::Native);
    }

    #[test]
    fn complete_native_coverage_does_not_add_redundant_automatic_bridges() {
        // Combine a complete Native source with a Chat-only fallback source.
        let registration = PublicModelRegistration {
            public_name: "complete-native-model",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::Strict,
            providers: &[
                ProviderRouteRegistration {
                    route_prefix: "complete-primary",
                    upstream_target: "complete-primary-target",
                    surface: PublicModelSurface::DualProtocolNativeOnly,
                },
                ProviderRouteRegistration {
                    route_prefix: "complete-chat-fallback",
                    upstream_target: "complete-chat-fallback-target",
                    surface: PublicModelSurface::ChatNativeOnly,
                },
            ],
        };

        // Do not create a Chat-only fallback Bridge when the Public Model already has both Native protocols.
        let compiled = registration.compile();
        let expected = [
            "complete-primary-chat",
            "complete-chat-fallback-chat",
            "complete-primary-responses",
        ];
        assert_eq!(compiled.public_model.routes, expected);
    }
}
