//! 尚未绑定真实模型事实的 OpenAI-compatible 占位模型。

use crate::registry::{ModelContextLength, RealModelDefinition, ReasoningSupport};

pub(crate) const MODEL_ID: &str = "openai/configured-model";

pub(crate) fn definition() -> RealModelDefinition {
    RealModelDefinition {
        id: MODEL_ID.to_owned(),
        name: "Configured OpenAI-compatible model".to_owned(),
        description: Some(
            "Replace this placeholder with metadata verified for the real upstream model."
                .to_owned(),
        ),
        context_length: ModelContextLength::default(),
        supported_parameters: Vec::new(),
        reasoning: ReasoningSupport::Unknown,
        reasoning_levels: Vec::new(),
    }
}
