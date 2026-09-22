use crate::semantic::value::Text;
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum OutputConstraint {
    #[default]
    Text,
    JsonObject,
    JsonSchema {
        name: Text,
        schema: serde_json::Value,
        strict: bool,
    },
}
