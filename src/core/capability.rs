use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySet {
    pub chat: bool,
    pub responses: bool,
    pub streaming: bool,
    pub function_tools: bool,
    pub structured_output: bool,
    pub previous_response_id: bool,
    pub background: bool,
    pub response_store: bool,
}

impl CapabilitySet {
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        (!self.chat || upper.chat)
            && (!self.responses || upper.responses)
            && (!self.streaming || upper.streaming)
            && (!self.function_tools || upper.function_tools)
            && (!self.structured_output || upper.structured_output)
            && (!self.previous_response_id || upper.previous_response_id)
            && (!self.background || upper.background)
            && (!self.response_store || upper.response_store)
    }
}
