//! 已编译配置中与逻辑模型绑定的稳定元数据。
//!
//! 模型元信息与 deployment 分开：模型说明“这是什么模型及其已知上限”，deployment
//! 说明“通过哪个 provider、endpoint 和原生 model id 调用它”。这允许同一逻辑模型
//! 复用到多个受信上游，同时不把 provider 专属认证或路由状态泄漏到模型目录。

/// 模型声明的 reasoning 支持状态。
///
/// `Unknown` 不表示支持；它用于目录信息尚未由模型所有者确认的情形。请求显式要求
/// reasoning 时，路由将 `Unknown` 视为 fail-closed。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReasoningSupport {
    #[default]
    Unknown,
    Supported,
    Unsupported,
}

/// 模型声明的输入和输出 token 上限。
///
/// `input` 与 `output` 均可未知。`output` 是单次生成上限，可在不计算 prompt token 的
/// 前提下用于 egress 前筛选；`input` 仅作为 metadata 保留，直到接入 model-specific
/// tokenizer 后再做精确 token 计数。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ModelContextLength {
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
}

impl ModelContextLength {
    pub(crate) const fn new(input_tokens: Option<u32>, output_tokens: Option<u32>) -> Self {
        Self {
            input_tokens,
            output_tokens,
        }
    }

    pub const fn input_tokens(self) -> Option<u32> {
        self.input_tokens
    }

    pub const fn output_tokens(self) -> Option<u32> {
        self.output_tokens
    }
}

/// 由 route document 编译后的模型目录项。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelMetadata {
    id: String,
    name: String,
    description: Option<String>,
    context_length: ModelContextLength,
    supported_parameters: Vec<String>,
    reasoning: ReasoningSupport,
}

impl ModelMetadata {
    pub(crate) fn new(
        id: String,
        name: String,
        description: Option<String>,
        context_length: ModelContextLength,
        supported_parameters: Vec<String>,
        reasoning: ReasoningSupport,
    ) -> Self {
        Self {
            id,
            name,
            description,
            context_length,
            supported_parameters,
            reasoning,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub const fn context_length(&self) -> ModelContextLength {
        self.context_length
    }

    pub fn supported_parameters(&self) -> &[String] {
        &self.supported_parameters
    }

    pub const fn reasoning(&self) -> ReasoningSupport {
        self.reasoning
    }
}
