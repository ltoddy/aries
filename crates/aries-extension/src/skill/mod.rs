/// see more: https://agentskills.io/specification
mod definition;
mod loader;
#[cfg(test)]
mod tests;

pub use self::definition::{Frontmatter, SkillDefinition};
pub use self::loader::SkillsLoader;
