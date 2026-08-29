//! Declarative generation Public Model registrations for the built-in catalog.
//!
//! This module owns checked-in downstream identities, trusted Target references, and declared
//! Native/Bridge surfaces. It does not construct RouteConfig values or perform request-time
//! routing; those responsibilities belong to the route compiler and pipeline planner.

use crate::registry::ReasoningLevelPolicy;

/// Returns the checked-in generation Public Model registrations in catalog order.
pub(super) fn generation_registrations() -> &'static [PublicModelRegistration] {
    &[
        PublicModelRegistration {
            public_name: "gpt-5.6-sol",
            routing_strategy: PublicModelRoutingStrategy::SourceFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "chatgpt/gpt-5-6-sol",
                surface: PublicModelSurface::ResponsesNativeWithChatBridge,
            }],
        },
        PublicModelRegistration {
            public_name: "gpt-5.6-terra",
            routing_strategy: PublicModelRoutingStrategy::SourceFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "chatgpt/gpt-5-6-terra",
                surface: PublicModelSurface::ResponsesNativeWithChatBridge,
            }],
        },
        PublicModelRegistration {
            public_name: "gpt-5.6-luna",
            routing_strategy: PublicModelRoutingStrategy::SourceFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "chatgpt/gpt-5-6-luna",
                surface: PublicModelSurface::ResponsesNativeWithChatBridge,
            }],
        },
        PublicModelRegistration {
            public_name: "gpt-5.5",
            routing_strategy: PublicModelRoutingStrategy::SourceFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "chatgpt/gpt-5-5",
                surface: PublicModelSurface::ResponsesNativeWithChatBridge,
            }],
        },
        PublicModelRegistration {
            public_name: "gpt-5.3-codex-spark",
            routing_strategy: PublicModelRoutingStrategy::SourceFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "chatgpt/gpt-5-3-codex-spark",
                surface: PublicModelSurface::ResponsesNativeWithChatBridge,
            }],
        },
        PublicModelRegistration {
            public_name: "LongCat-2.0",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "longcat-2",
                surface: PublicModelSurface::DualProtocolWithBridges,
            }],
        },
        PublicModelRegistration {
            public_name: "deepseek-v4-pro",
            routing_strategy: PublicModelRoutingStrategy::SourceFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[
                ProviderRouteRegistration {
                    upstream_target: "bailian/deepseek-v4-pro",
                    surface: PublicModelSurface::DualProtocolNativeOnly,
                },
                ProviderRouteRegistration {
                    upstream_target: "deepseek-v4-pro",
                    surface: PublicModelSurface::DualProtocolNativeOnly,
                },
            ],
        },
        PublicModelRegistration {
            public_name: "deepseek-v4-flash",
            routing_strategy: PublicModelRoutingStrategy::SourceFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[
                ProviderRouteRegistration {
                    upstream_target: "bailian/deepseek-v4-flash",
                    surface: PublicModelSurface::ChatNativeOnly,
                },
                ProviderRouteRegistration {
                    upstream_target: "deepseek-v4-flash",
                    surface: PublicModelSurface::DualProtocolNativeOnly,
                },
                ProviderRouteRegistration {
                    upstream_target: "openrouter/deepseek-v4-flash",
                    surface: PublicModelSurface::DualProtocolNativeOnly,
                },
            ],
        },
        PublicModelRegistration {
            public_name: "deepseek-v4-flash-vision-exp",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "deepseek-v4-flash-vision-exp",
                surface: PublicModelSurface::DualProtocolNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "minimax-m3",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[
                ProviderRouteRegistration {
                    upstream_target: "nvidia/minimax-m3",
                    surface: PublicModelSurface::ChatNativeOnly,
                },
                ProviderRouteRegistration {
                    upstream_target: "openrouter/minimax-m3",
                    surface: PublicModelSurface::DualProtocolNativeOnly,
                },
            ],
        },
        PublicModelRegistration {
            public_name: "gemma-4-31b-it",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "openrouter/gemma-4-31b-it",
                surface: PublicModelSurface::DualProtocolNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "gemini-3.7-flash",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "openrouter/gemini-3-7-flash",
                surface: PublicModelSurface::DualProtocolNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "grok-4.6",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "openrouter/grok-4-6",
                surface: PublicModelSurface::DualProtocolNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "muse-spark-1.2-contributor",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "openrouter/muse-spark-1.2-contributor",
                // Keep downstream Responses on the typed Chat bridge until native Responses
                // first-event framing has a verified compatibility baseline for this model.
                surface: PublicModelSurface::ChatNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "glm-5.3",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "zhipu-cn/glm-5-3",
                surface: PublicModelSurface::ChatNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "glm-5.3-flash",
            routing_strategy: PublicModelRoutingStrategy::SourceFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[
                ProviderRouteRegistration {
                    upstream_target: "zhipu-cn/glm-5-3-flash",
                    surface: PublicModelSurface::ChatNativeOnly,
                },
                ProviderRouteRegistration {
                    upstream_target: "openrouter/glm-5.3-flash",
                    surface: PublicModelSurface::DualProtocolNativeOnly,
                },
            ],
        },
        PublicModelRegistration {
            public_name: "glm-5.2",
            routing_strategy: PublicModelRoutingStrategy::SourceFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[
                ProviderRouteRegistration {
                    upstream_target: "bailian/glm-5-2",
                    surface: PublicModelSurface::ChatNativeOnly,
                },
                ProviderRouteRegistration {
                    upstream_target: "zhipu-cn/glm-5-2",
                    surface: PublicModelSurface::ChatNativeOnly,
                },
            ],
        },
        PublicModelRegistration {
            public_name: "kimi-k3",
            routing_strategy: PublicModelRoutingStrategy::SourceFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[
                ProviderRouteRegistration {
                    upstream_target: "bailian/kimi-k3",
                    surface: PublicModelSurface::ChatNativeOnly,
                },
                ProviderRouteRegistration {
                    upstream_target: "kimi-cn-kimi-k3",
                    surface: PublicModelSurface::ChatNativeOnly,
                },
            ],
        },
        PublicModelRegistration {
            public_name: "qwen3.7-plus",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "bailian/qwen3-7-plus",
                surface: PublicModelSurface::DualProtocolNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "qwen3.7-max",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "bailian/qwen3-7-max",
                surface: PublicModelSurface::DualProtocolNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "qwen3.8-max",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "bailian/qwen3-8-max",
                surface: PublicModelSurface::DualProtocolNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "qwen3.8-27b",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "bailian/qwen3-8-27b",
                surface: PublicModelSurface::DualProtocolNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "mimo-v2.5-pro",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "mimo-v2-5-pro",
                surface: PublicModelSurface::DualProtocolNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "mimo-v2.5",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::ClampPositiveFloor,
            providers: &[ProviderRouteRegistration {
                upstream_target: "mimo-v2-5",
                surface: PublicModelSurface::DualProtocolNativeOnly,
            }],
        },
        PublicModelRegistration {
            public_name: "mimo-v2.5-asr",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::Strict,
            providers: &[ProviderRouteRegistration {
                upstream_target: "mimo-v2-5-asr",
                surface: PublicModelSurface::ChatNativeOnlyWithoutBridge,
            }],
        },
        PublicModelRegistration {
            public_name: "mimo-v2.5-tts",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::Strict,
            providers: &[ProviderRouteRegistration {
                upstream_target: "mimo-v2-5-tts",
                surface: PublicModelSurface::ChatNativeOnlyWithoutBridge,
            }],
        },
        PublicModelRegistration {
            public_name: "mimo-v2.5-tts-voicedesign",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::Strict,
            providers: &[ProviderRouteRegistration {
                upstream_target: "mimo-v2-5-tts-voicedesign",
                surface: PublicModelSurface::ChatNativeOnlyWithoutBridge,
            }],
        },
        PublicModelRegistration {
            public_name: "mimo-v2.5-tts-voiceclone",
            routing_strategy: PublicModelRoutingStrategy::NativeFirst,
            reasoning_level_policy: ReasoningLevelPolicy::Strict,
            providers: &[ProviderRouteRegistration {
                upstream_target: "mimo-v2-5-tts-voiceclone",
                surface: PublicModelSurface::ChatNativeOnlyWithoutBridge,
            }],
        },
    ]
}

/// One downstream model identity with Provider route sources ordered by fallback priority.
#[derive(Clone, Copy)]
pub(super) struct PublicModelRegistration {
    /// Stable downstream Public Model identity.
    pub(super) public_name: &'static str,
    /// Typed candidate-ordering policy applied independently to each downstream protocol.
    pub(super) routing_strategy: PublicModelRoutingStrategy,
    /// Typed downstream reasoning-level acceptance and resolution policy.
    pub(super) reasoning_level_policy: ReasoningLevelPolicy,
    /// Provider Target surfaces in explicit fallback priority.
    pub(super) providers: &'static [ProviderRouteRegistration],
}

/// Candidate-ordering policy for one Public Model's fixed Provider sources.
#[derive(Clone, Copy)]
pub(super) enum PublicModelRoutingStrategy {
    /// Places every Native candidate before Bridge candidates while preserving source order per phase.
    NativeFirst,
    /// Preserves source priority first, then prefers Native over Bridge within each source.
    SourceFirst,
}

/// One Provider Target's executable protocol surface within a Public Model.
#[derive(Clone, Copy)]
pub(super) struct ProviderRouteRegistration {
    /// Trusted Upstream Target ID referenced by generated Routes.
    pub(super) upstream_target: &'static str,
    /// Native surface and automatic or explicit Bridge policy supplied by the Target.
    pub(super) surface: PublicModelSurface,
}

/// Native and Bridge surfaces that a Provider Target contributes to one Public Model.
///
/// Every variant is a valid reserved protocol-surface contract. Chat and Responses Native support,
/// together with either Bridge direction, must remain expressible even when the current catalog has
/// no registration using a particular combination; do not remove variants only because they are dead code.
#[derive(Clone, Copy)]
#[allow(
    dead_code,
    reason = "all Native and bidirectional Bridge surface combinations are reserved catalog semantics"
)]
pub(super) enum PublicModelSurface {
    /// Provides both Native protocols plus both reverse Bridge paths.
    DualProtocolWithBridges,
    /// Provides both Native protocols without Bridge paths.
    DualProtocolNativeOnly,
    /// Provides a Chat Native path and allows automatic Responses Bridge supplementation.
    ChatNativeOnly,
    /// Provides a Chat Native path without automatic Bridge supplementation for task-specific surfaces.
    ChatNativeOnlyWithoutBridge,
    /// Provides a Responses Native path and allows automatic Chat Bridge supplementation.
    ResponsesNativeOnly,
    /// Provides a Responses Native path and always contributes its explicit Chat Bridge.
    ResponsesNativeWithChatBridge,
}
