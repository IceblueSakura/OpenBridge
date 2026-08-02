//! OpenBridge 编译进二进制的 canonical 模型目录。
//!
//! 模型事实与具体 Provider/endpoint 解耦；多个 Upstream Target 可以引用同一个模型 id，
//! 各 Upstream API 提供上游 model id 与更保守的协议级约束。

mod catalog;
mod deepseek;
mod glm;
pub(crate) mod gpt;
mod hy;
mod kimi;
pub mod longcat;
mod mimo;
mod minimax;
pub(crate) mod nemotron;
mod qwen;

#[cfg(test)]
mod tests {
    use super::{gpt, longcat, nemotron};

    #[test]
    fn family_modules_expose_version_scoped_model_definitions() {
        assert_eq!(gpt::v5_6::SOL_ID, "openai/gpt-5.6-sol");
        assert_eq!(longcat::v2::ID, "meituan/longcat-2.0");
        assert_eq!(nemotron::v3::ULTRA_ID, "nvidia/nemotron-3-ultra-550b-a55b");
        assert_eq!(gpt::configs().len(), 5);
    }
}

pub(crate) use catalog::compiled_configs;
