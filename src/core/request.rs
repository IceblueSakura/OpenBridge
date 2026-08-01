//! 下游原生协议和已通过 HTTP 基础检查的请求值对象。
//!
//! `ApiRequest` 保存 RoutePlan 已确定协议的 JSON bytes：Native Route 保留下游 body，Bridged
//! Route 保存 `BridgePlan` 生成的目标协议 body；adapter 随后负责真实 model 与上游相对请求。

use bytes::Bytes;

/// OpenAI-compatible 下游请求所使用的原生协议。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiProtocol {
    /// OpenAI Chat Completions 协议。
    ChatCompletions,
    /// OpenAI Responses 协议。
    Responses,
}

/// 已完成 HTTP 层基础检查、可交给 provider adapter 的请求。
#[derive(Clone, Debug)]
pub struct ApiRequest {
    protocol: ApiProtocol,
    body: Bytes,
}

impl ApiRequest {
    /// 创建一个带协议标识的请求视图。
    pub fn new(protocol: ApiProtocol, body: Bytes) -> Self {
        Self { protocol, body }
    }

    /// 返回请求所属的原生协议。
    pub fn protocol(&self) -> ApiProtocol {
        self.protocol
    }

    /// 返回当前执行协议的请求 JSON bytes。
    pub fn body(&self) -> &Bytes {
        &self.body
    }
}
