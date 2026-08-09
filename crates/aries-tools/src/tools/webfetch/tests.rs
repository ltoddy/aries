// This file contains tests generated with AI assistance.

use super::*;

#[test]
fn renders_markdown_output() {
    let raw = serde_json::json!({ "content": "# Firecrawl" });

    assert_eq!(WebFetchOutput::render_output(raw).unwrap(), "# Firecrawl");
}
