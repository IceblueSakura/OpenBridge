//! provider-independent capability 上界与协议分域值对象。
//!
//! capability 只能在 registry 构建阶段从 provider contract 收窄；请求 routing 复用这里的
//! 子集判断，确保未实现的能力不会通过配置或协议字段越权进入 egress。

/// 一个 OpenAI-compatible 生成端点的能力上界。
///
/// 这些名称是 OpenBridge 的语义能力名，不是把请求 wire 字段直接复制到配置中。实际
/// 字段仍由协议决定，例如 image input 在 Chat 中为 `image_url`、在 Responses 中为
/// `input_image`，而 `function_calling` 的请求载体为 `tools`。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EndpointCapabilities {
    /// 该端点是否可用。
    pub enabled: bool,
    /// 是否支持以 SSE 返回增量结果。
    pub streaming: bool,
    /// 是否支持 JSON-schema function tool 调用。
    pub function_calling: bool,
    /// 对请求 wire 字段 `parallel_tool_calls: true` 的支持。
    pub parallel_tool_calls: bool,
    /// 是否支持图像输入内容 part。
    pub image_input: bool,
    /// 是否支持结构化输出约束。
    pub structured_outputs: bool,
    /// 对请求 wire 字段 `store: true` 的支持。
    pub store: bool,
}

impl EndpointCapabilities {
    /// 判断当前能力是否未超过给定上界。
    pub(crate) const fn is_subset_of(self, upper: Self) -> bool {
        (!self.enabled || upper.enabled)
            && (!self.streaming || upper.streaming)
            && (!self.function_calling || upper.function_calling)
            && (!self.parallel_tool_calls || upper.parallel_tool_calls)
            && (!self.image_input || upper.image_input)
            && (!self.structured_outputs || upper.structured_outputs)
            && (!self.store || upper.store)
    }
}

/// Responses API 专有能力，以及与其共享的端点能力。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResponsesCapabilities {
    /// Responses endpoint 是否可用。
    pub enabled: bool,
    /// 是否支持 Responses streaming。
    pub streaming: bool,
    /// 是否支持 function tool 调用。
    pub function_calling: bool,
    /// 是否支持并行 tool calls。
    pub parallel_tool_calls: bool,
    /// 是否支持图像输入。
    pub image_input: bool,
    /// 是否支持结构化输出。
    pub structured_outputs: bool,
    /// 是否支持持久化 response。
    pub store: bool,
    /// 是否支持以 `previous_response_id` 继续对话状态。
    pub previous_response_id: bool,
    /// 是否支持后台响应。
    pub background: bool,
}

impl ResponsesCapabilities {
    /// 提取 Responses 与 Chat 共享的端点能力。
    pub(crate) const fn protocol_capabilities(self) -> EndpointCapabilities {
        EndpointCapabilities {
            enabled: self.enabled,
            streaming: self.streaming,
            function_calling: self.function_calling,
            parallel_tool_calls: self.parallel_tool_calls,
            image_input: self.image_input,
            structured_outputs: self.structured_outputs,
            store: self.store,
        }
    }

    /// 判断当前 Responses 能力是否未超过给定上界。
    pub(crate) const fn is_subset_of(self, upper: Self) -> bool {
        self.protocol_capabilities()
            .is_subset_of(upper.protocol_capabilities())
            && (!self.previous_response_id || upper.previous_response_id)
            && (!self.background || upper.background)
    }
}

/// Provider contract 的协议分域能力上界。
///
/// Upstream API 只能把 provider contract 已支持的能力关闭，不能把未实现的能力打开；请求
/// routing 使用同一集合在网络调用前拒绝不受支持的 feature。Chat Completions 与
/// Responses 分开建模，以免把一个端点的观察错误外推到另一个端点。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ApiCapabilities {
    /// Chat Completions endpoint 的能力上界。
    pub chat_completions: EndpointCapabilities,
    /// Responses endpoint 的能力上界。
    pub responses: ResponsesCapabilities,
}

impl ApiCapabilities {
    /// 按 Chat/Responses 两个协议分域判断能力是否收窄。
    pub(crate) const fn is_subset_of(self, upper: Self) -> bool {
        self.chat_completions.is_subset_of(upper.chat_completions)
            && self.responses.is_subset_of(upper.responses)
    }
}
