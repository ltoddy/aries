// This file contains tests generated with AI assistance.

use std::path::PathBuf;

use serde_json::Value;

use crate::document::{DocumentError, FrontmatterDocument};
use crate::jsonl::{self, JsonlAppender};
use crate::walk::{walk_dir, walk_dirs};
use crate::{path_to_slug, path_to_uri};

async fn write_document(content: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("document.md");
    tokio::fs::write(&file_path, content).await.unwrap();
    (dir, file_path)
}

#[tokio::test]
async fn reads_frontmatter_document() {
    let (_dir, file_path) = write_document("---\nname: test\n---\nbody").await;

    let document = FrontmatterDocument::<Value>::read(&file_path).await.unwrap();

    assert_eq!(document.frontmatter["name"], "test");
    assert_eq!(document.body, "body");
}

#[tokio::test]
async fn writes_and_reads_jsonl_values() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("events.jsonl");
    let values = [serde_json::json!({ "id": 1 }), serde_json::json!({ "id": 2 })];

    jsonl::write(&file_path, &values).await.unwrap();
    let result = jsonl::read::<Value>(&file_path).await.unwrap();

    assert_eq!(result, values);
}

#[tokio::test]
async fn jsonl_read_drops_invalid_trailing_line() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("events.jsonl");
    tokio::fs::write(&file_path, "{\"id\":1}\nnot-json").await.unwrap();

    let result = jsonl::read::<Value>(&file_path).await.unwrap();

    assert_eq!(result, [serde_json::json!({ "id": 1 })]);
}

#[tokio::test]
async fn jsonl_appender_appends_and_overwrites_values() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("events.jsonl");
    let appender = JsonlAppender::open(&file_path).await.unwrap();

    appender.append(&[serde_json::json!({ "id": 1 })]).await.unwrap();
    appender.append(&[serde_json::json!({ "id": 2 })]).await.unwrap();
    appender.flush().await.unwrap();
    let result = jsonl::read::<Value>(&file_path).await.unwrap();
    assert_eq!(result, [serde_json::json!({ "id": 1 }), serde_json::json!({ "id": 2 })]);

    appender.overwrite(&[serde_json::json!({ "id": 3 })]).await.unwrap();
    appender.flush().await.unwrap();
    let result = jsonl::read::<Value>(&file_path).await.unwrap();
    assert_eq!(result, [serde_json::json!({ "id": 3 })]);
}

#[tokio::test]
async fn lock_creates_file_without_truncating_existing_content() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("lock-file");
    tokio::fs::write(&file_path, "keep me").await.unwrap();

    let file = crate::lock::lock(&file_path).await.unwrap();
    drop(file);

    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(content, "keep me");
}

#[tokio::test]
async fn try_lock_fails_when_file_is_already_locked() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("lock-file");
    let file = crate::lock::lock(&file_path).await.unwrap();

    let result = crate::lock::try_lock(&file_path).await;

    assert!(result.is_err());
    drop(file);
}

#[test]
fn walk_dir_returns_direct_children_when_not_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("file.txt"), "content").unwrap();
    std::fs::create_dir(root.join("nested")).unwrap();
    std::fs::write(root.join("nested").join("child.txt"), "content").unwrap();

    let entries = walk_dir(root, false, false).unwrap();

    assert!(entries.contains(&root.join("file.txt")));
    assert!(entries.contains(&root.join("nested")));
    assert!(!entries.contains(&root.join("nested").join("child.txt")));
}

#[test]
fn walk_dir_returns_descendants_when_recursive() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir(root.join("nested")).unwrap();
    std::fs::write(root.join("nested").join("child.txt"), "content").unwrap();

    let entries = walk_dir(root, true, false).unwrap();

    assert!(entries.contains(&root.join("nested")));
    assert!(entries.contains(&root.join("nested").join("child.txt")));
}

#[test]
fn walk_dir_respects_hidden_filter() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("visible.txt"), "content").unwrap();
    std::fs::write(root.join(".hidden.txt"), "content").unwrap();

    let all_entries = walk_dir(root, false, false).unwrap();
    let visible_entries = walk_dir(root, false, true).unwrap();

    assert!(all_entries.contains(&root.join("visible.txt")));
    assert!(all_entries.contains(&root.join(".hidden.txt")));
    assert!(visible_entries.contains(&root.join("visible.txt")));
    assert!(!visible_entries.contains(&root.join(".hidden.txt")));
}

#[test]
fn walk_dir_errors_for_non_directory_root() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    std::fs::write(&file_path, "content").unwrap();

    let result = walk_dir(&file_path, false, false);

    assert!(result.is_err());
}

#[test]
fn walk_dirs_skips_non_directory_roots() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("file.txt"), "content").unwrap();
    let non_directory = dir.path().join("file.txt");
    std::fs::write(&non_directory, "content").unwrap();

    let entries = walk_dirs([&root, &non_directory], false, false).unwrap();

    assert_eq!(entries, [root.join("file.txt")]);
}

#[tokio::test]
async fn path_to_uri_returns_file_uri_for_existing_path() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    tokio::fs::write(&file_path, "content").await.unwrap();

    let uri = path_to_uri(&file_path).await;

    assert!(uri.starts_with("file://"));
    assert!(uri.ends_with("/file.txt"));
}

#[test]
fn path_to_slug_replaces_non_alphanumeric_characters() {
    let slug = path_to_slug("foo/bar baz.txt");

    assert!(slug.ends_with("foo-bar-baz-txt"));
}

#[tokio::test]
async fn rejects_frontmatter_not_at_start_of_file() {
    let (_dir, file_path) = write_document("prefix---\nname: test\n---\nbody").await;

    let result = FrontmatterDocument::<Value>::read(&file_path).await;

    assert!(matches!(result, Err(DocumentError::WrongFormat { .. })));
}

#[tokio::test]
async fn rejects_missing_closing_delimiter() {
    let (_dir, file_path) = write_document("---\nname: test\nbody").await;

    let result = FrontmatterDocument::<Value>::read(&file_path).await;

    assert!(matches!(result, Err(DocumentError::WrongFormat { .. })));
}

#[tokio::test]
async fn rejects_invalid_yaml_frontmatter() {
    let (_dir, file_path) = write_document("---\nname: [\n---\nbody").await;

    let result = FrontmatterDocument::<Value>::read(&file_path).await;

    assert!(matches!(result, Err(DocumentError::Yaml { .. })));
}

#[tokio::test]
async fn writes_readable_frontmatter_document() {
    let file = tempfile::NamedTempFile::new().unwrap();
    let file_path = file.path().to_owned();
    let document =
        FrontmatterDocument::new(&file_path, serde_json::json!({ "name": "test" }), "body");

    document.write().await.unwrap();
    let document = FrontmatterDocument::<Value>::read(&file_path).await.unwrap();

    assert_eq!(document.frontmatter["name"], "test");
    assert_eq!(document.body, "body");
}
