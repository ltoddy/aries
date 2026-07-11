// This file contains tests generated with AI assistance.

use super::*;

#[test]
fn test_glob_args_title() {
    let args = GlobArgs {
        pattern: "src/**/*.rs".to_string(),
        base_dir: None,
    };
    assert_eq!(args.title(), "Find files matching src/**/*.rs");
}

#[tokio::test]
async fn test_glob_finds_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::write(tmp.path().join("a.rs"), "").await.unwrap();
    tokio::fs::create_dir(tmp.path().join("sub")).await.unwrap();
    tokio::fs::write(tmp.path().join("sub/b.rs"), "").await.unwrap();

    let tool = GlobTool::new(tmp.path().to_path_buf());
    let result = tool
        .call(GlobArgs {
            pattern: "*.rs".to_string(),
            base_dir: None,
        })
        .await
        .unwrap();
    assert!(result.files.contains(&"a.rs".to_string()));
}

#[tokio::test]
async fn test_glob_recursive() {
    let tmp = tempfile::TempDir::new().unwrap();
    tokio::fs::create_dir(tmp.path().join("sub")).await.unwrap();
    tokio::fs::write(tmp.path().join("sub/b.rs"), "").await.unwrap();

    let tool = GlobTool::new(tmp.path().to_path_buf());
    let result = tool
        .call(GlobArgs {
            pattern: "**/*.rs".to_string(),
            base_dir: None,
        })
        .await
        .unwrap();
    assert_eq!(result.files, vec!["sub/b.rs"]);
}
