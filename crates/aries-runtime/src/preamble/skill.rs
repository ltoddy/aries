use aries_extension::skill::SkillDefinition;
use itertools::Itertools;

pub fn section(skills: impl AsRef<[SkillDefinition]>) -> String {
    let skills = skills.as_ref();
    if skills.is_empty() {
        return "<available_skills>(none)</available_skills>".to_string();
    }

    let skills = skills.iter().map(|s| s.frontmatter.render(&s.location)).join("\n");
    format!("<available_skills>\n{skills}\n</available_skills>",)
}
