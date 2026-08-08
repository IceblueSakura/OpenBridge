//! Verifies the registered canonical model catalog and provider-independent model facts.

use super::*;

#[test]
fn compiled_model_catalog_preserves_registered_model_facts() {
    let definition = compiled_config();
    let mut expected = [
        "meituan/longcat-2.0",
        "openai/gpt-5.6-sol",
        "openai/gpt-5.6-terra",
        "openai/gpt-5.6-luna",
        "openai/gpt-5.5",
        "chatgpt/gpt-5.6-sol",
        "chatgpt/gpt-5.6-terra",
        "chatgpt/gpt-5.6-luna",
        "chatgpt/gpt-5.5",
        "chatgpt/gpt-5.3-codex-spark",
        "openai/text-embedding-3-small",
        "deepseek/deepseek-v4-pro",
        "deepseek/deepseek-v4-flash",
        "xiaomi/mimo-v2.5-pro",
        "xiaomi/mimo-v2.5",
        "xiaomi/mimo-v2.5-asr",
        "xiaomi/mimo-v2.5-tts",
        "xiaomi/mimo-v2.5-tts-voicedesign",
        "xiaomi/mimo-v2.5-tts-voiceclone",
        "qwen/qwen3.7-max",
        "qwen/qwen3.7-plus",
        "qwen/qwen3.7-text-embedding",
        "qwen/qwen3.8-max",
        "qwen/qwen-audio-3.0-asr-flash",
        "qwen/qwen-image-3.0",
        "qwen/qwen-image-3.0-pro",
        "qwen/qwen3.5-livetranslate-flash-realtime",
        "qwen/qwen3.6-27b",
        "z-ai/glm-5.2",
        "moonshotai/kimi-k3",
        "minimax/minimax-m3",
    ];

    // Compare registered membership without freezing private module or aggregation order.
    let mut actual = definition
        .models
        .iter()
        .map(|model| model.id.as_str())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    expected.sort_unstable();
    assert_eq!(actual, expected);

    // Every model has an official catalog description except the ChatGPT Codex Spark profile, which OpenRouter does not list precisely.
    assert!(
        definition.models.iter().all(|model| {
            model.id == "chatgpt/gpt-5.3-codex-spark" || model.description.is_some()
        })
    );

    let longcat = definition
        .models
        .iter()
        .find(|model| model.id == "meituan/longcat-2.0")
        .expect("OpenRouter LongCat id is canonical");
    assert_eq!(longcat.context_length.context_tokens(), Some(1_048_756));
    assert_eq!(longcat.context_length.input_tokens(), Some(1_048_756));
    assert_eq!(longcat.context_length.output_tokens(), Some(262_144));
    assert_eq!(longcat.mode, Some(ModelMode::Chat));
    assert_eq!(longcat.input_modalities, Some(vec![InputModality::Text]));
    assert_eq!(longcat.output_modalities, Some(vec![OutputModality::Text]));
    assert_eq!(longcat.tokenizer.as_deref(), Some("Other"));
    assert_eq!(longcat.knowledge_cutoff, None);
    assert_eq!(
        longcat.reasoning_levels,
        [ReasoningLevel::High, ReasoningLevel::None]
    );

    let sol = definition
        .models
        .iter()
        .find(|model| model.id == "openai/gpt-5.6-sol")
        .unwrap();
    assert_eq!(sol.context_length.context_tokens(), Some(1_050_000));
    assert_eq!(sol.context_length.input_tokens(), Some(1_050_000));
    assert_eq!(sol.context_length.output_tokens(), Some(128_000));
    assert_eq!(sol.mode, Some(ModelMode::Chat));
    assert_eq!(
        sol.input_modalities,
        Some(vec![
            InputModality::Text,
            InputModality::Image,
            InputModality::File
        ])
    );
    assert_eq!(sol.output_modalities, Some(vec![OutputModality::Text]));
    assert_eq!(sol.tokenizer.as_deref(), Some("GPT"));
    assert_eq!(sol.knowledge_cutoff.as_deref(), Some("2026-02-16"));
    assert_eq!(
        sol.reasoning_levels,
        [
            ReasoningLevel::Max,
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ]
    );

    let gpt_5_5 = definition
        .models
        .iter()
        .find(|model| model.id == "openai/gpt-5.5")
        .unwrap();
    assert_eq!(
        gpt_5_5.reasoning_levels,
        [
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::None,
        ]
    );
    assert_eq!(gpt_5_5.context_length.context_tokens(), Some(1_050_000));
    assert_eq!(gpt_5_5.context_length.input_tokens(), Some(1_050_000));
    assert_eq!(gpt_5_5.context_length.output_tokens(), Some(128_000));

    // ChatGPT subscription profiles retain the copied model facts with a 272K context window.
    for model_id in [
        "chatgpt/gpt-5.6-sol",
        "chatgpt/gpt-5.6-terra",
        "chatgpt/gpt-5.6-luna",
        "chatgpt/gpt-5.5",
    ] {
        let model = definition
            .models
            .iter()
            .find(|model| model.id == model_id)
            .expect("ChatGPT subscription GPT profile should be in the catalog");
        assert_eq!(model.context_length.context_tokens(), Some(272_000));
        assert_eq!(model.context_length.input_tokens(), Some(272_000));
        assert_eq!(model.context_length.output_tokens(), Some(128_000));
    }

    let codex_spark = definition
        .models
        .iter()
        .find(|model| model.id == "chatgpt/gpt-5.3-codex-spark")
        .unwrap();
    assert_eq!(codex_spark.context_length.context_tokens(), Some(128_000));
    assert_eq!(codex_spark.context_length.input_tokens(), None);
    assert_eq!(codex_spark.context_length.output_tokens(), Some(128_000));
    assert_eq!(
        codex_spark.reasoning_levels,
        [
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
        ]
    );

    // Representative models retain context, output limits, and standard reasoning levels.
    let deepseek = definition
        .models
        .iter()
        .find(|model| model.id == "deepseek/deepseek-v4-pro")
        .unwrap();
    assert_eq!(deepseek.context_length.context_tokens(), Some(1_048_576));
    assert_eq!(deepseek.context_length.input_tokens(), Some(1_048_576));
    assert_eq!(deepseek.context_length.output_tokens(), Some(384_000));
    assert_eq!(deepseek.mode, Some(ModelMode::Chat));
    assert_eq!(deepseek.input_modalities, Some(vec![InputModality::Text]));
    assert_eq!(deepseek.output_modalities, Some(vec![OutputModality::Text]));
    assert_eq!(deepseek.tokenizer.as_deref(), Some("DeepSeek"));
    assert_eq!(
        deepseek.reasoning_levels,
        [ReasoningLevel::Max, ReasoningLevel::High]
    );

    let deepseek_flash = definition
        .models
        .iter()
        .find(|model| model.id == "deepseek/deepseek-v4-flash")
        .unwrap();
    assert_eq!(deepseek_flash.context_length.output_tokens(), Some(393_216));

    // Qwen3.7 records the full seven-level Responses model contract independently of its Chat-only Bailian binding.
    for model_id in ["qwen/qwen3.7-max", "qwen/qwen3.7-plus"] {
        let model = definition
            .models
            .iter()
            .find(|model| model.id == model_id)
            .expect("Qwen3.7 model should be in the catalog");
        assert_eq!(
            model.reasoning_levels,
            [
                ReasoningLevel::Max,
                ReasoningLevel::XHigh,
                ReasoningLevel::High,
                ReasoningLevel::Medium,
                ReasoningLevel::Low,
                ReasoningLevel::Minimal,
                ReasoningLevel::None,
            ]
        );
    }

    // MiMo records every accepted Responses effort even though enabled values currently share behavior.
    for model_id in ["xiaomi/mimo-v2.5", "xiaomi/mimo-v2.5-pro"] {
        let model = definition
            .models
            .iter()
            .find(|model| model.id == model_id)
            .expect("MiMo text model should be in the catalog");
        assert_eq!(
            model.reasoning_levels,
            [
                ReasoningLevel::High,
                ReasoningLevel::Medium,
                ReasoningLevel::Low,
                ReasoningLevel::None,
            ]
        );
    }

    let qwen3_8_max = definition
        .models
        .iter()
        .find(|model| model.id == "qwen/qwen3.8-max")
        .expect("Qwen3.8 Max should be in the catalog");
    assert_eq!(qwen3_8_max.context_length.context_tokens(), Some(1_000_000));
    assert_eq!(qwen3_8_max.context_length.input_tokens(), Some(1_000_000));
    assert_eq!(qwen3_8_max.context_length.output_tokens(), Some(131_072));
    assert_eq!(
        qwen3_8_max.input_modalities,
        Some(vec![
            InputModality::Text,
            InputModality::Image,
            InputModality::Video,
        ])
    );
    assert_eq!(
        qwen3_8_max.reasoning_levels,
        [
            ReasoningLevel::XHigh,
            ReasoningLevel::High,
            ReasoningLevel::Medium,
            ReasoningLevel::Low,
            ReasoningLevel::Minimal,
        ]
    );

    let qwen_image = definition
        .models
        .iter()
        .find(|model| model.id == "qwen/qwen-image-3.0-pro")
        .expect("Qwen Image 3.0 Pro should be in the catalog");
    assert_eq!(qwen_image.context_length, Default::default());
    assert_eq!(
        qwen_image.input_modalities,
        Some(vec![InputModality::Text, InputModality::Image])
    );
    assert_eq!(
        qwen_image.output_modalities,
        Some(vec![OutputModality::Image])
    );
    assert_eq!(qwen_image.reasoning, ReasoningSupport::Unknown);

    let live_translate = definition
        .models
        .iter()
        .find(|model| model.id == "qwen/qwen3.5-livetranslate-flash-realtime")
        .expect("Qwen3.5 LiveTranslate Flash Realtime should be in the catalog");
    assert_eq!(live_translate.context_length.context_tokens(), Some(53_248));
    assert_eq!(live_translate.context_length.input_tokens(), Some(49_152));
    assert_eq!(live_translate.context_length.output_tokens(), Some(4_096));
    assert_eq!(
        live_translate.input_modalities,
        Some(vec![InputModality::Audio, InputModality::Image])
    );
    assert_eq!(
        live_translate.output_modalities,
        Some(vec![OutputModality::Text, OutputModality::Audio])
    );

    let qwen3_6 = definition
        .models
        .iter()
        .find(|model| model.id == "qwen/qwen3.6-27b")
        .expect("Qwen3.6 27B should be in the catalog");
    assert_eq!(qwen3_6.context_length.context_tokens(), Some(262_144));
    assert_eq!(qwen3_6.context_length.input_tokens(), Some(260_096));
    assert_eq!(qwen3_6.context_length.output_tokens(), Some(65_536));
    assert_eq!(
        qwen3_6.input_modalities,
        Some(vec![
            InputModality::Text,
            InputModality::Image,
            InputModality::Video,
        ])
    );
    assert_eq!(qwen3_6.output_modalities, Some(vec![OutputModality::Text]));
    assert_eq!(qwen3_6.reasoning, ReasoningSupport::Supported);
}
