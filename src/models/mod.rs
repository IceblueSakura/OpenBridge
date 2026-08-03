//! Canonical model catalog compiled into the OpenBridge binary.
//!
//! Model facts are independent of providers and endpoints; multiple Upstream Targets can
//! reference the same model ID, while each Upstream API supplies its upstream model ID and
//! more conservative protocol constraints.

mod catalog;
pub(crate) mod deepseek;
mod glm;
pub(crate) mod gpt;
mod hy;
mod kimi;
pub mod longcat;
pub(crate) mod mimo;
mod minimax;
pub(crate) mod nemotron;
mod qwen;

pub(crate) use catalog::compiled_configs;

#[cfg(test)]
mod tests {
    use super::{deepseek, gpt, longcat, mimo, nemotron, qwen};

    #[test]
    fn family_modules_expose_version_scoped_model_definitions() {
        assert_eq!(gpt::v5_6_sol::ID, "openai/gpt-5.6-sol");
        assert_eq!(gpt::v5_6_terra::ID, "openai/gpt-5.6-terra");
        assert_eq!(gpt::v5_6_luna::ID, "openai/gpt-5.6-luna");
        assert_eq!(deepseek::v4_pro::ID, "deepseek/deepseek-v4-pro");
        assert_eq!(deepseek::v4_flash::ID, "deepseek/deepseek-v4-flash");
        assert_eq!(mimo::v2_5_pro::ID, "xiaomi/mimo-v2.5-pro");
        assert_eq!(mimo::v2_5::ID, "xiaomi/mimo-v2.5");
        assert_eq!(qwen::v3_7_max::ID, "qwen/qwen3.7-max");
        assert_eq!(qwen::v3_7_plus::ID, "qwen/qwen3.7-plus");
        assert_eq!(longcat::v2::ID, "meituan/longcat-2.0");
        assert_eq!(nemotron::v3::ULTRA_ID, "nvidia/nemotron-3-ultra-550b-a55b");
        assert_eq!(gpt::configs().len(), 5);
    }
}
