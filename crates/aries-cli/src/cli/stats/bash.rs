use std::collections::BTreeMap;

use anyhow::Context;
use aries_init::GlobalContext;
use aries_persistence::ToolCallRepository;
use aries_tools::bash;
use clap::Parser;
use itertools::Itertools;
use jiff::{Span, Timestamp, Zoned};
use prettytable::{Cell, Row, Table, row};

#[derive(Parser, Debug, Clone)]
#[command(about = "Show bash command usage statistics")]
pub struct BashArgs {}

pub async fn execute(gctx: GlobalContext, _: BashArgs) -> anyhow::Result<()> {
    let db = aries_persistence::connect(&gctx.root_dir)
        .await
        .with_context(|| format!("connecting to database at {}", gctx.root_dir.display()))?;

    let now = Zoned::now();
    let thirty_days_ago = now.saturating_sub(Span::new().days(30));

    let mut tool_call_repo = ToolCallRepository::new(db.clone());

    let tool_calls = tool_call_repo
        .find_by_created_at_greater_than(Timestamp::from(&thirty_days_ago))
        .await
        .with_context(|| format!("finding tool calls from {}", gctx.root_dir.display()))?;

    let bash_tool_calls =
        tool_calls.into_iter().filter(|t| t.tool_name.eq(bash::NAME)).collect::<Vec<_>>();

    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_bash::LANGUAGE.into();
    parser.set_language(&language).with_context(|| "setting tree-sitter bash language")?;

    let mut calls = BTreeMap::<String, usize>::new();
    for call in bash_tool_calls {
        if let Ok(args) = serde_json::from_str::<bash::BashArgs>(&call.args)
            && let Some(tree) = parser.parse(&args.command, None)
        {
            let mut command_names = Vec::<String>::new();
            walk_node(tree.root_node(), &args.command, &mut command_names);
            command_names.into_iter().for_each(|name| {
                *calls.entry(name).or_insert(0) += 1;
            });
        }
    }
    println!("Bash command usage over the past 30 days:");
    let mut bash_table = Table::new();
    bash_table.add_row(row!["Command", "Calls"]);
    for (command_name, count) in calls.into_iter().sorted_by(|a, b| b.1.cmp(&a.1)) {
        bash_table.add_row(Row::new(vec![Cell::new(&command_name), Cell::new(&count.to_string())]));
    }
    bash_table.printstd();

    Ok(())
}

fn walk_node(node: tree_sitter::Node, source: &str, commands: &mut Vec<String>) {
    if node.kind() == "command_name" {
        if let Some(name_node) = node.child(0)
            && let Ok(name) = name_node.utf8_text(source.as_bytes())
        {
            commands.push(name.to_string());
        }
        return;
    }

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            walk_node(child, source, commands);
        }
    }
}
