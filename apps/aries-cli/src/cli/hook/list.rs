use aries_extension::HooksLoader;
use aries_init::GlobalContext;
use clap::Parser;
use itertools::Itertools;
use prettytable::{Cell, Row, Table, row};

use crate::text;

#[derive(Clone, Debug, Parser)]
#[command(about = "List available hooks")]
pub struct ListHooksArgs {}

pub async fn execute(_args: ListHooksArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let cwd = gctx.current_dir();

    let loader = HooksLoader::new(&cwd, gctx.home_dir());
    let mut hooks = loader.load().await;
    hooks.sort_by(|prev, next| prev.location.cmp(&next.location));

    if hooks.is_empty() {
        println!("No hooks found.");
        return Ok(());
    }

    let mut table = Table::new();
    table.add_row(row!["Location", "Description", "Events"]);
    for hook in hooks {
        let description = hook.description.unwrap_or_default();
        let events = hook.hooks.0.keys().map(|event| format!("{event:?}")).sorted().join(", ");
        table.add_row(Row::new(vec![
            Cell::new(&hook.location.display().to_string()),
            Cell::new(&text::wrap(&description, 30)),
            Cell::new(&text::wrap(&events, 30)),
        ]));
    }

    table.printstd();

    Ok(())
}
