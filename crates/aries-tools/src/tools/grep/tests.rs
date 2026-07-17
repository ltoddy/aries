// This file contains tests generated with AI assistance.

use super::*;

#[test]
fn test_grep_args_title() {
    let args = GrepArgs { pattern: "fn main".to_string(), include: None };
    assert_eq!(args.title(), "Search for fn main in files");

    let args =
        GrepArgs { pattern: "fn main".to_string(), include: Some("src/**/*.rs".to_string()) };
    assert_eq!(args.title(), "Search for fn main in src/**/*.rs");
}

#[tokio::test]
async fn test_grep_finds_pattern() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join("test.rs"), "fn main() {\n    println!(\"hello\");\n}\n")
        .await
        .unwrap();

    let tool = GrepTool::new(tmp.path().to_path_buf());
    let result =
        tool.call(GrepArgs { pattern: r"println".to_string(), include: None }).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert!(result.matches[0].contains("println"));
}
