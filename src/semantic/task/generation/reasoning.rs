#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReasoningSummary {
    Auto,
    Concise,
    Detailed,
}
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct ReasoningRequest {
    pub effort: Option<ReasoningEffort>,
    pub summary: Option<ReasoningSummary>,
}
