// This file contains tests generated with AI assistance.

use super::*;

fn glob_args(pattern: &str) -> GlobArgs {
    GlobArgs { pattern: pattern.to_owned(), base_dir: None, hidden: false, respect_gitignore: true }
}

#[test]
fn test_glob_args_title() {
    let args = glob_args("src/**/*.rs");
    assert_eq!(args.title(), "Find files matching src/**/*.rs");
}

#[test]
fn test_glob_args_serde_defaults() {
    // 只给 pattern 时，hidden 默认 false、respect_gitignore 默认 true。
    let args: GlobArgs = serde_json::from_str(r#"{"pattern": "*.rs"}"#).unwrap();
    assert!(!args.hidden);
    assert!(args.respect_gitignore);
}

#[tokio::test]
async fn test_glob_finds_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join("a.rs"), "").await.unwrap();
    tokio::fs::create_dir(tmp.path().join("sub")).await.unwrap();
    tokio::fs::write(tmp.path().join("sub/b.rs"), "").await.unwrap();

    let mut context = ToolContext::new();
    let tool = GlobTool::new(tmp.path().to_path_buf());
    let result = tool.call(&mut context, glob_args("*.rs")).await.unwrap();
    assert!(result.files.contains(&PathBuf::from("a.rs")));
}

#[tokio::test]
async fn test_glob_recursive() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::create_dir(tmp.path().join("sub")).await.unwrap();
    tokio::fs::write(tmp.path().join("sub/b.rs"), "").await.unwrap();

    let mut context = ToolContext::new();
    let tool = GlobTool::new(tmp.path().to_path_buf());
    let result = tool.call(&mut context, glob_args("**/*.rs")).await.unwrap();
    assert_eq!(result.files, vec![PathBuf::from("sub/b.rs")]);
    assert!(!result.truncated);
}

#[tokio::test]
async fn test_glob_sorts_by_mtime_newest_first() {
    let tmp = tempfile::TempDir::new().unwrap();
    // 先写 old.rs，隔开一段时间再写 new.rs，确保 mtime 有明确先后。
    tokio::fs::write(tmp.path().join("old.rs"), "").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    tokio::fs::write(tmp.path().join("new.rs"), "").await.unwrap();

    let mut context = ToolContext::new();
    let tool = GlobTool::new(tmp.path().to_path_buf());
    let result = tool.call(&mut context, glob_args("*.rs")).await.unwrap();

    // 降序：最新在前。
    assert_eq!(result.files, vec![PathBuf::from("new.rs"), PathBuf::from("old.rs")]);
}

#[tokio::test]
async fn test_glob_truncates_over_limit() {
    let tmp = tempfile::TempDir::new().unwrap();
    for i in 0..(MAX_RESULTS + 10) {
        tokio::fs::write(tmp.path().join(format!("f{i}.rs")), "").await.unwrap();
    }

    let mut context = ToolContext::new();
    let tool = GlobTool::new(tmp.path().to_path_buf());
    let result = tool.call(&mut context, glob_args("*.rs")).await.unwrap();

    assert_eq!(result.files.len(), MAX_RESULTS);
    assert!(result.truncated);
}

#[tokio::test]
async fn test_glob_no_files_found() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join("a.txt"), "").await.unwrap();

    let mut context = ToolContext::new();
    let tool = GlobTool::new(tmp.path().to_path_buf());
    let result = tool.call(&mut context, glob_args("*.rs")).await.unwrap();
    assert!(result.files.is_empty());

    // render_output 对空结果给出明确提示。
    let raw = serde_json::to_value(&result).unwrap();
    assert_eq!(GlobOutput::render_output(raw).unwrap(), "No files found");
}

#[tokio::test]
async fn test_glob_hidden_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join(".hidden.rs"), "").await.unwrap();

    let mut context = ToolContext::new();
    let tool = GlobTool::new(tmp.path().to_path_buf());

    // 默认跳过隐藏文件。
    let default_result = tool.call(&mut context, glob_args("*.rs")).await.unwrap();
    assert!(default_result.files.is_empty());

    // 显式 hidden=true 时包含隐藏文件。
    let args = GlobArgs {
        pattern: "*.rs".to_owned(),
        base_dir: None,
        hidden: true,
        respect_gitignore: true,
    };
    let hidden_result = tool.call(&mut context, args).await.unwrap();
    assert!(hidden_result.files.contains(&PathBuf::from(".hidden.rs")));
}
