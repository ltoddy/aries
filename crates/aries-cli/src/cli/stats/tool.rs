use std::collections::BTreeMap;

use anyhow::Context;
use aries_init::GlobalContext;
use aries_persistence::{ToolCall, ToolCallRepository};
use clap::Parser;
use jiff::{Span, Timestamp, Zoned};
use prettytable::{Cell, Row, Table, row};

#[derive(Parser, Debug, Clone)]
pub struct ToolArgs {}

pub async fn execute(gctx: GlobalContext, _: ToolArgs) -> anyhow::Result<()> {
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

    let mut calls_by_tool = BTreeMap::<String, Vec<ToolCall>>::new();
    for tool_call in tool_calls {
        calls_by_tool.entry(tool_call.tool_name.clone()).or_default().push(tool_call);
    }

    println!("Tool call activity over the past 30 days:");
    let mut tool_table = Table::new();
    tool_table.add_row(row!["Tool Name", "Calls"]);
    for (tool_name, calls) in calls_by_tool {
        tool_table
            .add_row(Row::new(vec![Cell::new(&tool_name), Cell::new(&calls.len().to_string())]));
    }
    tool_table.printstd();

    Ok(())
}
