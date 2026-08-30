pub mod agentsmd;
pub mod env;
pub mod memory;
pub mod repository;
pub mod skill;

use std::path::Path;

use aries_extension::SkillDefinition;
use aries_init::GlobalContext;

pub fn sections(
    gctx: GlobalContext,
    cwd: impl AsRef<Path>,
    model: impl Into<String>,
    skills: &[SkillDefinition],
) -> Vec<String> {
    let cwd = cwd.as_ref();
    let model = model.into();

    vec![
        skill::section(skills),
        env::section(cwd, model),
        repository::section(cwd),
        memory::section(gctx.memory_root_dir.join(aries_filesystem::path_to_slug(cwd))),
        agentsmd::section(cwd),
    ]
}
