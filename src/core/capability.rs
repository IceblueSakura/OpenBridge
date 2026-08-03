//! provider-independent capability 上界与协议分域值对象。
//!
//! capability 只能在 registry 构建阶段从 provider contract 收窄；请求 routing 复用这里的
//! 子集判断，确保未实现的能力不会通过配置或协议字段越权进入 egress。

/// 上游生成 reasoning 的可观察输出类型。
///
/// `Unknown` 表示没有足够的 wire 证据，不能被当作可读文本；`Opaque` 覆盖 provider-issued
/// 的不可读 continuation，例如 Responses 的 `encrypted_content`。只有 `PlainText` 与
/// `Summary` 能进入跨协议 reasoning channel，且具体可转换方向仍由上游协议决定。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReasoningOutput {
    /// 没有足够的上游 wire 证据判断输出格式。
    #[default]
    Unknown,
    /// 上游明确不返回 reasoning 输出。
    Unsupported,
    /// 上游返回可读的完整 reasoning 文本。
    PlainText,
    /// 上游只返回可读的 reasoning summary。
    Summary,
    /// 上游返回不可读的 opaque/encrypted continuation。
    Opaque,
}

impl ReasoningOutput {
    /// 判断该输出是否包含可读 reasoning 文本或 summary。
    pub const fn is_readable(self) -> bool {
        matches!(self, Self::PlainText | Self::Summary)
    }

    /// 判断当前配置是否没有向 provider contract 声称额外的 reasoning 输出能力。
    pub(crate) const fn is_subset_of(self, upper: Self) -> bool {
        matches!(
            (self, upper),
            (Self::Unknown | Self::Unsupported, _)
                | (Self::PlainText, Self::PlainText)
                | (Self::Summary, Self::Summary)
                | (Self::Opaque, Self::Opaque)
        )
    }
}

/// Responses Create 可引用的 OpenAI-hosted tool 种类。
///
/// 这些枚举只保留标准协议位置；当前 pipeline、adapter 和 Provider 注册均未实现这些工具。
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedToolKind {
    /// Web search tool。
    WebSearch,
    /// File search tool。
    FileSearch,
    /// Code Interpreter tool。
    CodeInterpreter,
    /// Computer Use tool。
    ComputerUse,
    /// Image generation tool。
    ImageGeneration,
    /// Remote MCP tool。
    Mcp,
    /// Hosted shell tool。
    Shell,
    /// Apply patch tool。
    ApplyPatch,
    /// Tool search tool。
    ToolSearch,
    /// Skills tool。
    Skills,
    /// Programmatic Tool Calling tool。
    ProgrammaticToolCalling,
}

/// Responses Create 的 `include` 标准附加输出种类。
///
/// 枚举值使用语义化 Rust 名称，Rustdoc 标明对应 wire path；当前仅作为预留接口。
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseInclude {
    /// `web_search_call.action.sources`。
    WebSearchCallSources,
    /// `code_interpreter_call.outputs`。
    CodeInterpreterCallOutputs,
    /// `computer_call_output.output.image_url`。
    ComputerCallOutputImageUrl,
    /// `file_search_call.results`。
    FileSearchCallResults,
    /// `message.input_image.image_url`。
    InputImageImageUrl,
    /// `message.output_text.logprobs`。
    OutputTextLogprobs,
    /// `reasoning.encrypted_content`。
    ReasoningEncryptedContent,
}

/// Chat Completions 与 Responses 共享的生成能力投影。
///
/// 该值只用于请求分析和协议公共子集判断；静态注册应使用协议专有的
/// [`ChatCompletionsCapabilities`] 或 [`ResponsesCapabilities`]。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GenerationCapabilities {
    /// 该端点是否可用。
    pub(crate) enabled: bool,
    /// 是否支持以 SSE 返回增量结果。
    pub(crate) streaming: bool,
    /// 是否支持 JSON-schema function tool 调用。
    pub(crate) function_calling: bool,
    /// 对请求 wire 字段 `parallel_tool_calls: true` 的支持。
    pub(crate) parallel_tool_calls: bool,
    /// 是否支持图像输入内容 part。
    pub(crate) image_input: bool,
    /// 是否支持结构化输出约束。
    pub(crate) structured_outputs: bool,
    /// 对请求 wire 字段 `store: true` 的支持。
    pub(crate) store: bool,
    /// 上游 reasoning 输出的可观察类型。
    pub(crate) reasoning_output: ReasoningOutput,
}

impl GenerationCapabilities {
    /// 判断当前能力是否未超过给定上界。
    pub(crate) const fn is_subset_of(self, upper: Self) -> bool {
        (!self.enabled || upper.enabled)
            && (!self.streaming || upper.streaming)
            && (!self.function_calling || upper.function_calling)
            && (!self.parallel_tool_calls || upper.parallel_tool_calls)
            && (!self.image_input || upper.image_input)
            && (!self.structured_outputs || upper.structured_outputs)
            && (!self.store || upper.store)
            && self.reasoning_output.is_subset_of(upper.reasoning_output)
    }
}

/// Chat Completions Create endpoint 的能力上界。
///
/// 已实现字段保持当前 routing 语义；audio/file/custom tool、predicted outputs 等新增字段
/// 只保留定义位置，启用时会在 registry 编译阶段触发 `unimplemented!`。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChatCompletionsCapabilities {
    /// Chat Completions endpoint 是否可用。
    pub enabled: bool,
    /// 是否支持 Chat Completions streaming。
    pub streaming: bool,
    /// 是否支持 JSON-schema function tool 调用。
    pub function_calling: bool,
    /// 对请求 wire 字段 `parallel_tool_calls: true` 的支持。
    pub parallel_tool_calls: bool,
    /// 是否支持 `image_url` 输入内容 part。
    pub image_input: bool,
    /// 是否支持 `response_format` 或 strict function 的结构化输出约束。
    pub structured_outputs: bool,
    /// 对请求 wire 字段 `store: true` 的支持。
    pub store: bool,
    /// 上游 reasoning 输出的可观察类型。
    pub reasoning_output: ReasoningOutput,
    /// 是否支持 `type: "custom"` tool。
    pub custom_tool_calling: bool,
    /// 是否支持 `input_audio` 输入内容 part。
    pub audio_input: bool,
    /// 是否支持 `file` 输入内容 part。
    pub file_input: bool,
    /// 是否支持 `modalities` 中的 audio 输出。
    pub audio_output: bool,
    /// 是否支持 `prediction` predicted outputs。
    pub predicted_outputs: bool,
    /// 是否支持 `web_search_options`。
    pub web_search: bool,
    /// 是否支持 prompt cache key/options/breakpoint 语义。
    pub prompt_caching: bool,
    /// 是否支持请求级 moderation 配置。
    pub moderation: bool,
    /// 是否支持 token log probabilities。
    pub logprobs: bool,
    /// 是否支持 `n > 1` 的多个 choice。
    pub multiple_choices: bool,
}

impl ChatCompletionsCapabilities {
    /// 提取 Chat Completions 与 Responses 共享的生成能力。
    pub(crate) const fn generation_capabilities(self) -> GenerationCapabilities {
        GenerationCapabilities {
            enabled: self.enabled,
            streaming: self.streaming,
            function_calling: self.function_calling,
            parallel_tool_calls: self.parallel_tool_calls,
            image_input: self.image_input,
            structured_outputs: self.structured_outputs,
            store: self.store,
            reasoning_output: self.reasoning_output,
        }
    }

    /// 判断当前 Chat Completions 能力是否未超过给定上界。
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        // 阻止预留字段在实现请求处理前进入静态能力契约。
        self.assert_reserved_unimplemented();
        upper.assert_reserved_unimplemented();

        // 比较当前已实现的协议公共能力。
        self.generation_capabilities()
            .is_subset_of(upper.generation_capabilities())
    }

    /// 在预留字段被静态注册时停止编译，避免形成虚假的运行时能力。
    fn assert_reserved_unimplemented(self) {
        if self.custom_tool_calling
            || self.audio_input
            || self.file_input
            || self.audio_output
            || self.predicted_outputs
            || self.web_search
            || self.prompt_caching
            || self.moderation
            || self.logprobs
            || self.multiple_choices
        {
            unimplemented!("reserved Chat Completions capabilities are not implemented");
        }
    }
}

/// Responses Create endpoint 的能力上界。
///
/// resource retrieve/cancel/delete 等其他 endpoint 不属于此结构；新增 Create 字段当前只保留
/// 类型位置，启用时会在 registry 编译阶段触发 `unimplemented!`。
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
    /// 上游 reasoning 输出的可观察类型。
    pub reasoning_output: ReasoningOutput,
    /// 是否支持 `type: "custom"` tool。
    pub custom_tool_calling: bool,
    /// 已声明支持的 OpenAI-hosted tool 种类。
    pub hosted_tools: &'static [HostedToolKind],
    /// 是否支持 file input item/content part。
    pub file_input: bool,
    /// 是否支持 `conversation` 持久状态。
    pub conversation: bool,
    /// 是否支持 `prompt` 模板引用。
    pub prompt_templates: bool,
    /// 是否支持 prompt cache key/options/breakpoint 语义。
    pub prompt_caching: bool,
    /// 是否支持 `context_management`。
    pub context_management: bool,
    /// 已声明支持的 `include` 附加输出种类。
    pub include: &'static [ResponseInclude],
    /// 是否支持请求级 moderation 配置。
    pub moderation: bool,
    /// 是否支持 message output text log probabilities。
    pub logprobs: bool,
}

impl ResponsesCapabilities {
    /// 提取 Responses 与 Chat 共享的端点能力。
    pub(crate) const fn generation_capabilities(self) -> GenerationCapabilities {
        GenerationCapabilities {
            enabled: self.enabled,
            streaming: self.streaming,
            function_calling: self.function_calling,
            parallel_tool_calls: self.parallel_tool_calls,
            image_input: self.image_input,
            structured_outputs: self.structured_outputs,
            store: self.store,
            reasoning_output: self.reasoning_output,
        }
    }

    /// 判断当前 Responses 能力是否未超过给定上界。
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        // 阻止预留字段在实现请求处理前进入静态能力契约。
        self.assert_reserved_unimplemented();
        upper.assert_reserved_unimplemented();

        // 比较已实现的公共能力与 Responses 状态能力。
        self.generation_capabilities()
            .is_subset_of(upper.generation_capabilities())
            && (!self.previous_response_id || upper.previous_response_id)
            && (!self.background || upper.background)
    }

    /// 在预留字段被静态注册时停止编译，避免形成虚假的运行时能力。
    fn assert_reserved_unimplemented(self) {
        if self.custom_tool_calling
            || !self.hosted_tools.is_empty()
            || self.file_input
            || self.conversation
            || self.prompt_templates
            || self.prompt_caching
            || self.context_management
            || !self.include.is_empty()
            || self.moderation
            || self.logprobs
        {
            unimplemented!("reserved Responses capabilities are not implemented");
        }
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
    pub chat_completions: ChatCompletionsCapabilities,
    /// Responses endpoint 的能力上界。
    pub responses: ResponsesCapabilities,
}

impl ApiCapabilities {
    /// 按 Chat/Responses 两个协议分域判断能力是否收窄。
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        self.chat_completions.is_subset_of(upper.chat_completions)
            && self.responses.is_subset_of(upper.responses)
    }
}
