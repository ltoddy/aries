use aries_context::GlobalContext;
use itertools::Itertools;

use crate::AgentType;
use crate::ext::skill::SkillInfo;
use crate::instructions::AgentsmdFileLoader;

pub async fn render(
    gctx: &GlobalContext,
    agent_type: AgentType,
    available_skills: &[SkillInfo],
) -> String {
    match agent_type {
        AgentType::Build | AgentType::General | AgentType::Plan => {
            let mut preamble = agent_type.preamble().to_string();

            let loader = AgentsmdFileLoader::new(&gctx.current_dir);
            if let Some(content) = loader.read().await {
                preamble
                    .push_str(&format!("Instructions from: {}\n", loader.file_path().display()));
                preamble.push_str(&format!("{content}\n"));
            }

            if !available_skills.is_empty() {
                preamble.push_str("\n\n");
                preamble.push_str(&render_available_skills(available_skills));
            }
            preamble
        },
        _ => agent_type.preamble().to_string(),
    }
}

fn render_available_skills(skills: &[SkillInfo]) -> String {
    if skills.is_empty() {
        return "<available_skills>\n(none)\n</available_skills>".to_string();
    }

    let skills = skills.iter().map(|s| s.frontmatter.render(&s.location)).join("\n");
    format!("<available_skills>\n{skills}\n</available_skills>",)
}
