//! Canonical model catalog compiled into the OpenBridge binary.
//!
//! Model facts are explicit compile-time profiles rather than runtime Provider discovery; multiple
//! Upstream Targets can reference the same model ID, while each Upstream API supplies its upstream
//! model ID and more conservative protocol constraints. Most roots follow developer namespaces,
//! while `chatgpt` is reserved for subscription profiles whose verified limits differ from the
//! general API profile.

mod catalog;
pub(crate) mod chatgpt;
pub(crate) mod deepseek;
pub mod meituan;
mod minimax;
mod moonshotai;
pub(crate) mod openai;
mod qwen;
pub(crate) mod xiaomi;
mod z_ai;

pub(crate) use catalog::compiled_configs;

#[cfg(test)]
mod tests {
    use super::{chatgpt, deepseek, meituan, minimax, moonshotai, openai, qwen, xiaomi, z_ai};

    #[test]
    fn model_modules_expose_model_definitions() {
        assert_eq!(openai::gpt_5_6_sol::ID, "openai/gpt-5.6-sol");
        assert_eq!(openai::gpt_5_6_terra::ID, "openai/gpt-5.6-terra");
        assert_eq!(openai::gpt_5_6_luna::ID, "openai/gpt-5.6-luna");
        assert_eq!(
            openai::text_embedding_3_small::ID,
            "openai/text-embedding-3-small"
        );
        assert_eq!(deepseek::deepseek_v4_pro::ID, "deepseek/deepseek-v4-pro");
        assert_eq!(
            deepseek::deepseek_v4_flash::ID,
            "deepseek/deepseek-v4-flash"
        );
        assert_eq!(xiaomi::mimo_v2_5_pro::ID, "xiaomi/mimo-v2.5-pro");
        assert_eq!(xiaomi::mimo_v2_5::ID, "xiaomi/mimo-v2.5");
        assert_eq!(qwen::qwen3_7_max::ID, "qwen/qwen3.7-max");
        assert_eq!(qwen::qwen3_7_plus::ID, "qwen/qwen3.7-plus");
        assert_eq!(meituan::longcat_2_0::ID, "meituan/longcat-2.0");
        assert_eq!(z_ai::configs()[0].id, "z-ai/glm-5.2");
        assert_eq!(moonshotai::configs()[0].id, "moonshotai/kimi-k3");
        assert_eq!(minimax::configs()[0].id, "minimax/minimax-m3");
        assert_eq!(openai::generation_configs().len(), 4);
        assert_eq!(openai::embedding_configs().len(), 1);
    }

    #[test]
    fn chatgpt_module_owns_codex_spark_profile() {
        assert_eq!(
            chatgpt::gpt_5_3_codex_spark::ID,
            "chatgpt/gpt-5.3-codex-spark"
        );
        assert_eq!(chatgpt::configs().len(), 5);
        assert_eq!(
            super::compiled_configs()
                .iter()
                .filter(|model| model.id == chatgpt::gpt_5_3_codex_spark::ID)
                .count(),
            1
        );
    }

    #[test]
    fn chatgpt_module_contains_gpt_5_5_and_gpt_5_6_profiles() {
        let models = chatgpt::configs();
        let expected_ids = [
            "chatgpt/gpt-5.6-sol",
            "chatgpt/gpt-5.6-terra",
            "chatgpt/gpt-5.6-luna",
            "chatgpt/gpt-5.5",
        ];

        assert_eq!(models.len(), 5);
        for expected_id in expected_ids {
            let model = models
                .iter()
                .find(|model| model.id == expected_id)
                .expect("ChatGPT subscription GPT profile should be compiled");
            assert_eq!(model.context_length.context_tokens(), Some(272_000));
            assert_eq!(model.context_length.input_tokens(), Some(272_000));
            assert_eq!(model.context_length.output_tokens(), Some(128_000));
        }
    }
}
