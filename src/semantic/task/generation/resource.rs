//! Media values are inert: validation never fetches, decodes pixels or uploads.
use super::{GenerationError, MAX_TEXT_BYTES};
use crate::semantic::value::Text;
use base64::engine::general_purpose::STANDARD;

pub const MAX_IMAGE_URL_BYTES: usize = 8192;
pub const MAX_IMAGE_DECODED_BYTES: usize = 768 * 1024;
#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumString, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum ImageDetail {
    Auto,
    Low,
    High,
    Original,
}
impl ImageDetail {
    pub fn label(self) -> &'static str {
        self.into()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceKind {
    Image,
    Audio,
    File,
}
#[derive(Clone, Eq, PartialEq)]
pub enum ResourceLocation {
    Url(Text),
    Inline { media_type: Text, data_base64: Text },
    OpaqueReference(Text),
}
// URLs, query strings and inline bodies can contain private data.
impl std::fmt::Debug for ResourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Url(_) => "Url([redacted])",
            Self::Inline { .. } => "Inline([redacted])",
            Self::OpaqueReference(_) => "OpaqueReference([redacted])",
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    pub kind: ResourceKind,
    pub location: ResourceLocation,
    /// Omitted differs from explicit auto; null has no admitted meaning.
    pub image_detail: Option<ImageDetail>,
}
impl Resource {
    /// Account encoded and decoded sizes separately, without allocating decoded bytes.
    /// This proves representation, not format validity, dimensions or model acceptance.
    pub fn validate(&self) -> Result<usize, GenerationError> {
        if self.image_detail.is_some() && self.kind != ResourceKind::Image {
            return Err(GenerationError::InvalidResource);
        }
        let bytes = match &self.location {
            ResourceLocation::Url(text) => {
                let raw = text.as_str();
                if raw.len() > MAX_IMAGE_URL_BYTES {
                    return Err(GenerationError::Limit);
                }
                let url = url::Url::parse(raw).map_err(|_| GenerationError::InvalidResource)?;
                if !raw.starts_with("https://") && !raw.starts_with("http://")
                    || url.host_str().is_none()
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || raw.contains('\\')
                    || raw.split_once("://").is_some_and(|(_, rest)| {
                        rest.split(['/', '?', '#'])
                            .next()
                            .is_some_and(|authority| authority.contains('@'))
                    })
                    || raw
                        .bytes()
                        .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
                {
                    return Err(GenerationError::InvalidResource);
                }
                raw.len()
            }
            ResourceLocation::Inline {
                media_type,
                data_base64,
            } => {
                let data = data_base64.as_str();
                if data.len() > MAX_TEXT_BYTES {
                    return Err(GenerationError::Limit);
                }
                if self.kind == ResourceKind::Image
                    && !matches!(
                        media_type.as_str(),
                        "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/bmp"
                    )
                {
                    return Err(GenerationError::InvalidResource);
                }
                if data.is_empty() {
                    return Err(GenerationError::InvalidResource);
                }
                // Write decoded bytes to a counter, not a pixel buffer or a second payload.
                let mut reader = base64::read::DecoderReader::new(data.as_bytes(), &STANDARD);
                let decoded = std::io::copy(&mut reader, &mut std::io::sink())
                    .map_err(|_| GenerationError::InvalidResource)?;
                if decoded > MAX_IMAGE_DECODED_BYTES as u64 {
                    return Err(GenerationError::Limit);
                }
                // Include the data URL header in the aggregate semantic budget.
                media_type.as_str().len() + data.len() + "data:;base64,".len()
            }
            ResourceLocation::OpaqueReference(id) => {
                if id.as_str().is_empty() || id.as_str().len() > 256 {
                    return Err(GenerationError::InvalidResource);
                }
                id.as_str().len()
            }
        };
        Ok(bytes + self.image_detail.map_or(0, |d| d.label().len()))
    }
}
