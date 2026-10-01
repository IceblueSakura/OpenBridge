//! In-process proofs of explicitly selected history dependencies, never payload stores.
use super::*;
use sha2::{Digest, Sha256};
use std::fmt::Write;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HistoryDependency {
    MessageGroup(ItemId),
    PrefixThrough(ItemId),
}
/// Not a serialized identity, issuer signature or permission to replay. The
/// contract chooses the scope; capture never infers it from item adjacency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestDependencyProof {
    scope: HistoryDependency,
    settings: bool,
    digest: [u8; 32],
}
impl RequestDependencyProof {
    pub fn capture(
        request: &GenerationRequest,
        scope: HistoryDependency,
        settings: bool,
    ) -> Result<Self, GenerationError> {
        Ok(Self {
            scope,
            settings,
            digest: dependency(request, scope, settings)?,
        })
    }
    pub fn check(&self, request: &GenerationRequest) -> Result<(), GenerationError> {
        if self.digest != dependency(request, self.scope, self.settings)? {
            return Err(GenerationError::InvalidDependency);
        }
        Ok(())
    }
}
// Debug is an in-process structural encoding, not a persistent hash format.
// Stream it into the digest to avoid allocating a second body. Schema Map Debug
// preserves insertion order unlike Value equality. Redacted values bind separately.
struct HashWriter {
    hash: Sha256,
    remaining: usize,
}
impl std::fmt::Write for HashWriter {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.remaining = self
            .remaining
            .checked_sub(value.len())
            .ok_or(std::fmt::Error)?;
        self.hash.update(value.as_bytes());
        Ok(())
    }
}
fn dependency(
    request: &GenerationRequest,
    scope: HistoryDependency,
    settings: bool,
) -> Result<[u8; 32], GenerationError> {
    request.validate()?;
    let items = match scope {
        HistoryDependency::MessageGroup(owner) => request
            .message_groups()
            .find(|g| g.owner() == owner)
            .ok_or(GenerationError::InvalidDependency)?
            .items(),
        HistoryDependency::PrefixThrough(owner) => {
            let end = request
                .items()
                .iter()
                .position(|(id, _)| *id == owner)
                .ok_or(GenerationError::InvalidDependency)?;
            &request.items()[..=end]
        }
    };
    let mut writer = HashWriter {
        hash: Sha256::new(),
        remaining: MAX_TOTAL_BYTES * 8,
    };
    write!(writer, "{scope:?}:{settings}:{items:?}").map_err(|_| GenerationError::Limit)?;
    if settings {
        write!(writer, "{:?}", request.settings()).map_err(|_| GenerationError::Limit)?;
    }
    for (id, item) in items {
        writer.hash.update(id.get().to_le_bytes());
        match item {
            Item::Reasoning(r) => {
                if let Some(value) = &r.encrypted {
                    writer.hash.update(value.fingerprint());
                }
            }
            Item::Message(m) => {
                for part in &m.parts {
                    if let ContentPart::Resource(resource) = &part.content {
                        resource_dependency(&mut writer.hash, resource);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(writer.hash.finalize().into())
}
fn resource_dependency(hash: &mut Sha256, resource: &Resource) {
    let values: &[&str] = match &resource.location {
        ResourceLocation::Url(value) | ResourceLocation::OpaqueReference(value) => {
            &[value.as_str()]
        }
        ResourceLocation::Inline {
            media_type,
            data_base64,
        } => &[media_type.as_str(), data_base64.as_str()],
    };
    for value in values {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
}
