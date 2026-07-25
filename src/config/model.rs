//! 已编译配置中与单个上游模型绑定的稳定元数据。
//!
//! 这里不定义 TOML 文档形状；它只表达路由和 probe 可消费的运行时模型信息。后续新增
//! `Model` 配置对象时，可在文档层映射到本类型，而不必改变 pipeline 的读取边界。

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ModelLimits {
    context_window_tokens: Option<u32>,
    max_output_tokens: Option<u32>,
}

impl ModelLimits {
    pub(crate) const fn new(
        context_window_tokens: Option<u32>,
        max_output_tokens: Option<u32>,
    ) -> Self {
        Self {
            context_window_tokens,
            max_output_tokens,
        }
    }

    pub const fn context_window_tokens(self) -> Option<u32> {
        self.context_window_tokens
    }

    pub const fn max_output_tokens(self) -> Option<u32> {
        self.max_output_tokens
    }
}
