/// 一个 OpenAI-compatible 生成端点的能力上界。
///
/// 这些名称是 OpenBridge 的语义能力名，不是把请求 wire 字段直接复制到配置中。实际
/// 字段仍由协议决定，例如 image input 在 Chat 中为 `image_url`、在 Responses 中为
/// `input_image`，而 `function_calling` 的请求载体为 `tools`。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProtocolCapabilities {
    /// 该端点是否可用。
    pub enabled: bool,
    pub streaming: bool,
    pub function_calling: bool,
    /// 对请求 wire 字段 `parallel_tool_calls: true` 的支持。
    pub parallel_tool_calls: bool,
    pub image_input: bool,
    pub structured_outputs: bool,
    /// 对请求 wire 字段 `store: true` 的支持。
    pub store: bool,
}

impl ProtocolCapabilities {
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
    pub enabled: bool,
    pub streaming: bool,
    pub function_calling: bool,
    pub parallel_tool_calls: bool,
    pub image_input: bool,
    pub structured_outputs: bool,
    pub store: bool,
    pub previous_response_id: bool,
    pub background: bool,
}

impl ResponsesCapabilities {
    pub(crate) const fn protocol_capabilities(self) -> ProtocolCapabilities {
        ProtocolCapabilities {
            enabled: self.enabled,
            streaming: self.streaming,
            function_calling: self.function_calling,
            parallel_tool_calls: self.parallel_tool_calls,
            image_input: self.image_input,
            structured_outputs: self.structured_outputs,
            store: self.store,
        }
    }

    const fn is_subset_of(self, upper: Self) -> bool {
        self.protocol_capabilities()
            .is_subset_of(upper.protocol_capabilities())
            && (!self.previous_response_id || upper.previous_response_id)
            && (!self.background || upper.background)
    }
}

/// 部署的协议分域能力上界。
///
/// route 配置只能把 provider descriptor 已支持的能力关闭，不能把未实现的能力打开；请求
/// routing 使用同一集合在网络调用前拒绝不受支持的 feature。Chat Completions 与
/// Responses 分开建模，以免把一个端点的观察错误外推到另一个端点。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CapabilitySet {
    pub chat_completions: ProtocolCapabilities,
    pub responses: ResponsesCapabilities,
}

impl CapabilitySet {
    pub(crate) const fn is_subset_of(self, upper: Self) -> bool {
        self.chat_completions.is_subset_of(upper.chat_completions)
            && self.responses.is_subset_of(upper.responses)
    }
}
