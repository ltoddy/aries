use aries_filesystem::path_to_uri;
use aries_filesystem::walk::walk_dir;

use crate::skill::SkillDefinition;

#[derive(Debug)]
pub struct SkillExecutor<'a> {
    skills: &'a [SkillDefinition],
}

impl<'a> SkillExecutor<'a> {
    pub fn new(skills: &'a [SkillDefinition]) -> Self {
        Self { skills }
    }

    pub async fn execute(&self, input: impl AsRef<str>) -> Option<String> {
        let input = input.as_ref().trim();
        let (name, args) = input.split_once(' ').unwrap_or((input, ""));
        let skill = self.skills.iter().find(|s| s.frontmatter.name == name)?;
        let dir = skill.location.parent()?;

        let skill_dir = dir.display().to_string();
        let body = shellexpand::env_with_context_no_errors(&skill.body, |name| match name {
            "ARGUMENTS" => Some(args),
            "SKILL_DIR" => Some(skill_dir.as_str()),
            _ => None,
        })
        .into_owned();

        let files = walk_dir(dir, true, true).unwrap_or_default();
        let files = files
            .into_iter()
            .filter(|f| f.is_file())
            .map(|f| format!("<file>{}</file>", f.display()))
            .collect::<Vec<_>>();

        let mut lines = vec![
            format!(r#"<skill_content name="{}">"#, skill.frontmatter.name),
            format!("# Skill: {}", skill.frontmatter.name),
            body,
            format!("Base directory for this skill: {}", path_to_uri(dir).await),
            "Relative paths in this skill (e.g., scripts/, reference/) are relative to this base directory."
                .to_owned(),
        ];

        if !skill.frontmatter.allowed_tools.is_empty() {
            lines.push(format!(
                "Allowed tools for this skill: {}",
                skill.frontmatter.allowed_tools.as_slice().join(", ")
            ));
        }

        lines.extend([
            "<skill_files>".to_owned(),
            files.join("\n"),
            "</skill_files>".to_owned(),
            "</skill_content>".to_owned(),
        ]);

        Some(lines.join("\n"))
    }
}
