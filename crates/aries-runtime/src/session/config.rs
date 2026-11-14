use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Copy, Clone)]
pub enum SessionConfig {
    Mode,
    Model,
    ThoughtLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid session config ID `{0}`: expected `mode`, `model`, or `thought level`")]
pub struct ParseSessionConfigError(String);

impl ParseSessionConfigError {
    pub fn new(config_id: impl Into<String>) -> Self {
        Self(config_id.into())
    }
}

impl Display for SessionConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionConfig::Mode => write!(f, "mode"),
            SessionConfig::Model => write!(f, "model"),
            SessionConfig::ThoughtLevel => write!(f, "thought level"),
        }
    }
}

impl FromStr for SessionConfig {
    type Err = ParseSessionConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "mode" => Ok(Self::Mode),
            "model" => Ok(Self::Model),
            "thought level" => Ok(Self::ThoughtLevel),
            _ => Err(ParseSessionConfigError::new(s)),
        }
    }
}
