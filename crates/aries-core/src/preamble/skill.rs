use aries_extension::skill::definition::SkillDefinition;
use itertools::Itertools;

pub fn render(skills: &[SkillDefinition]) -> String {
    if skills.is_empty() {
        return "<available_skills>\n(none)\n</available_skills>".to_string();
    }

    let skills = skills.iter().map(|s| s.frontmatter.render(&s.location)).join("\n");
    format!("<available_skills>\n{skills}\n</available_skills>",)
}
