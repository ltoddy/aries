use std::collections::BTreeMap;

use aries_init::GlobalContext;
use aries_persistence::{ToolCall, ToolCallRepository};
use clap::Parser;
use jiff::{Span, Timestamp, Zoned};
use prettytable::{Cell, Row, Table, row};

use crate::Result;
use crate::error::Error;

#[derive(Parser, Debug, Clone)]
#[command(about = "Show tool call statistics")]
pub struct ToolArgs {}

pub async fn execute(gctx: GlobalContext, _: ToolArgs) -> Result<()> {
    let root_dir = gctx.root_dir();
    let db = aries_persistence::connect(root_dir)
        .await
        .map_err(|source| Error::database("connecting to database at", root_dir, source))?;

    let now = Zoned::now();
    let thirty_days_ago = now.saturating_sub(Span::new().days(30));

    let mut tool_call_repo = ToolCallRepository::new(db);

    let tool_calls = tool_call_repo
        .find_by_created_at_greater_than(Timestamp::from(&thirty_days_ago))
        .await
        .map_err(|source| Error::database("finding tool calls from", root_dir, source))?;

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
