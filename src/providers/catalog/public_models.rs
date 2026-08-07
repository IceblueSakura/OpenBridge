//! Declarative generation Public Model registrations for the built-in catalog.
//!
//! This module owns checked-in downstream identities, trusted Target references, and declared
//! Native/Bridge surfaces. It does not construct RouteConfig values or perform request-time
//! routing; those responsibilities belong to the route compiler and pipeline planner.

/// Returns the checked-in generation Public Model registrations in catalog order.
pub(super) fn generation_registrations() -> &'static [PublicModelRegistration] {
    &[
        PublicModelRegistration {
            public_name: "gpt-5.6-sol",
            providers: &[
                ProviderRouteRegistration {
                    route_prefix: "gpt-5.6-sol-openai",
                    upstream_target: "openai-main",
                    surface: PublicModelSurface::DualProtocolWithBridges,
                },
                ProviderRouteRegistration {
                    route_prefix: "gpt-5.6-sol-chatgpt",
                    upstream_target: "chatgpt-gpt-5-6-sol",
                    surface: PublicModelSurface::ResponsesNativeWithChatBridge,
                },
            ],
        },
        PublicModelRegistration {
            public_name: "chatgpt-gpt-5.3-codex-spark",
            providers: &[ProviderRouteRegistration {
                route_prefix: "chatgpt-gpt-5-3-codex-spark",
                upstream_target: "chatgpt-gpt-5-3-codex-spark",
                surface: PublicModelSurface::ResponsesNativeWithChatBridge,
            }],
        },
        PublicModelRegistration {
            public_name: "chatgpt-gpt-5.5",
            providers: &[ProviderRouteRegistration {
                route_prefix: "chatgpt-gpt-5-5",
                upstream_target: "chatgpt-gpt-5-5",
                surface: PublicModelSurface::ResponsesNativeWithChatBridge,
            }],
        },
        PublicModelRegistration {
            public_name: "chatgpt-gpt-5.6-luna",
            providers: &[ProviderRouteRegistration {
                route_prefix: "chatgpt-gpt-5-6-luna",
                upstream_target: "chatgpt-gpt-5-6-luna",
                surface: PublicModelSurface::ResponsesNativeWithChatBridge,
            }],
        },
        PublicModelRegistration {
            public_name: "chatgpt-gpt-5.6-terra",
            providers: &[ProviderRouteRegistration {
                route_prefix: "chatgpt-gpt-5-6-terra",
                upstream_target: "chatgpt-gpt-5-6-terra",
                surface: PublicModelSurface::ResponsesNativeWithChatBridge,
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
                surface: PublicModelSurface::DualProtocolNativeOnly,
            }],
        },
    ]
}

/// One downstream model identity with Provider route sources ordered by fallback priority.
#[derive(Clone, Copy)]
pub(super) struct PublicModelRegistration {
    /// Stable downstream Public Model identity.
    pub(super) public_name: &'static str,
    /// Provider Target surfaces in explicit fallback priority.
    pub(super) providers: &'static [ProviderRouteRegistration],
}

/// One Provider Target's executable protocol surface within a Public Model.
#[derive(Clone, Copy)]
pub(super) struct ProviderRouteRegistration {
    /// Prefix used to derive stable Route IDs.
    pub(super) route_prefix: &'static str,
    /// Trusted Upstream Target ID referenced by generated Routes.
    pub(super) upstream_target: &'static str,
    /// Native and Bridge surface supplied by the Target.
    pub(super) surface: PublicModelSurface,
}

/// Native and Bridge surfaces that a Provider Target contributes to one Public Model.
#[derive(Clone, Copy)]
pub(super) enum PublicModelSurface {
    /// Provides both Native protocols plus both reverse Bridge paths.
    DualProtocolWithBridges,
    /// Provides both Native protocols without Bridge paths.
    DualProtocolNativeOnly,
    /// Provides only a Chat Completions Native path.
    ChatNativeOnly,
    /// Provides a Responses Native path and a Chat Completions path bridged through Responses.
    ResponsesNativeWithChatBridge,
}
