//! Images interface DTO and request-time accessors.

use super::*;

/// Unique fixed capability contract for the Images Generations operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImagesInterfaceCapabilities {
    pub(super) max_outputs: u32,
    pub(super) default_outputs: u32,
    pub(super) allowed_sizes: Option<ImagesSizeDomain>,
    pub(super) default_response_format: ImagesResponseFormat,
    pub(super) allowed_response_formats: Option<Vec<ImagesResponseFormat>>,
    pub(super) supported_parameters: Vec<String>,
    pub(super) dashscope_extensions: Option<DashScopeImagesCapabilities>,
}

impl ImagesInterfaceCapabilities {
    /// Returns the conservative DashScope extension profile when every candidate agrees.
    pub(crate) const fn dashscope_extensions(&self) -> Option<DashScopeImagesCapabilities> {
        self.dashscope_extensions
    }

    /// Resolves an omitted or explicit output count against the fixed domain.
    pub(crate) fn resolve_outputs(&self, requested: Option<u32>) -> Option<u32> {
        match requested {
            None => Some(self.default_outputs),
            Some(requested) if (1..=self.max_outputs).contains(&requested) => Some(requested),
            Some(_) => None,
        }
    }

    /// Returns whether one explicit `WxH` pair stays inside the fixed size domain.
    pub(crate) fn supports_size(&self, width: u32, height: u32) -> bool {
        self.allowed_sizes
            .is_some_and(|domain| domain.contains(width, height))
    }

    /// Resolves an omitted or explicit response format against the fixed domain.
    pub(crate) fn resolve_response_format(
        &self,
        requested: Option<ImagesResponseFormat>,
    ) -> Option<ImagesResponseFormat> {
        match requested {
            None => Some(self.default_response_format),
            Some(requested)
                if self
                    .allowed_response_formats
                    .as_ref()
                    .is_some_and(|allowed| allowed.contains(&requested)) =>
            {
                Some(requested)
            }
            Some(_) => None,
        }
    }

    /// Returns whether this interface exposes an optional top-level request parameter.
    pub(crate) fn supports_parameter(&self, parameter: &str) -> bool {
        self.supported_parameters
            .iter()
            .any(|supported| supported == parameter)
    }
}
