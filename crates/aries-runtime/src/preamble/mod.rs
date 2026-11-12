pub mod agentsmd;
pub mod env;
pub mod memory;
pub mod skill;

use std::path::Path;

use aries_extension::skill::SkillDefinition;
use aries_init::GlobalContext;

pub async fn sections(
    gctx: &GlobalContext,
    cwd: impl AsRef<Path>,
    model: impl AsRef<str>,
    skills: impl AsRef<[SkillDefinition]>,
) -> Vec<String> {
    let cwd = cwd.as_ref();
    let model = model.as_ref();

    vec![
        skill::section(skills),
        env::section(cwd, model),
        memory::section(gctx.memory_root_dir().join(aries_filesystem::path_to_slug(cwd))),
        agentsmd::section(cwd),
    ]
}
