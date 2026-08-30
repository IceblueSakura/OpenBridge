//! Portable reasoning request semantics.

/// Requested reasoning effort, including the distinction between omitted and explicit `none`.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReasoningEffort {
    /// The downstream request omitted the effort field.
    #[default]
    Omitted,
    /// Reasoning is explicitly disabled.
    None,
    /// Minimal reasoning.
    Minimal,
    /// Low reasoning.
    Low,
    /// Medium reasoning.
    Medium,
    /// High reasoning.
    High,
    /// Extra-high reasoning.
    XHigh,
    /// Maximum reasoning.
    Max,
}

impl ReasoningEffort {
    /// Parses the stable protocol label, including the explicit omitted sentinel.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "omitted" => Some(Self::Omitted),
            "none" => Some(Self::None),
            "minimal" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" => Some(Self::XHigh),
            "max" => Some(Self::Max),
            _ => None,
        }
    }

    /// Returns the wire label, or `None` when the field must be omitted.
    pub const fn as_wire(self) -> Option<&'static str> {
        match self {
            Self::Omitted => None,
            Self::None => Some("none"),
            Self::Minimal => Some("minimal"),
            Self::Low => Some("low"),
            Self::Medium => Some("medium"),
            Self::High => Some("high"),
            Self::XHigh => Some("xhigh"),
            Self::Max => Some("max"),
        }
    }
}

/// Requested reasoning-summary mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReasoningSummary {
    /// The summary field was omitted.
    #[default]
    Omitted,
    /// The request explicitly disables a summary.
    Disabled,
    /// The Provider chooses the summary representation.
    Auto,
}

impl ReasoningSummary {
    /// Parses the closed summary labels accepted by the static IR.
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "omitted" => Some(Self::Omitted),
            "disabled" | "none" | "false" => Some(Self::Disabled),
            "auto" | "true" => Some(Self::Auto),
            _ => None,
        }
    }

    /// Returns the canonical wire label, or `None` when the field is omitted.
    pub const fn as_wire(self) -> Option<&'static str> {
        match self {
            Self::Omitted => None,
            Self::Disabled => Some("disabled"),
            Self::Auto => Some("auto"),
        }
    }
}

/// Whether the downstream request supplied a reasoning object.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReasoningPresence {
    /// No reasoning object was supplied.
    #[default]
    Absent,
    /// A reasoning object was supplied, even when all child fields were omitted.
    Present,
}

/// Reasoning controls carried by one canonical Generation request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReasoningRequest {
    presence: ReasoningPresence,
    effort: ReasoningEffort,
    summary: ReasoningSummary,
}

impl ReasoningRequest {
    /// Creates a present reasoning request while retaining omitted versus explicit child values.
    pub const fn new(effort: ReasoningEffort, summary: ReasoningSummary) -> Self {
        Self {
            presence: ReasoningPresence::Present,
            effort,
            summary,
        }
    }

    /// Returns whether the reasoning object was absent or present.
    pub const fn presence(self) -> ReasoningPresence {
        self.presence
    }

    /// Returns the requested effort.
    pub const fn effort(self) -> ReasoningEffort {
        self.effort
    }

    /// Returns the requested summary mode.
    pub const fn summary(self) -> ReasoningSummary {
        self.summary
    }
}
