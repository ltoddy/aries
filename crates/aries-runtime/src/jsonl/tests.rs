use std::collections::BTreeMap;
use std::io;

use tempfile::TempDir;

use crate::jsonl::{self, JsonlAppender};

#[tokio::test]
async fn write_serializes_one_element_per_line_and_round_trips() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");
    let elements = ["hello", "line\nbreak", "中文"];

    jsonl::write(&path, &elements).await?;

    assert_eq!(tokio::fs::read_to_string(&path).await?, "\"hello\"\n\"line\\nbreak\"\n\"中文\"");
    assert_eq!(jsonl::read::<String>(&path).await?, elements);
    Ok(())
}

#[tokio::test]
async fn write_replaces_existing_content() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");
    tokio::fs::write(&path, "100\n200\n300\n").await?;

    jsonl::write(&path, &[1]).await?;

    assert_eq!(tokio::fs::read_to_string(&path).await?, "1");
    Ok(())
}

#[tokio::test]
async fn write_empty_input_creates_or_truncates_file() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");

    jsonl::write(&path, &[] as &[i32]).await?;
    assert!(tokio::fs::read(&path).await?.is_empty());

    tokio::fs::write(&path, "1\n2\n").await?;
    jsonl::write(&path, &[] as &[i32]).await?;
    assert!(tokio::fs::read(&path).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn write_serialization_error_preserves_existing_content() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");
    tokio::fs::write(&path, "1\n").await?;
    let elements = [BTreeMap::new(), BTreeMap::from([(vec![1, 2], 3)])];

    let error = jsonl::write(&path, &elements).await.unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("element 1"), "unexpected error: {error}");
    assert_eq!(tokio::fs::read_to_string(&path).await?, "1\n");
    Ok(())
}

#[tokio::test]
async fn write_returns_error_when_parent_is_missing() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("missing").join("records.jsonl");

    let error = jsonl::write(&path, &[1]).await.unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    Ok(())
}

#[tokio::test]
async fn read_handles_empty_files_and_whitespace() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");
    for (content, expected) in [
        ("", vec![]),
        (" \t\r\n\n", vec![]),
        ("1\n2", vec![1, 2]),
        ("\n 1 \r\n\n\t2\r\n", vec![1, 2]),
    ] {
        tokio::fs::write(&path, content).await?;

        assert_eq!(jsonl::read::<i32>(&path).await?, expected, "input: {content:?}");
    }
    Ok(())
}

#[tokio::test]
async fn read_returns_valid_prefix_before_deserialization_error() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");
    for content in ["1\ninvalid\n2\n", "1\n\"wrong type\"\n2\n", "1\n{\"unfinished\":"] {
        tokio::fs::write(&path, content).await?;

        assert_eq!(jsonl::read::<i32>(&path).await?, vec![1], "input: {content:?}");
    }
    Ok(())
}

#[tokio::test]
async fn read_returns_error_for_missing_file_or_invalid_utf8() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");

    let error = jsonl::read::<i32>(&path).await.unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);

    tokio::fs::write(&path, [0xff]).await?;
    let error = jsonl::read::<i32>(&path).await.unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    Ok(())
}

#[tokio::test]
async fn appender_open_creates_file_and_retains_path() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");

    let appender = JsonlAppender::open(&path).await?;

    assert_eq!(appender.file_path(), path);
    assert!(tokio::fs::read(&path).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn appender_open_returns_error_when_parent_is_missing() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("missing").join("records.jsonl");

    let error = JsonlAppender::open(&path).await.unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    Ok(())
}

#[tokio::test]
async fn append_preserves_existing_content_and_adds_newlines() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");
    tokio::fs::write(&path, "1\n").await?;
    let appender = JsonlAppender::open(&path).await?;

    appender.append(&[2, 3]).await?;
    appender.append(&[] as &[i32]).await?;
    appender.append(&[4]).await?;
    appender.flush().await?;

    assert_eq!(tokio::fs::read_to_string(&path).await?, "1\n2\n3\n4\n");
    assert_eq!(jsonl::read::<i32>(&path).await?, vec![1, 2, 3, 4]);
    Ok(())
}

#[tokio::test]
async fn overwrite_truncates_previous_content_and_allows_append() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");
    let appender = JsonlAppender::open(&path).await?;
    appender.append(&[100, 200, 300]).await?;
    appender.flush().await?;

    appender.overwrite(&[1]).await?;
    appender.flush().await?;
    assert_eq!(tokio::fs::read_to_string(&path).await?, "1\n");

    appender.append(&[2]).await?;
    appender.flush().await?;
    assert_eq!(tokio::fs::read_to_string(&path).await?, "1\n2\n");
    Ok(())
}

#[tokio::test]
async fn overwrite_empty_input_clears_file() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");
    let appender = JsonlAppender::open(&path).await?;
    appender.append(&[1, 2]).await?;
    appender.flush().await?;

    appender.overwrite(&[] as &[i32]).await?;
    appender.flush().await?;
    assert!(tokio::fs::read(&path).await?.is_empty());

    appender.append(&[3]).await?;
    appender.flush().await?;
    assert_eq!(tokio::fs::read_to_string(&path).await?, "3\n");
    Ok(())
}

#[tokio::test]
async fn append_serialization_error_keeps_already_written_elements() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");
    tokio::fs::write(&path, "1\n").await?;
    let appender = JsonlAppender::open(&path).await?;
    let elements = [BTreeMap::new(), BTreeMap::from([(vec![1, 2], 3)])];

    let error = appender.append(&elements).await.unwrap_err();
    appender.flush().await?;

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("element 1"), "unexpected error: {error}");
    assert_eq!(tokio::fs::read_to_string(&path).await?, "1\n{}\n");
    Ok(())
}

#[tokio::test]
async fn overwrite_serialization_error_keeps_only_written_replacement_elements() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");
    tokio::fs::write(&path, "1\n2\n3\n").await?;
    let appender = JsonlAppender::open(&path).await?;
    let elements = [BTreeMap::new(), BTreeMap::from([(vec![1, 2], 3)])];

    let error = appender.overwrite(&elements).await.unwrap_err();
    appender.flush().await?;

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("element 1"), "unexpected error: {error}");
    assert_eq!(tokio::fs::read_to_string(&path).await?, "{}\n");
    Ok(())
}

#[tokio::test]
async fn cloned_appenders_do_not_interleave_batches() -> io::Result<()> {
    let dir = TempDir::new()?;
    let path = dir.path().join("records.jsonl");
    let appender = JsonlAppender::open(&path).await?;
    let cloned = appender.clone();

    let (first, second) = tokio::join!(appender.append(&[1, 2]), cloned.append(&[3, 4]));
    first?;
    second?;
    cloned.flush().await?;

    let content = tokio::fs::read_to_string(&path).await?;
    assert!(
        content == "1\n2\n3\n4\n" || content == "3\n4\n1\n2\n",
        "interleaved or missing records: {content:?}"
    );
    Ok(())
}
