//! 受限 Chat Completions 与 Responses 双向转换的公开计划与 renderer 门面。
//!
//! 请求、非流式响应、流式响应和共享 wire 辅助逻辑分别位于私有子模块。本模块保持
//! `BridgePlan`、`BridgeStreamRenderer` 与错误类型的稳定边界，不执行 tool，也不放宽
//! Provider 私有扩展或未建模语义的 fail-closed 规则。

use bytes::Bytes;
use thiserror::Error;

use crate::{
    core::{ApiProtocol, ApiRequest},
    transport::sse::SseEvent,
};

use super::BridgeStreamError;
use request::{chat_request_to_responses, reject_unsupported_request, responses_request_to_chat};
use response::{chat_response_to_responses, responses_response_to_chat};
use shared::parse_value_object;
use stream::{ChatToResponsesStream, ResponsesToChatStream};

mod request;
mod response;
mod shared;
mod stream;

/// 请求、响应或 stream 无法按受限 Bridge 契约转换时返回的错误。
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BridgeError {
    /// 输入不是符合方向要求的 JSON object。
    #[error("bridge input is not a valid protocol object")]
    InvalidShape,
    /// 输入使用了 Bridge 未声明支持的语义。
    #[error("bridge input uses unsupported semantics")]
    UnsupportedSemantics,
    /// function call/result identity 缺失、重复或无法关联。
    #[error("bridge tool identity is invalid")]
    InvalidToolIdentity,
    /// function arguments 不是闭合 JSON object。
    #[error("bridge function arguments are invalid")]
    InvalidToolArguments,
    /// 上游 stream 生命周期失败。
    #[error("bridge stream lifecycle is invalid")]
    InvalidStream,
}

impl From<BridgeStreamError> for BridgeError {
    fn from(_: BridgeStreamError) -> Self {
        Self::InvalidStream
    }
}

/// 一条已经固定转换方向、Public Model 和上游模型的执行计划。
#[derive(Clone, Debug)]
pub struct BridgePlan {
    downstream_protocol: ApiProtocol,
    upstream_protocol: ApiProtocol,
    public_model: String,
}

impl BridgePlan {
    /// 校验并转换下游请求，返回不可变计划与上游协议请求。
    pub fn prepare(
        downstream_protocol: ApiProtocol,
        upstream_protocol: ApiProtocol,
        public_model: &str,
        upstream_model: &str,
        body: Bytes,
    ) -> Result<(Self, ApiRequest), BridgeError> {
        // 拒绝同协议调用和不受支持的扩展，再执行方向专用转换。
        if downstream_protocol == upstream_protocol {
            return Err(BridgeError::UnsupportedSemantics);
        }
        let source = parse_value_object(&body)?;
        reject_unsupported_request(downstream_protocol, &source)?;
        let converted = match (downstream_protocol, upstream_protocol) {
            (ApiProtocol::ChatCompletions, ApiProtocol::Responses) => {
                chat_request_to_responses(&source, upstream_model)?
            }
            (ApiProtocol::Responses, ApiProtocol::ChatCompletions) => {
                responses_request_to_chat(&source, upstream_model)?
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        };

        // 固化响应转换需要的下游事实，并把紧凑 JSON 交给 Provider adapter。
        let request = ApiRequest::new(
            upstream_protocol,
            Bytes::from(serde_json::to_vec(&converted).map_err(|_| BridgeError::InvalidShape)?),
        );
        Ok((
            Self {
                downstream_protocol,
                upstream_protocol,
                public_model: public_model.to_owned(),
            },
            request,
        ))
    }

    /// 返回计划的下游协议。
    pub fn downstream_protocol(&self) -> ApiProtocol {
        self.downstream_protocol
    }

    /// 返回计划实际调用的上游协议。
    pub fn upstream_protocol(&self) -> ApiProtocol {
        self.upstream_protocol
    }

    /// 将一个完整成功上游 JSON response 转换为下游协议。
    pub fn render_non_stream(&self, body: Bytes) -> Result<Bytes, BridgeError> {
        // 解析上游对象并按固定方向生成下游 response。
        let source = parse_value_object(&body)?;
        let converted = match (self.downstream_protocol, self.upstream_protocol) {
            (ApiProtocol::ChatCompletions, ApiProtocol::Responses) => {
                responses_response_to_chat(&source, &self.public_model)?
            }
            (ApiProtocol::Responses, ApiProtocol::ChatCompletions) => {
                chat_response_to_responses(&source, &self.public_model)?
            }
            _ => return Err(BridgeError::UnsupportedSemantics),
        };
        serde_json::to_vec(&converted)
            .map(Bytes::from)
            .map_err(|_| BridgeError::InvalidShape)
    }

    /// 创建只服务于本次请求的增量 SSE renderer。
    pub fn stream_renderer(&self) -> BridgeStreamRenderer {
        BridgeStreamRenderer::new(self.clone())
    }
}

/// 将上游完整 SSE event 增量渲染成下游协议 event。
pub struct BridgeStreamRenderer {
    plan: BridgePlan,
    state: StreamState,
}

enum StreamState {
    ResponsesToChat(ResponsesToChatStream),
    ChatToResponses(ChatToResponsesStream),
}

impl BridgeStreamRenderer {
    fn new(plan: BridgePlan) -> Self {
        let state = match (plan.downstream_protocol, plan.upstream_protocol) {
            (ApiProtocol::ChatCompletions, ApiProtocol::Responses) => {
                StreamState::ResponsesToChat(ResponsesToChatStream::new())
            }
            (ApiProtocol::Responses, ApiProtocol::ChatCompletions) => {
                StreamState::ChatToResponses(ChatToResponsesStream::new())
            }
            _ => unreachable!("BridgePlan always has opposite protocols"),
        };
        Self { plan, state }
    }

    /// 消费一个已完成 framing 的上游 event，并返回零个或多个下游 SSE event bytes。
    pub fn render(&mut self, event: SseEvent) -> Result<Bytes, BridgeError> {
        match &mut self.state {
            StreamState::ResponsesToChat(state) => state.render(event, &self.plan.public_model),
            StreamState::ChatToResponses(state) => state.render(event, &self.plan.public_model),
        }
    }

    /// 结束上游输入并确认显式 terminal 已经到达。
    pub fn finish(&mut self) -> Result<Bytes, BridgeError> {
        match &mut self.state {
            StreamState::ResponsesToChat(state) => state.finish(),
            StreamState::ChatToResponses(state) => state.finish(),
        }
    }
}
