//! Shared semantic leaf values.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum ValueError {
    #[error("{kind} must not be empty")] Empty { kind: &'static str },
    #[error("{kind} exceeds the {max_bytes}-byte limit")] TooLarge { kind: &'static str, max_bytes: usize },
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Text(String);
impl Text {
    pub fn new(value: impl Into<String>, kind: &'static str, max_bytes: usize) -> Result<Self, ValueError> {
        let value=value.into();
        if value.is_empty(){return Err(ValueError::Empty{kind});}
        if value.len()>max_bytes{return Err(ValueError::TooLarge{kind,max_bytes});}
        Ok(Self(value))
    }
    pub fn as_str(&self)->&str{&self.0}
}
