//! Images Generations capability ceilings and closed-domain validation.
//!
//! Images has no generation protocol, stream, reasoning, or Bridge semantics. Its prompt, output
//! count, size domain, and response-format guarantees are validated and narrowed as one
//! independent domain.

/// Response formats preserved on the OpenAI-compatible downstream wire.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImagesResponseFormat {
    /// A short-lived Provider-hosted image URL.
    Url,
    /// Base64-encoded image data in the JSON response.
    B64Json,
}

/// Standard image containers exposed by the OpenAI Images contract.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImagesOutputFormat {
    /// Portable Network Graphics.
    Png,
    /// Joint Photographic Experts Group image.
    Jpeg,
    /// WebP image.
    Webp,
}

/// DashScope prompt-extension strategy accepted through OpenAI SDK `extra_body`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DashScopePromptExtendMode {
    /// Direct prompt optimization.
    Direct,
    /// Agent-guided prompt optimization.
    Agent,
}

/// Typed DashScope-only Images extension ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct DashScopeImagesCapabilities {
    /// Stable provider default when `prompt_extend` is omitted.
    pub default_prompt_extend: bool,
    /// Accepted prompt-extension modes.
    pub prompt_extend_modes: &'static [DashScopePromptExtendMode],
    /// Stable provider default when the mode is omitted.
    pub default_prompt_extend_mode: DashScopePromptExtendMode,
    /// Stable provider default when `enable_thinking` is omitted.
    pub default_enable_thinking: bool,
    /// Whether a non-blank negative prompt is accepted.
    pub negative_prompt: bool,
    /// Largest accepted non-negative seed.
    pub maximum_seed: u32,
    /// Stable provider default when `watermark` is omitted.
    pub default_watermark: bool,
}

impl DashScopeImagesCapabilities {
    /// Validates extension defaults and closed domains.
    fn validate(self) -> Result<(), &'static str> {
        if !is_strictly_sorted(self.prompt_extend_modes)
            || !self
                .prompt_extend_modes
                .contains(&self.default_prompt_extend_mode)
        {
            return Err("DashScope prompt_extend_modes must be ordered and contain the default");
        }
        Ok(())
    }

    /// Returns whether this extension contract stays inside an upper Provider ceiling.
    fn is_subset_of(self, upper: Self) -> bool {
        self.default_prompt_extend == upper.default_prompt_extend
            && self.default_prompt_extend_mode == upper.default_prompt_extend_mode
            && self.default_enable_thinking == upper.default_enable_thinking
            && self.default_watermark == upper.default_watermark
            && (!self.negative_prompt || upper.negative_prompt)
            && self.maximum_seed <= upper.maximum_seed
            && self
                .prompt_extend_modes
                .iter()
                .all(|mode| upper.prompt_extend_modes.contains(mode))
    }
}

/// Width or height bounds and pixel-area bounds accepted by the `size` request field.
///
/// The request format is `WxH`. A valid size must stay within both side bounds, within the area
/// bounds, and within the fixed 1:8–8:1 aspect-ratio contract enforced by the request analyzer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct ImagesSizeDomain {
    /// Smallest accepted positive side length.
    pub minimum_side: u32,
    /// Largest accepted positive side length.
    pub maximum_side: u32,
    /// Smallest accepted pixel area (`W * H`).
    pub minimum_area: u64,
    /// Largest accepted pixel area (`W * H`).
    pub maximum_area: u64,
}

impl ImagesSizeDomain {
    /// Returns whether one `WxH` pair stays inside this domain.
    pub(crate) fn contains(self, width: u32, height: u32) -> bool {
        let area = u64::from(width) * u64::from(height);
        self.minimum_side <= width
            && width <= self.maximum_side
            && self.minimum_side <= height
            && height <= self.maximum_side
            && (self.minimum_area..=self.maximum_area).contains(&area)
    }

    /// Returns the conservative size-domain intersection; `None` admits no valid pair.
    pub(crate) fn intersection(self, other: Self) -> Option<Self> {
        let minimum_side = if self.minimum_side > other.minimum_side {
            self.minimum_side
        } else {
            other.minimum_side
        };
        let maximum_side = if self.maximum_side < other.maximum_side {
            self.maximum_side
        } else {
            other.maximum_side
        };
        let minimum_area = if self.minimum_area > other.minimum_area {
            self.minimum_area
        } else {
            other.minimum_area
        };
        let maximum_area = if self.maximum_area < other.maximum_area {
            self.maximum_area
        } else {
            other.maximum_area
        };
        let domain = Self {
            minimum_side,
            maximum_side,
            minimum_area,
            maximum_area,
        };
        if domain.minimum_side > domain.maximum_side
            || domain.minimum_area > domain.maximum_area
            || domain.minimum_area > u64::from(domain.maximum_side) * u64::from(domain.maximum_side)
            || domain.maximum_area < u64::from(domain.minimum_side) * u64::from(domain.minimum_side)
        {
            return None;
        }
        Some(domain)
    }

    /// Returns whether every size in this domain is also accepted by the upper domain.
    fn is_subset_of(self, upper: Self) -> bool {
        self.minimum_side >= upper.minimum_side
            && self.maximum_side <= upper.maximum_side
            && self.minimum_area >= upper.minimum_area
            && self.maximum_area <= upper.maximum_area
    }
}

/// Complete Upstream API capability profile for Images Generations.
///
/// The profile contains only fixed request and response guarantees. It does not contain Provider,
/// endpoint, credential, or Route identity and is projected into an owned public interface by the
/// registry compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImagesGenerationsCapabilities {
    /// Output count produced when `n` is omitted.
    pub default_outputs: u32,
    /// Maximum output count accepted by one request.
    pub max_outputs: u32,
    /// Size domain clients may request explicitly; `None` forbids the request field.
    pub allowed_sizes: Option<ImagesSizeDomain>,
    /// Response format produced when `response_format` is omitted.
    pub default_response_format: ImagesResponseFormat,
    /// Response formats clients may request explicitly; `None` forbids the request field.
    pub allowed_response_formats: Option<&'static [ImagesResponseFormat]>,
    /// Optional top-level OpenAI request parameters accepted by this API.
    pub supported_parameters: &'static [&'static str],
    /// Optional DashScope-only extension contract accepted through OpenAI SDK `extra_body`.
    pub dashscope_extensions: Option<DashScopeImagesCapabilities>,
}

impl ImagesGenerationsCapabilities {
    /// Validates closed defaults, domains, limits, and parameter ownership.
    pub(crate) fn validate(self) -> Result<(), &'static str> {
        // Validate positive ordered output-count limits.
        if self.default_outputs == 0
            || self.max_outputs == 0
            || self.default_outputs > self.max_outputs
        {
            return Err("images output counts must be positive and ordered");
        }

        // Validate any explicit size domain.
        if let Some(domain) = self.allowed_sizes {
            if domain.minimum_side == 0
                || domain.minimum_side > domain.maximum_side
                || domain.minimum_area == 0
                || domain.minimum_area > domain.maximum_area
            {
                return Err("images size domain must be positive and ordered");
            }
            let smallest = u64::from(domain.minimum_side) * u64::from(domain.minimum_side);
            let largest = u64::from(domain.maximum_side) * u64::from(domain.maximum_side);
            if smallest > domain.maximum_area || largest < domain.minimum_area {
                return Err("images size domain bounds must admit a valid area");
            }
        }

        // Validate response-format sets contain the default and stay in the closed allowlist.
        if let Some(formats) = self.allowed_response_formats
            && (!is_strictly_sorted(formats) || !formats.contains(&self.default_response_format))
        {
            return Err(
                "allowed_response_formats must be non-empty, unique, ordered, and contain the default",
            );
        }

        // Keep the optional parameter set closed and consistent with explicit domains.
        if !is_sorted_unique_or_empty(self.supported_parameters)
            || self.supported_parameters.iter().any(|parameter| {
                !matches!(
                    *parameter,
                    "n" | "output_format" | "size" | "response_format" | "user"
                )
            })
        {
            return Err("supported_parameters must be an ordered subset of the Images allowlist");
        }
        if self.supported_parameters.contains(&"size") != self.allowed_sizes.is_some()
            || self.supported_parameters.contains(&"response_format")
                != self.allowed_response_formats.is_some()
        {
            return Err(
                "supported parameters must match the explicit size and response-format domains",
            );
        }
        if let Some(extensions) = self.dashscope_extensions {
            extensions.validate()?;
        }
        Ok(())
    }

    /// Returns whether this API profile stays within a Provider capability ceiling.
    pub(crate) fn is_subset_of(self, upper: Self) -> bool {
        if upper.default_outputs > self.default_outputs
            || upper.max_outputs < self.max_outputs
            || !format_supported_by(upper, self.default_response_format)
            || self
                .supported_parameters
                .iter()
                .any(|parameter| !upper.supported_parameters.contains(parameter))
        {
            return false;
        }
        if self.allowed_response_formats.is_some_and(|formats| {
            upper.allowed_response_formats.is_none_or(|upper_formats| {
                formats.iter().any(|format| !upper_formats.contains(format))
            })
        }) {
            return false;
        }
        match (self.dashscope_extensions, upper.dashscope_extensions) {
            (Some(extensions), Some(upper_extensions))
                if !extensions.is_subset_of(upper_extensions) =>
            {
                return false;
            }
            (Some(_), None) => return false,
            _ => {}
        }
        match (self.allowed_sizes, upper.allowed_sizes) {
            (Some(domain), Some(upper_domain)) => domain.is_subset_of(upper_domain),
            (Some(_), None) => false,
            _ => true,
        }
    }
}

/// Returns whether one non-empty slice is strictly ordered and therefore duplicate-free.
fn is_strictly_sorted<T: Ord>(values: &[T]) -> bool {
    !values.is_empty() && values.windows(2).all(|pair| pair[0] < pair[1])
}

/// Returns whether a slice is empty or strictly ordered and duplicate-free.
fn is_sorted_unique_or_empty<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

/// Returns whether the Provider ceiling can produce or explicitly accept one response format.
fn format_supported_by(upper: ImagesGenerationsCapabilities, value: ImagesResponseFormat) -> bool {
    upper.default_response_format == value
        || upper
            .allowed_response_formats
            .is_some_and(|formats| formats.contains(&value))
}
