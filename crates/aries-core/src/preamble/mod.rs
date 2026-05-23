mod env;
mod instruction;
mod repo;
mod skill;

use std::path::Path;

use crate::agents::AgentType;
use crate::ext::skill::SkillDefinition;

pub async fn render(
    cwd: impl AsRef<Path>,
    agent_type: AgentType,
    model: &str,
    available_skills: &[SkillDefinition],
) -> String {
    match agent_type {
        AgentType::Build | AgentType::General => {
            let mut preamble = agent_type.bare_preamble().to_string();

            preamble.push('\n');
            preamble.push_str(&env::render(&cwd, model));
            preamble.push('\n');

            if let Some(repo_prompt) = repo::render(&cwd).await {
                preamble.push('\n');
                preamble.push_str(&repo_prompt);
                preamble.push('\n');
            }

            if !available_skills.is_empty() {
                preamble.push('\n');
                preamble.push_str(&skill::render(available_skills));
                preamble.push('\n');
            }

            let loader = instruction::AgentsmdFileLoader::new(&cwd);
            if let Some(content) = loader.read().await {
                preamble.push('\n');
                preamble.push_str(&format!("Instructions from: {}", loader.file_path().display()));
                preamble.push('\n');
                preamble.push_str(&content);
                preamble.push('\n');
            }

            preamble
        },
        _ => agent_type.bare_preamble().to_string(),
    }
}
