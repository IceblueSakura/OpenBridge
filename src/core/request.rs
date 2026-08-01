//! 下游原生协议和已通过 HTTP 基础检查的请求值对象。
//!
//! `ApiRequest` 保留未经 provider 改写的 JSON bytes；只有 adapter 在确定 route 后才负责
//! 生成上游相对请求。

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

    /// 返回未修改的请求 JSON bytes。
    pub fn body(&self) -> &Bytes {
        &self.body
    }
}
