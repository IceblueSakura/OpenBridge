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

/// Bounds exact integer reachability work for untrusted startup documents.
const MAX_SIZE_REACHABILITY_CHECKS: u64 = 1_000_000;

impl ImagesSizeDomain {
    /// Returns whether one `WxH` pair stays inside this domain.
    pub(crate) fn contains(self, width: u32, height: u32) -> bool {
        let area = u64::from(width) * u64::from(height);
        let shorter = u64::from(width.min(height));
        let longer = u64::from(width.max(height));
        self.minimum_side <= width
            && width <= self.maximum_side
            && self.minimum_side <= height
            && height <= self.maximum_side
            && (self.minimum_area..=self.maximum_area).contains(&area)
            && longer <= shorter * 8
    }

    /// Returns a conservative intersection only when bounded validation proves an integer pair.
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
        if !domain.admits_valid_size() {
            return None;
        }
        Some(domain)
    }

    /// Proves within bounded startup work that one integer size satisfies all domain limits.
    fn admits_valid_size(self) -> bool {
        if self.minimum_side == 0
            || self.minimum_side > self.maximum_side
            || self.minimum_area == 0
            || self.minimum_area > self.maximum_area
        {
            return false;
        }

        // Order dimensions so width is the shorter side, then derive the only useful width range.
        let minimum_side = u64::from(self.minimum_side);
        let maximum_side = u64::from(self.maximum_side);
        let minimum_width = minimum_side
            .max(div_ceil(self.minimum_area, maximum_side))
            .max(ceil_sqrt(div_ceil(self.minimum_area, 8)));
        let maximum_width = maximum_side.min(self.maximum_area.isqrt());
        if minimum_width > maximum_width {
            return false;
        }

        // Find an exact integer witness without allowing pathological startup ranges to monopolize CPU.
        let checked_maximum = maximum_width
            .min(minimum_width.saturating_add(MAX_SIZE_REACHABILITY_CHECKS.saturating_sub(1)));
        (minimum_width..=checked_maximum).any(|width| {
            let minimum_height = width
                .max(minimum_side)
                .max(div_ceil(self.minimum_area, width));
            let maximum_height = maximum_side
                .min(width.saturating_mul(8))
                .min(self.maximum_area / width);
            minimum_height <= maximum_height
        })
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
            if !domain.admits_valid_size() {
                return Err("images size domain must admit a valid integer size");
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
        if self.default_outputs != upper.default_outputs
            || self.default_response_format != upper.default_response_format
            || upper.max_outputs < self.max_outputs
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

/// Returns the smallest integer quotient not below the exact ratio.
fn div_ceil(value: u64, divisor: u64) -> u64 {
    value / divisor + u64::from(!value.is_multiple_of(divisor))
}

/// Returns the smallest integer whose square is at least the value.
fn ceil_sqrt(value: u64) -> u64 {
    let floor = value.isqrt();
    if floor * floor == value {
        floor
    } else {
        floor + 1
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

#[cfg(test)]
mod tests {
    use super::{
        DashScopeImagesCapabilities, DashScopePromptExtendMode, ImagesGenerationsCapabilities,
        ImagesResponseFormat, ImagesSizeDomain,
    };

    const URL_FORMATS: &[ImagesResponseFormat] = &[ImagesResponseFormat::Url];
    const B64_FORMATS: &[ImagesResponseFormat] = &[ImagesResponseFormat::B64Json];
    const ALL_FORMATS: &[ImagesResponseFormat] =
        &[ImagesResponseFormat::Url, ImagesResponseFormat::B64Json];
    const STANDARD_PARAMETERS: &[&str] = &["n", "output_format", "response_format", "size", "user"];
    const DIRECT_MODE: &[DashScopePromptExtendMode] = &[DashScopePromptExtendMode::Direct];
    const ALL_MODES: &[DashScopePromptExtendMode] = &[
        DashScopePromptExtendMode::Direct,
        DashScopePromptExtendMode::Agent,
    ];

    fn size_domain(minimum_side: u32, maximum_side: u32) -> ImagesSizeDomain {
        ImagesSizeDomain {
            minimum_side,
            maximum_side,
            minimum_area: u64::from(minimum_side) * u64::from(minimum_side),
            maximum_area: u64::from(maximum_side) * u64::from(maximum_side),
        }
    }

    fn extensions(
        modes: &'static [DashScopePromptExtendMode],
        negative_prompt: bool,
        maximum_seed: u32,
    ) -> DashScopeImagesCapabilities {
        DashScopeImagesCapabilities {
            default_prompt_extend: true,
            prompt_extend_modes: modes,
            default_prompt_extend_mode: DashScopePromptExtendMode::Direct,
            default_enable_thinking: true,
            negative_prompt,
            maximum_seed,
            default_watermark: false,
        }
    }

    fn profile(
        max_outputs: u32,
        domain: ImagesSizeDomain,
        extension: DashScopeImagesCapabilities,
    ) -> ImagesGenerationsCapabilities {
        ImagesGenerationsCapabilities {
            default_outputs: 1,
            max_outputs,
            allowed_sizes: Some(domain),
            default_response_format: ImagesResponseFormat::Url,
            allowed_response_formats: Some(URL_FORMATS),
            supported_parameters: STANDARD_PARAMETERS,
            dashscope_extensions: Some(extension),
        }
    }

    #[test]
    fn images_size_intersection_obeys_idempotence_commutativity_associativity_and_subset() {
        let wide = size_domain(512, 2_048);
        let medium = size_domain(768, 1_536);
        let narrow = size_domain(1_024, 1_280);

        assert_eq!(wide.intersection(wide), Some(wide));
        assert_eq!(wide.intersection(medium), medium.intersection(wide));
        assert_eq!(
            wide.intersection(medium)
                .and_then(|value| value.intersection(narrow)),
            medium
                .intersection(narrow)
                .and_then(|value| wide.intersection(value))
        );
        let intersection = wide.intersection(medium).unwrap();
        assert!(intersection.is_subset_of(wide));
        assert!(intersection.is_subset_of(medium));
        assert_eq!(wide.intersection(size_domain(3_072, 4_096)), None);

        let dense = ImagesSizeDomain {
            minimum_side: 1,
            maximum_side: 10,
            minimum_area: 90,
            maximum_area: 100,
        };
        let exact_nonsquare = ImagesSizeDomain {
            minimum_side: 1,
            maximum_side: 19,
            minimum_area: 95,
            maximum_area: 95,
        };
        assert_eq!(dense.intersection(exact_nonsquare), None);

        let aspect_bounded = ImagesSizeDomain {
            minimum_side: 1,
            maximum_side: 10,
            minimum_area: 1,
            maximum_area: 100,
        };
        assert!(aspect_bounded.contains(1, 8));
        assert!(!aspect_bounded.contains(1, 9));
    }

    #[test]
    fn images_profiles_validate_and_narrow_without_exceeding_provider_ceiling() {
        let ceiling = profile(
            6,
            size_domain(512, 2_048),
            extensions(ALL_MODES, true, u32::MAX),
        );
        let narrowed = profile(
            2,
            size_domain(1_024, 1_536),
            extensions(DIRECT_MODE, false, 42),
        );
        assert!(ceiling.validate().is_ok());
        assert!(narrowed.validate().is_ok());
        assert!(narrowed.is_subset_of(ceiling));
        assert!(!ceiling.is_subset_of(narrowed));

        let mut changed_output_default = narrowed;
        changed_output_default.default_outputs = 2;
        assert!(changed_output_default.validate().is_ok());
        assert!(!changed_output_default.is_subset_of(ceiling));

        let mut flexible_format_ceiling = ceiling;
        flexible_format_ceiling.allowed_response_formats = Some(ALL_FORMATS);
        let mut changed_format_default = narrowed;
        changed_format_default.default_response_format = ImagesResponseFormat::B64Json;
        changed_format_default.allowed_response_formats = Some(B64_FORMATS);
        assert!(changed_format_default.validate().is_ok());
        assert!(!changed_format_default.is_subset_of(flexible_format_ceiling));
    }

    #[test]
    fn images_profile_validation_rejects_unreachable_defaults_domains_and_sets() {
        let base = profile(
            6,
            size_domain(512, 2_048),
            extensions(ALL_MODES, true, u32::MAX),
        );

        let mut invalid_count = base;
        invalid_count.default_outputs = 7;
        assert!(invalid_count.validate().is_err());

        let mut invalid_domain = base;
        invalid_domain.allowed_sizes = Some(ImagesSizeDomain {
            minimum_side: 2_048,
            maximum_side: 512,
            minimum_area: 1,
            maximum_area: 2,
        });
        assert!(invalid_domain.validate().is_err());

        let mut unreachable_integer_domain = base;
        unreachable_integer_domain.allowed_sizes = Some(ImagesSizeDomain {
            minimum_side: 1,
            maximum_side: 10,
            minimum_area: 95,
            maximum_area: 95,
        });
        assert!(unreachable_integer_domain.validate().is_err());

        let mut reachable_nonsquare_domain = base;
        reachable_nonsquare_domain.allowed_sizes = Some(ImagesSizeDomain {
            minimum_side: 1,
            maximum_side: 19,
            minimum_area: 95,
            maximum_area: 95,
        });
        assert!(reachable_nonsquare_domain.validate().is_ok());

        let mut missing_default_format = base;
        missing_default_format.default_response_format = ImagesResponseFormat::B64Json;
        assert!(missing_default_format.validate().is_err());

        let mut mismatched_parameter_domain = base;
        mismatched_parameter_domain.allowed_sizes = None;
        assert!(mismatched_parameter_domain.validate().is_err());

        let mut unordered_parameters = base;
        unordered_parameters.supported_parameters = &["size", "n"];
        assert!(unordered_parameters.validate().is_err());

        let mut invalid_extensions = base;
        invalid_extensions.dashscope_extensions = Some(extensions(&[], false, 42));
        assert!(invalid_extensions.validate().is_err());
    }
}
