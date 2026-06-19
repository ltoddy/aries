/// see more: https://agentskills.io/specification
pub mod definition;
pub mod error;
pub mod loader;

pub use crate::ext::skill::definition::{Frontmatter, SkillDefinition};
pub use crate::ext::skill::error::ParseSkillError;
pub use crate::ext::skill::loader::SkillsLoader;
