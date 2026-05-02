mod env;
mod instruction;
mod skill;

use aries_context::GlobalContext;

use crate::agent_type::AgentType;
use crate::ext::skill::SkillInfo;

pub async fn render(
    gctx: &GlobalContext,
    agent_type: AgentType,
    model: &str,
    available_skills: &[SkillInfo],
) -> String {
    match agent_type {
        AgentType::Build | AgentType::General | AgentType::Plan => {
            let mut preamble = agent_type.preamble().to_string();

            preamble.push('\n');
            preamble.push_str(&env::render(gctx, model));
            preamble.push('\n');

            if !available_skills.is_empty() {
                preamble.push('\n');
                preamble.push_str(&skill::render(available_skills));
                preamble.push('\n');
            }

            let loader = instruction::AgentsmdFileLoader::new(&gctx.current_dir);
            if let Some(content) = loader.read().await {
                preamble.push('\n');
                preamble.push_str(&format!("Instructions from: {}", loader.file_path().display()));
                preamble.push('\n');
                preamble.push_str(&content);
                preamble.push('\n');
            }

            preamble
        },
        _ => agent_type.preamble().to_string(),
    }
}
