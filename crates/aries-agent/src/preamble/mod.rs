mod instruction;
pub mod memory;
mod repo;
mod skill;

use std::path::Path;

use aries_extension::skill::definition::SkillDefinition;
use aries_mode::Mode;

pub async fn render(
    cwd: impl AsRef<Path>,
    mode: Mode,
    model: impl Into<String>,
    available_skills: &[SkillDefinition],
    memory: Option<&str>,
) -> String {
    let model = model.into();

    match mode {
        Mode::Build | Mode::General => {
            let mut preamble = mode.bare_preamble().to_string();

            preamble.push('\n');
            preamble.push_str(&aries_preamble::env::section(&cwd, model));
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

            if let Some(mem) = memory {
                preamble.push('\n');
                preamble.push_str(mem);
                preamble.push('\n');
            }

            let loader = instruction::AgentsmdFileLoader::new(&cwd);
            if let Some(content) = loader.read().await {
                preamble.push('\n');
                preamble
                    .push_str(&format!("Instructions from: {}\n", loader.file_path().display()));
                preamble.push_str(&content);
                preamble.push('\n');
            }

            preamble
        },
        _ => mode.bare_preamble().to_string(),
    }
}
