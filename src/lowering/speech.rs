//! Explicit single-target admission; unsupported speech controls are never dropped.
use crate::semantic::{
    task::speech_synthesis::{SpeechError, SpeechRequest, valid_voice},
    value::AudioEncoding,
};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeechCapabilities {
    pub voices: Vec<String>,
    pub formats: Vec<AudioEncoding>,
    pub instructions: bool,
    pub speed: bool,
}
impl SpeechCapabilities {
    pub fn validate(&self) -> Result<(), SpeechError> {
        if !(1..=64).contains(&self.voices.len())
            || self.voices.iter().any(|v| !valid_voice(v))
            || self.voices.iter().collect::<BTreeSet<_>>().len() != self.voices.len()
            || !(1..=6).contains(&self.formats.len())
            || self.formats.iter().collect::<BTreeSet<_>>().len() != self.formats.len()
        {
            return Err(SpeechError);
        }
        Ok(())
    }
}
pub fn check_request(
    request: &SpeechRequest,
    target: &SpeechCapabilities,
) -> Result<(), SpeechError> {
    request.validate()?;
    target.validate()?;
    if !target.voices.iter().any(|v| v == request.voice())
        || !target.formats.contains(&request.requested_format())
        || request.instructions.is_some() && !target.instructions
        || request.speed.is_some() && !target.speed
    {
        return Err(SpeechError);
    }
    Ok(())
}
