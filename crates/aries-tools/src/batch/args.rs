use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const NAME: &str = "Batch";

#[derive(Debug, Deserialize, Serialize)]
pub struct BatchCall {
    pub tool: String,
    pub parameters: Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BatchArgs {
    pub calls: Vec<BatchCall>,
}

impl BatchArgs {
    pub fn title(&self) -> String {
        format!("Run {} tool calls in parallel", self.calls.len())
    }
}

impl BatchArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;

        let first = format!("{} tool calls", args.calls.len());
        if args.calls.is_empty() {
            return Ok((first, None));
        }

        let rest: Vec<String> = args
            .calls
            .iter()
            .map(|c| {
                if c.tool == NAME {
                    format!("- {} (nested batch not allowed)", c.tool)
                } else {
                    format!("- {}: {}", c.tool, c.parameters)
                }
            })
            .collect();

        Ok((first, Some(rest.join("\n"))))
    }
}
