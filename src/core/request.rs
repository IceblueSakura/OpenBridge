use bytes::Bytes;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Protocol {
    /// OpenAI Chat Completions 协议。
    ChatCompletions,
    /// OpenAI Responses 协议。
    Responses,
}

/// 已完成 HTTP 层基础检查、可交给 provider adapter 的请求。
#[derive(Clone, Debug)]
pub struct ValidatedRequest {
    protocol: Protocol,
    body: Bytes,
}

impl ValidatedRequest {
    /// 创建一个带协议标识的请求视图。
    pub fn new(protocol: Protocol, body: Bytes) -> Self {
        Self { protocol, body }
    }

    /// 返回请求所属的原生协议。
    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    /// 返回未修改的请求 JSON bytes。
    pub fn body(&self) -> &Bytes {
        &self.body
    }
}
