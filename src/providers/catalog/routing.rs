//! 内置 Public Model 与其完整 Route 集合。
//!
//! 每个 Public Model 和它引用的 Route 在同一注册单元中生成，避免分别维护 Route ID 与候选顺序。

use crate::{
    core::ApiProtocol,
    registry::{PublicModelConfig, RouteConfig, RouteMode},
};

/// 编译目录使用的 Route 与 Public Model 聚合结果。
pub(super) struct CompiledRouting {
    pub(super) routes: Vec<RouteConfig>,
    pub(super) public_models: Vec<PublicModelConfig>,
}

/// 返回所有编译进二进制的 Public Model 及其 Route。
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

    // 按显式注册顺序共同生成 Route 与 Public Model 候选。
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
    /// 按注册的 surface 类型生成完整 route 与 Public Model 候选。
    fn compile(self) -> CompiledPublicModel {
        match self.surface {
            PublicModelSurface::DualProtocolWithBridges => self.compile_dual_protocol(),
            PublicModelSurface::DualProtocolNativeOnly => self.compile_dual_protocol_native_only(),
            PublicModelSurface::ChatNativeWithResponsesBridge => {
                self.compile_chat_native_with_responses_bridge()
            }
        }
    }

    /// 生成 Chat/Responses Native-first 与反向 Bridge 的完整 surface。
    fn compile_dual_protocol(self) -> CompiledPublicModel {
        // 生成两个 Native 与两个反向 Bridged Route 的稳定 ID。
        let chat = format!("{}-chat", self.route_prefix);
        let chat_via_responses = format!("{}-chat-via-responses", self.route_prefix);
        let responses = format!("{}-responses", self.route_prefix);
        let responses_via_chat = format!("{}-responses-via-chat", self.route_prefix);

        // 按下游协议的 Native-first 顺序构造完整 Route 集合。
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

        // 复用同一批 ID 构造 Public Model 的稳定候选顺序。
        let public_model = PublicModelConfig {
            name: self.public_name.to_owned(),
            routes: vec![chat, chat_via_responses, responses, responses_via_chat],
        };
        CompiledPublicModel {
            routes,
            public_model,
        }
    }

    /// 只生成两个协议的 Native surface，不加入 Bridge 或 fallback。
    fn compile_dual_protocol_native_only(self) -> CompiledPublicModel {
        // 生成 Chat 与 Responses Native Route，避免暗示 Bridge 可用。
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

        // 让 Public Model 只引用各协议当前唯一完整候选。
        let public_model = PublicModelConfig {
            name: self.public_name.to_owned(),
            routes: vec![chat, responses],
        };
        CompiledPublicModel {
            routes,
            public_model,
        }
    }

    /// 生成 Chat Native 与 Responses→Chat Bridge surface。
    fn compile_chat_native_with_responses_bridge(self) -> CompiledPublicModel {
        // 生成 Chat Native 与 Responses bridge 的稳定 ID。
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

        // 让两个下游协议各引用唯一完整候选。
        let public_model = PublicModelConfig {
            name: self.public_name.to_owned(),
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

/// 构造一个绑定 target、Upstream API、下游协议和处理模式的 route 定义。
fn route(
    id: &str,
    upstream_target: &str,
    upstream_api: &str,
    downstream_protocol: ApiProtocol,
    mode: RouteMode,
) -> RouteConfig {
    // 将调用点的协议方向和模式固化为一个不可变 route 定义。
    RouteConfig {
        id: id.to_owned(),
        upstream_target: upstream_target.to_owned(),
        upstream_api: upstream_api.to_owned(),
        downstream_protocol,
        mode,
    }
}
