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
        },
        PublicModelRegistration {
            public_name: "LongCat-2.0",
            route_prefix: "longcat-2",
            upstream_target: "longcat-2",
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
}

impl PublicModelRegistration {
    fn compile(self) -> CompiledPublicModel {
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
}

struct CompiledPublicModel {
    routes: Vec<RouteConfig>,
    public_model: PublicModelConfig,
}

fn route(
    id: &str,
    upstream_target: &str,
    upstream_api: &str,
    downstream_protocol: ApiProtocol,
    mode: RouteMode,
) -> RouteConfig {
    RouteConfig {
        id: id.to_owned(),
        upstream_target: upstream_target.to_owned(),
        upstream_api: upstream_api.to_owned(),
        downstream_protocol,
        mode,
    }
}
