mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use aries_extension::skill::definition::SkillDefinition;
use aries_filesystem::path_to_uri;
use aries_filesystem::walk::walk_dir;
use itertools::Itertools;
use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;

pub use self::args::SkillArgs;
pub use self::error::SkillError;
pub use self::output::{SkillMetadata, SkillOutput};

pub const NAME: &str = "Skill";

pub struct SkillTool {
    skills: Vec<SkillDefinition>,
}

impl SkillTool {
    pub fn new(skills: Vec<SkillDefinition>) -> Self {
        Self { skills }
    }

    fn available_skills_display(&self) -> String {
        if self.skills.is_empty() {
            "none".to_owned()
        } else {
            self.skills.iter().map(|s| &s.frontmatter.name).join(", ")
        }
    }
}

impl Tool for SkillTool {
    const NAME: &'static str = NAME;
    type Error = SkillError;
    type Args = SkillArgs;
    type Output = SkillOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_owned(),
            description: include_str!("description.md").to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name of the skill from available_skills"
                    }
                },
                "required": ["name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let name = args.name;
        let available = self.available_skills_display();

        let skill = self
            .skills
            .iter()
            .find(|s| s.frontmatter.name == name)
            .ok_or_else(|| SkillError::not_allowed(name.clone(), available.clone()))?;

        let dir = skill
            .location
            .parent()
            .ok_or_else(|| SkillError::not_found(name.clone(), available.clone()))?;

        let entries = walk_dir(dir, true, true)?;
        let files = entries
            .iter()
            .filter(|e| e.is_file())
            .map(|f| format!("<file>{}</file>", f.display()))
            .join("\n");

        let output = [
            format!(r#"<skill_content name="{}">"#, skill.frontmatter.name),
            format!("# Skill: {}", skill.frontmatter.name),
            skill.body.clone(),
            format!("Base directory for this skill: {}", path_to_uri(dir)),
            "Relative paths in this skill (e.g., scripts/, reference/) are relative to this base directory."
                .to_owned(),
            "Note: file list is sampled.".to_owned(),
            "<skill_files>".to_owned(),
            files,
            "</skill_files>".to_owned(),
            "</skill_content>".to_owned(),
        ]
        .join("\n");

        Ok(SkillOutput {
            title: format!("Loaded skill: {name}"),
            output,
            metadata: SkillMetadata { name, dir: dir.to_path_buf() },
        })
    }
}
