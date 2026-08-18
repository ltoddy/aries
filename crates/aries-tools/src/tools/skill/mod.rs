mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use aries_extension::skill::definition::SkillDefinition;
use aries_filesystem::path_to_uri;
use aries_filesystem::walk::walk_dir;
use itertools::Itertools;
use rig::tool::{Tool, ToolContext};
use serde_json::Value;

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
    type Args = SkillArgs;
    type Output = SkillOutput;
    type Error = SkillError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The name of the skill from available_skills"
                }
            },
            "required": ["name"]
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let available = self.available_skills_display();

        let skill = self
            .skills
            .iter()
            .find(|s| s.frontmatter.name == args.name)
            .ok_or_else(|| SkillError::not_allowed(&args.name, &available))?;

        let dir =
            skill.location.parent().ok_or_else(|| SkillError::not_found(&args.name, &available))?;

        let entries = walk_dir(dir, true, true)?;
        let files = entries
            .iter()
            .filter(|e| e.is_file())
            .map(|f| format!("<file>{}</file>", f.display()))
            .join("\n");

        let mut lines = vec![
            format!(r#"<skill_content name="{}">"#, skill.frontmatter.name),
            format!("# Skill: {}", skill.frontmatter.name),
            skill.body.clone(),
            format!("Base directory for this skill: {}", path_to_uri(dir).await),
            "Relative paths in this skill (e.g., scripts/, reference/) are relative to this base directory."
                .to_owned(),
        ];

        if let Some(allowed_tools) = &skill.frontmatter.allowed_tools
            && !allowed_tools.is_empty()
        {
            lines.push(format!("Allowed tools for this skill: {}", allowed_tools.join(", ")));
        }

        lines.extend([
            "<skill_files>".to_owned(),
            files,
            "</skill_files>".to_owned(),
            "</skill_content>".to_owned(),
        ]);

        let output = lines.join("\n");

        Ok(SkillOutput {
            title: format!("Loaded skill: {}", args.name),
            output,
            metadata: SkillMetadata { name: args.name, dir: dir.to_path_buf() },
        })
    }
}
