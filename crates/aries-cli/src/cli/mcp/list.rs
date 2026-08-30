use std::env::current_dir;

use anyhow::Context;
use aries_extension::{McpServerConfig, McpsLoader};
use aries_init::GlobalContext;
use clap::Parser;
use prettytable::{Cell, Row, Table, row};

#[derive(Clone, Debug, Parser)]
#[command(about = "List configured MCP servers")]
pub struct ListMcpArgs {}

pub async fn execute(_args: ListMcpArgs, gctx: GlobalContext) -> anyhow::Result<()> {
    let cwd = current_dir().with_context(|| "could not determine current directory")?;

    let loader = McpsLoader::new(&cwd, &gctx.home_dir);
    let definitions = loader.load().await;

    if definitions.is_empty() {
        println!("No MCP servers found.");
        return Ok(());
    }

    let mut table = Table::new();
    table.add_row(row!["Name", "Type", "Detail"]);
    for definition in definitions {
        let mut entries: Vec<_> = definition.mcp_servers.into_iter().collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));

        for (name, config) in entries {
            let (type_str, detail) = match config {
                McpServerConfig::Stdio(stdio) => {
                    let cmd = if stdio.args.is_empty() {
                        stdio.command.clone()
                    } else {
                        format!("{} {}", stdio.command, stdio.args.join(" "))
                    };
                    ("stdio", cmd)
                },
                McpServerConfig::Sse(sse) => ("sse", sse.url),
                McpServerConfig::Http(http) => ("http", http.url),
            };
            table.add_row(Row::new(vec![
                Cell::new(&name),
                Cell::new(type_str),
                Cell::new(&detail),
            ]));
        }
    }

    table.printstd();

    Ok(())
}
