use std::env::current_dir;

use anyhow::Context;
use aries_extension::{SkillDefinition, SkillsLoader};
use aries_init::GlobalContext;
use clap::Parser;
use prettytable::{Cell, Row, Table, row};

use crate::text;

#[derive(Clone, Debug, Parser)]
#[command(about = "List available skills")]
pub struct ListSkillArgs {}

pub async fn execute(_args: ListSkillArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let cwd = current_dir().with_context(|| "could not determine current directory")?;

    let loader = SkillsLoader::new(cwd, gctx.home_dir);
    let mut skills = loader.load().await;
    skills.sort_by(|prev, next| prev.frontmatter.name.cmp(&next.frontmatter.name));

    if skills.is_empty() {
        println!("No skills found.");
        return Ok(());
    }

    let mut table = Table::new();
    table.add_row(row!["Name", "Description", "Location"]);
    for SkillDefinition { location, frontmatter, .. } in skills {
        table.add_row(Row::new(vec![
            Cell::new(&frontmatter.name),
            Cell::new(&text::wrap(&frontmatter.description, 50)),
            Cell::new(&location.display().to_string()),
        ]));
    }

    table.printstd();

    Ok(())
}
