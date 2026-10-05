//! Independent, static single-image task. No conversation, runtime target or file service.
use crate::semantic::value::{ImageFormat, Presence};
use base64::engine::general_purpose::STANDARD;

pub const MAX_PROMPT_CHARS: usize = 32_000;
pub const MAX_PROMPT_BYTES: usize = MAX_PROMPT_CHARS * 4;
pub const MAX_IMAGE_BYTES: usize = 2 << 20;
pub const MAX_IMAGE_BASE64_BYTES: usize = MAX_IMAGE_BYTES.div_ceil(3) * 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid or over-budget image generation value")]
pub struct ImageError;

#[derive(Clone, Eq, PartialEq)]
pub struct ImageGenerationRequest {
    prompt: String,
    pub count: Presence<u8>,
    pub format: Presence<ImageFormat>,
}
impl std::fmt::Debug for ImageGenerationRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageGenerationRequest")
            .field("prompt", &"[redacted]")
            .field("count", &self.count)
            .field("format", &self.format)
            .finish()
    }
}
impl ImageGenerationRequest {
    pub fn new(prompt: impl Into<String>) -> Result<Self, ImageError> {
        let value = Self {
            prompt: prompt.into(),
            count: Presence::Absent,
            format: Presence::Absent,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn prompt(&self) -> &str {
        &self.prompt
    }
    pub fn set_prompt(&mut self, prompt: impl Into<String>) -> Result<(), ImageError> {
        let next = Self::new(prompt)?;
        self.prompt = next.prompt;
        Ok(())
    }
    pub fn validate(&self) -> Result<(), ImageError> {
        if self.prompt.is_empty()
            || self.prompt.len() > MAX_PROMPT_BYTES
            || self.prompt.chars().count() > MAX_PROMPT_CHARS
            || self.count.value().is_some_and(|n| *n != 1)
            || self.format.value().is_some_and(|f| {
                !matches!(f, ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Webp)
            })
        {
            return Err(ImageError);
        }
        Ok(())
    }
}

/// Encoded bytes are inert. This checks Base64 and budgets, not pixels or MIME truth.
#[derive(Clone, Eq, PartialEq)]
pub struct ImageData(String);
impl std::fmt::Debug for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ImageData([redacted])")
    }
}
impl ImageData {
    pub fn new(data: &str) -> Result<Self, ImageError> {
        if data.is_empty() || data.len() > MAX_IMAGE_BASE64_BYTES {
            return Err(ImageError);
        }
        let mut reader = base64::read::DecoderReader::new(data.as_bytes(), &STANDARD);
        let bytes = std::io::copy(&mut reader, &mut std::io::sink()).map_err(|_| ImageError)?;
        if bytes == 0 || bytes > MAX_IMAGE_BYTES as u64 {
            return Err(ImageError);
        }
        Ok(Self(data.into()))
    }
    pub fn as_base64(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}
impl ImageSize {
    pub fn validate(self) -> Result<(), ImageError> {
        if self.width == 0 || self.height == 0 || self.width > 65_536 || self.height > 65_536 {
            Err(ImageError)
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumString, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum ImageBackground {
    Transparent,
    Opaque,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumString, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum ImageQuality {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedImage {
    pub data: ImageData,
    /// Reported facts only. A request default does not prove an artifact's format.
    pub format: Presence<ImageFormat>,
    pub size: Presence<ImageSize>,
    pub background: Presence<ImageBackground>,
    pub quality: Presence<ImageQuality>,
}
mod accounting;
pub use accounting::{
    ImageBilling, ImageCostBreakdown, ImageTokenBreakdown, ImageUsage, UsdAmount,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageGenerationResponse {
    pub created: u64,
    pub image: GeneratedImage,
    pub usage: Presence<ImageUsage>,
}
impl ImageGenerationResponse {
    /// Missing reports stay unknown; a reported contradiction cannot satisfy an explicit intent.
    pub fn validate_for(&self, request: &ImageGenerationRequest) -> Result<(), ImageError> {
        request.validate()?;
        self.validate()?;
        if let (Some(expected), Some(reported)) =
            (request.format.value(), self.image.format.value())
            && expected != reported
        {
            return Err(ImageError);
        }
        Ok(())
    }
    pub fn new(created: u64, data: ImageData) -> Self {
        Self {
            created,
            image: GeneratedImage {
                data,
                format: Presence::Absent,
                size: Presence::Absent,
                background: Presence::Absent,
                quality: Presence::Absent,
            },
            usage: Presence::Absent,
        }
    }
    pub fn validate(&self) -> Result<(), ImageError> {
        if self
            .image
            .format
            .value()
            .is_some_and(|f| !matches!(f, ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Webp))
        {
            return Err(ImageError);
        }
        if let Some(size) = self.image.size.value() {
            size.validate()?;
        }
        if let Some(usage) = self.usage.value() {
            usage.validate()?;
        }
        Ok(())
    }
}
