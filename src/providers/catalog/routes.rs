//! 内置 Public Model 使用的显式 Route 定义。

use crate::{
    core::ApiProtocol,
    registry::{RouteConfig, RouteMode},
};

pub(super) const CODE_PRIMARY_OPENAI_CHAT: &str = "code-primary-openai-chat";
pub(super) const CODE_PRIMARY_OPENAI_CHAT_VIA_RESPONSES: &str =
    "code-primary-openai-chat-via-responses";
pub(super) const CODE_PRIMARY_OPENAI_RESPONSES: &str = "code-primary-openai-responses";
pub(super) const CODE_PRIMARY_OPENAI_RESPONSES_VIA_CHAT: &str =
    "code-primary-openai-responses-via-chat";
pub(super) const LONGCAT_2_CHAT: &str = "longcat-2-chat";
pub(super) const LONGCAT_2_CHAT_VIA_RESPONSES: &str = "longcat-2-chat-via-responses";
pub(super) const LONGCAT_2_RESPONSES: &str = "longcat-2-responses";
pub(super) const LONGCAT_2_RESPONSES_VIA_CHAT: &str = "longcat-2-responses-via-chat";

/// 返回所有编译进二进制的 Route。
pub(super) fn compiled_routes() -> Vec<RouteConfig> {
    vec![
        native_route(
            CODE_PRIMARY_OPENAI_CHAT,
            "openai-main",
            "chat",
            ApiProtocol::ChatCompletions,
        ),
        bridged_route(
            CODE_PRIMARY_OPENAI_CHAT_VIA_RESPONSES,
            "openai-main",
            "responses",
            ApiProtocol::ChatCompletions,
        ),
        native_route(
            CODE_PRIMARY_OPENAI_RESPONSES,
            "openai-main",
            "responses",
            ApiProtocol::Responses,
        ),
        bridged_route(
            CODE_PRIMARY_OPENAI_RESPONSES_VIA_CHAT,
            "openai-main",
            "chat",
            ApiProtocol::Responses,
        ),
        native_route(
            LONGCAT_2_CHAT,
            "longcat-2",
            "chat",
            ApiProtocol::ChatCompletions,
        ),
        bridged_route(
            LONGCAT_2_CHAT_VIA_RESPONSES,
            "longcat-2",
            "responses",
            ApiProtocol::ChatCompletions,
        ),
        native_route(
            LONGCAT_2_RESPONSES,
            "longcat-2",
            "responses",
            ApiProtocol::Responses,
        ),
        bridged_route(
            LONGCAT_2_RESPONSES_VIA_CHAT,
            "longcat-2",
            "chat",
            ApiProtocol::Responses,
        ),
    ]
}

fn native_route(
    id: &str,
    upstream_target: &str,
    upstream_api: &str,
    downstream_protocol: ApiProtocol,
) -> RouteConfig {
    // 只构造保持下游协议原生一致的 route 定义。
    RouteConfig {
        id: id.to_owned(),
        upstream_target: upstream_target.to_owned(),
        upstream_api: upstream_api.to_owned(),
        downstream_protocol,
        mode: RouteMode::Native,
    }
}

fn bridged_route(
    id: &str,
    upstream_target: &str,
    upstream_api: &str,
    downstream_protocol: ApiProtocol,
) -> RouteConfig {
    // 只构造下游协议与 Upstream API 相反的受限转换 route 定义。
    RouteConfig {
        id: id.to_owned(),
        upstream_target: upstream_target.to_owned(),
        upstream_api: upstream_api.to_owned(),
        downstream_protocol,
        mode: RouteMode::Bridged,
    }
}
