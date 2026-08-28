//! Images request facts and single-candidate Native execution-plan types.

use crate::{
    core::{DashScopePromptExtendMode, ImagesOutputFormat, ImagesRequest, ImagesResponseFormat},
    registry::{OperationResponseBudget, UpstreamApiKey},
};

/// Registry-independent facts extracted from one strict Images Generations request.
#[derive(Debug)]
pub struct ImagesRequestRequirements {
    pub(in crate::pipeline) public_model: String,
    pub(in crate::pipeline) prompt_length: u32,
    pub(in crate::pipeline) requested_outputs: Option<u32>,
    pub(in crate::pipeline) requested_size: Option<ImagesRequestedSize>,
    pub(in crate::pipeline) requested_response_format: Option<ImagesResponseFormat>,
    pub(in crate::pipeline) requested_output_format: Option<ImagesOutputFormat>,
    pub(in crate::pipeline) requested_stream: Option<bool>,
    pub(in crate::pipeline) unsupported_standard_fields: Vec<ImagesUnsupportedStandardField>,
    pub(in crate::pipeline) dashscope: DashScopeImagesRequestRequirements,
    pub(in crate::pipeline) user_present: bool,
}

/// Frozen DashScope-only extension facts without retaining negative-prompt content.
#[derive(Debug, Default)]
pub(in crate::pipeline) struct DashScopeImagesRequestRequirements {
    pub(in crate::pipeline) prompt_extend: Option<bool>,
    pub(in crate::pipeline) prompt_extend_mode: Option<DashScopePromptExtendMode>,
    pub(in crate::pipeline) enable_thinking: Option<bool>,
    pub(in crate::pipeline) negative_prompt_present: bool,
    pub(in crate::pipeline) seed: Option<u32>,
    pub(in crate::pipeline) watermark: Option<bool>,
}

impl DashScopeImagesRequestRequirements {
    /// Returns the first present extension field for field-level capability errors.
    pub(in crate::pipeline) const fn first_present_parameter(&self) -> Option<&'static str> {
        if self.prompt_extend.is_some() {
            Some("prompt_extend")
        } else if self.prompt_extend_mode.is_some() {
            Some("prompt_extend_mode")
        } else if self.enable_thinking.is_some() {
            Some("enable_thinking")
        } else if self.negative_prompt_present {
            Some("negative_prompt")
        } else if self.seed.is_some() {
            Some("seed")
        } else if self.watermark.is_some() {
            Some("watermark")
        } else {
            None
        }
    }
}

/// Structurally valid OpenAI Images fields with no qwen execution semantics in this focus.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::pipeline) enum ImagesUnsupportedStandardField {
    Background,
    Moderation,
    OutputCompression,
    PartialImages,
    Quality,
    Style,
}

impl ImagesUnsupportedStandardField {
    /// Returns the exact downstream parameter name used in public errors.
    pub(in crate::pipeline) const fn parameter(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Moderation => "moderation",
            Self::OutputCompression => "output_compression",
            Self::PartialImages => "partial_images",
            Self::Quality => "quality",
            Self::Style => "style",
        }
    }
}

/// One parsed OpenAI size request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImagesRequestedSize {
    /// Let the selected model choose an output size.
    Auto,
    /// Request exact positive pixel dimensions.
    Exact {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
}

/// Single-candidate Native execution plan for an Images Generations request.
#[derive(Debug)]
pub struct ImagesRoutePlan {
    pub(in crate::pipeline) candidate: ImagesRouteCandidate,
    pub(in crate::pipeline) outputs: u32,
    pub(in crate::pipeline) size: Option<ImagesRequestedSize>,
    pub(in crate::pipeline) response_format: ImagesResponseFormat,
    pub(in crate::pipeline) response_budget: OperationResponseBudget,
}

/// Trusted Native Images Route candidate bound to one target and Upstream API.
#[derive(Debug)]
pub struct ImagesRouteCandidate {
    pub(in crate::pipeline) upstream_target_id: String,
    pub(in crate::pipeline) upstream_api_key: UpstreamApiKey,
    pub(in crate::pipeline) request: ImagesRequest,
}

impl ImagesRequestRequirements {
    /// Returns the Public Model selected by the downstream Images request.
    pub fn public_model(&self) -> &str {
        &self.public_model
    }

    /// Returns the non-blank prompt length frozen by strict analysis.
    pub fn prompt_length(&self) -> u32 {
        self.prompt_length
    }

    /// Returns the explicit requested output count when present.
    pub fn requested_outputs(&self) -> Option<u32> {
        self.requested_outputs
    }

    /// Returns the explicit requested `WxH` size when present.
    pub fn requested_size(&self) -> Option<ImagesRequestedSize> {
        self.requested_size
    }

    /// Returns the explicit requested response format when present.
    pub fn requested_response_format(&self) -> Option<ImagesResponseFormat> {
        self.requested_response_format
    }
}

impl ImagesRequestedSize {
    /// Returns the width component in pixels.
    pub fn width(&self) -> Option<u32> {
        match self {
            Self::Auto => None,
            Self::Exact { width, .. } => Some(*width),
        }
    }

    /// Returns the height component in pixels.
    pub fn height(&self) -> Option<u32> {
        match self {
            Self::Auto => None,
            Self::Exact { height, .. } => Some(*height),
        }
    }
}

impl ImagesRoutePlan {
    /// Returns the single trusted Images candidate.
    pub fn candidate(&self) -> &ImagesRouteCandidate {
        &self.candidate
    }

    /// Returns the resolved output count after fixed-interface preflight.
    pub fn outputs(&self) -> u32 {
        self.outputs
    }

    /// Returns the resolved effective size after fixed-interface preflight.
    pub fn size(&self) -> Option<ImagesRequestedSize> {
        self.size
    }

    /// Returns the resolved response format after fixed-interface preflight.
    pub fn response_format(&self) -> ImagesResponseFormat {
        self.response_format
    }

    /// Returns the JSON response limit compiled with the Images interface.
    pub(crate) const fn max_json_response_body_bytes(&self) -> usize {
        self.response_budget.max_json_body_bytes()
    }
}

impl ImagesRouteCandidate {
    /// Returns the trusted Upstream Target ID.
    pub fn upstream_target_id(&self) -> &str {
        &self.upstream_target_id
    }

    /// Returns the complete trusted Upstream API identity.
    pub fn upstream_api_key(&self) -> UpstreamApiKey {
        self.upstream_api_key
    }

    /// Returns the preserved Native Images request.
    pub fn request(&self) -> &ImagesRequest {
        &self.request
    }
}
