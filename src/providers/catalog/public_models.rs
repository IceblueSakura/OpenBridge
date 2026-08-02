//! 下游 Public Model 与有序 Route 候选。

use crate::registry::PublicModelConfig;

use super::routes::{
    CODE_PRIMARY_OPENAI_CHAT, CODE_PRIMARY_OPENAI_CHAT_VIA_RESPONSES,
    CODE_PRIMARY_OPENAI_RESPONSES, CODE_PRIMARY_OPENAI_RESPONSES_VIA_CHAT, LONGCAT_2_CHAT,
    LONGCAT_2_CHAT_VIA_RESPONSES, LONGCAT_2_RESPONSES, LONGCAT_2_RESPONSES_VIA_CHAT,
};

/// 返回所有编译进二进制的 Public Model。
pub(super) fn compiled_public_models() -> Vec<PublicModelConfig> {
    vec![
        PublicModelConfig {
            name: "code-primary".to_owned(),
            routes: vec![
                CODE_PRIMARY_OPENAI_CHAT.to_owned(),
                CODE_PRIMARY_OPENAI_CHAT_VIA_RESPONSES.to_owned(),
                CODE_PRIMARY_OPENAI_RESPONSES.to_owned(),
                CODE_PRIMARY_OPENAI_RESPONSES_VIA_CHAT.to_owned(),
            ],
        },
        PublicModelConfig {
            name: "LongCat-2.0".to_owned(),
            routes: vec![
                LONGCAT_2_CHAT.to_owned(),
                LONGCAT_2_CHAT_VIA_RESPONSES.to_owned(),
                LONGCAT_2_RESPONSES.to_owned(),
                LONGCAT_2_RESPONSES_VIA_CHAT.to_owned(),
            ],
        },
    ]
}
