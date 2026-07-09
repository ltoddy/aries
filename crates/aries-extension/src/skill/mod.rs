/// see more: https://agentskills.io/specification
pub mod definition;
pub mod loader;

pub use self::definition::{Frontmatter, SkillDefinition};
pub use self::loader::SkillsLoader;
