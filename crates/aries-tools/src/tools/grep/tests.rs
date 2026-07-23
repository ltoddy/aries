// This file contains tests generated with AI assistance.

use super::*;

/// 构造带默认值的 GrepArgs，减少各用例的样板。
fn grep_args(pattern: &str) -> GrepArgs {
    GrepArgs {
        pattern: pattern.to_owned(),
        include: None,
        output_mode: OutputMode::default(),
        case_insensitive: false,
        show_line_numbers: true,
        context_before: None,
        context_after: None,
        context: None,
        head_limit: 250,
    }
}

#[test]
fn test_grep_args_title() {
    let args = grep_args("fn main");
    assert_eq!(args.title(), "Search for fn main in files");

    let mut args = grep_args("fn main");
    args.include = Some("src/**/*.rs".to_string());
    assert_eq!(args.title(), "Search for fn main in src/**/*.rs");
}

#[test]
fn test_grep_args_serde_defaults() {
    // 只给 pattern 时：output_mode=files_with_matches、case_insensitive=false、
    // show_line_numbers=true、head_limit=250、上下文行均为 None。
    let args: GrepArgs = serde_json::from_str(r#"{"pattern": "foo"}"#).unwrap();
    assert_eq!(args.output_mode, OutputMode::FilesWithMatches);
    assert!(!args.case_insensitive);
    assert!(args.show_line_numbers);
    assert_eq!(args.head_limit, 250);
    assert_eq!(args.context, None);
}

#[tokio::test]
async fn test_grep_finds_pattern() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join("test.rs"), "fn main() {\n    println!(\"hello\");\n}\n")
        .await
        .unwrap();

    let tool = GrepTool::new(tmp.path().to_path_buf());
    let mut args = grep_args("println");
    args.output_mode = OutputMode::Content;
    let result = tool.call(args).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert!(result.matches[0].contains("println"));
    // content 模式默认带行号，匹配行用 ':' 分隔。
    assert!(result.matches[0].contains("test.rs:2:"));
}

#[tokio::test]
async fn test_grep_case_insensitive() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join("a.rs"), "Hello World\n").await.unwrap();

    let tool = GrepTool::new(tmp.path().to_path_buf());

    // 默认区分大小写：小写 pattern 不命中。
    let mut sensitive = grep_args("hello");
    sensitive.output_mode = OutputMode::Content;
    assert!(tool.call(sensitive).await.unwrap().matches.is_empty());

    // 开启 case_insensitive 后命中。
    let mut insensitive = grep_args("hello");
    insensitive.output_mode = OutputMode::Content;
    insensitive.case_insensitive = true;
    assert_eq!(tool.call(insensitive).await.unwrap().matches.len(), 1);
}

#[tokio::test]
async fn test_grep_no_line_numbers() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join("a.rs"), "target line\n").await.unwrap();

    let tool = GrepTool::new(tmp.path().to_path_buf());
    let mut args = grep_args("target");
    args.output_mode = OutputMode::Content;
    args.show_line_numbers = false;
    let result = tool.call(args).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    // 关闭行号后，输出中不含 ":2:" 这样的行号片段。
    assert!(!result.matches[0].contains(":1:"));
    assert!(result.matches[0].contains("target line"));
}

#[tokio::test]
async fn test_grep_context_lines() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join("a.rs"), "line1\nline2\nMATCH\nline4\nline5\n").await.unwrap();

    let tool = GrepTool::new(tmp.path().to_path_buf());
    let mut args = grep_args("MATCH");
    args.output_mode = OutputMode::Content;
    args.context = Some(1);
    let result = tool.call(args).await.unwrap();
    // context=1：匹配行 + 前后各一行，共 3 行。
    assert_eq!(result.matches.len(), 3);
    assert!(result.matches[0].contains("line2"));
    assert!(result.matches[1].contains("MATCH"));
    assert!(result.matches[2].contains("line4"));
    // 上下文行用 '-' 分隔，匹配行用 ':' 分隔。
    assert!(result.matches[0].contains("a.rs-2-"));
    assert!(result.matches[1].contains("a.rs:3:"));
}

#[tokio::test]
async fn test_grep_files_with_matches_sorted_by_mtime() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join("old.rs"), "needle\n").await.unwrap();
    // 拉开 mtime，确保 new.rs 更新。
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    tokio::fs::write(tmp.path().join("new.rs"), "needle\n").await.unwrap();

    let tool = GrepTool::new(tmp.path().to_path_buf());
    // 默认 output_mode 即 files_with_matches。
    let result = tool.call(grep_args("needle")).await.unwrap();
    assert_eq!(result.matches, vec!["new.rs".to_string(), "old.rs".to_string()]);
}

#[tokio::test]
async fn test_grep_count_mode() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join("a.rs"), "hit\nmiss\nhit\nhit\n").await.unwrap();

    let tool = GrepTool::new(tmp.path().to_path_buf());
    let mut args = grep_args("hit");
    args.output_mode = OutputMode::Count;
    let result = tool.call(args).await.unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0], "a.rs:3");
}

#[tokio::test]
async fn test_grep_head_limit_truncates() {
    let tmp = tempfile::TempDir::new().unwrap();
    for i in 0..10 {
        tokio::fs::write(tmp.path().join(format!("f{i}.rs")), "needle\n").await.unwrap();
    }

    let tool = GrepTool::new(tmp.path().to_path_buf());
    let mut args = grep_args("needle");
    args.head_limit = 3;
    let result = tool.call(args).await.unwrap();
    assert_eq!(result.matches.len(), 3);
    assert!(result.truncated);
}

#[tokio::test]
async fn test_grep_no_matches_found() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join("a.rs"), "nothing here\n").await.unwrap();

    let tool = GrepTool::new(tmp.path().to_path_buf());
    let result = tool.call(grep_args("absent_pattern")).await.unwrap();
    assert!(result.matches.is_empty());
    assert!(!result.truncated);

    // render_output 对空结果返回 "No matches found"。
    let raw = serde_json::to_string(&result).unwrap();
    assert_eq!(GrepOutput::render_output(&raw).unwrap(), "No matches found");
}
