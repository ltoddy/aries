use std::env::current_dir;

use anyhow::Context;
use aries_extension::agent::AgentsLoader;
use aries_init::GlobalContext;
use clap::Parser;
use prettytable::{Cell, Row, Table, row};

use crate::text;

#[derive(Clone, Debug, Parser)]
#[command(about = "List available agents")]
pub struct ListAgentArgs {}

pub async fn execute(_args: ListAgentArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let cwd = current_dir().with_context(|| "could not determine current directory")?;

    let loader = AgentsLoader::new(&cwd, &gctx.home_dir);
    let mut agents = loader.load().await;
    agents.sort_by(|prev, next| prev.frontmatter.name.cmp(&next.frontmatter.name));

    if agents.is_empty() {
        println!("No agents found.");
        return Ok(());
    }

    let mut table = Table::new();
    table.add_row(row!["Name", "Description", "Model", "Location"]);
    for agent in agents {
        let model = agent.frontmatter.model.as_deref().unwrap_or("default");
        table.add_row(Row::new(vec![
            Cell::new(&agent.frontmatter.name),
            Cell::new(&text::wrap(&agent.frontmatter.description, 50)),
            Cell::new(model),
            Cell::new(&agent.location().display().to_string()),
        ]));
    }

    table.printstd();

    Ok(())
}
