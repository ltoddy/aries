use std::env::current_dir;

use anyhow::Context;
use aries_extension::CommandsLoader;
use aries_init::GlobalContext;
use clap::Parser;
use prettytable::{Cell, Row, Table, row};

use crate::text;

#[derive(Clone, Debug, Parser)]
#[command(about = "List available commands")]
pub struct ListCommandArgs {}

pub async fn execute(_args: ListCommandArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let cwd = current_dir().with_context(|| "could not determine current directory")?;

    let loader = CommandsLoader::new(&cwd, &gctx.home_dir);
    let mut commands = loader.load().await;
    commands.sort_by(|prev, next| prev.frontmatter.name.cmp(&next.frontmatter.name));

    if commands.is_empty() {
        println!("No commands found.");
        return Ok(());
    }

    let mut table = Table::new();
    table.add_row(row!["Name", "Description", "Location"]);
    for command in commands {
        table.add_row(Row::new(vec![
            Cell::new(&command.frontmatter.name),
            Cell::new(&text::wrap(&command.frontmatter.description, 50)),
            Cell::new(&command.location().display().to_string()),
        ]));
    }

    table.printstd();

    Ok(())
}
