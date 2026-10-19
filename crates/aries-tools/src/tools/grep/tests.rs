use std::path::Path;

use tokio::process::Command as Bash;

use super::*;

fn tool(cwd: impl AsRef<Path>) -> GrepTool {
    let (notifier, _) = aries_event::Notifier::channel();
    GrepTool::new(crate::context::ToolContext::new(None, notifier, cwd))
}

fn grep_args(pattern: &str) -> GrepArgs {
    GrepArgs {
        pattern: pattern.to_owned(),
        include: None,
        output_mode: OutputMode::default(),
        case_insensitive: false,
        show_line_numbers: true,
        context: 0,
        hidden: false,
        respect_ignore: true,
        limit: 250,
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
    let args: GrepArgs =
        serde_json::from_str(r#"{"pattern": "foo"}"#).expect("test operation should succeed");
    assert_eq!(args.output_mode, OutputMode::FilesWithMatches);
    assert!(!args.case_insensitive);
    assert!(args.show_line_numbers);
    assert!(!args.hidden);
    assert!(args.respect_ignore);
    assert_eq!(args.limit, 250);
    let serialized = serde_json::to_value(&args).expect("test operation should succeed");
    assert_eq!(serialized["context"], 0);
    assert!(serialized.get("context_before").is_none());
    assert!(serialized.get("context_after").is_none());
}

#[tokio::test]
async fn test_grep_finds_pattern() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("test.rs"), "fn main() {\n    println!(\"hello\");\n}\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("println");
    args.output_mode = OutputMode::Content;
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");
    assert_eq!(result.matches.len(), 1);
    assert!(result.matches[0].contains("println"));
    assert!(result.matches[0].contains("test.rs:2:"));
}

#[tokio::test]
async fn test_grep_case_insensitive() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "Hello World\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());

    let mut sensitive = grep_args("hello");
    sensitive.output_mode = OutputMode::Content;
    assert!(
        tool.call(&mut context, sensitive)
            .await
            .expect("test operation should succeed")
            .matches
            .is_empty()
    );

    let mut insensitive = grep_args("hello");
    insensitive.output_mode = OutputMode::Content;
    insensitive.case_insensitive = true;
    assert_eq!(
        tool.call(&mut context, insensitive)
            .await
            .expect("test operation should succeed")
            .matches
            .len(),
        1
    );
}

#[tokio::test]
async fn test_grep_no_line_numbers() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "target line\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("target");
    args.output_mode = OutputMode::Content;
    args.show_line_numbers = false;
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");
    assert_eq!(result.matches.len(), 1);
    assert!(!result.matches[0].contains(":1:"));
    assert!(result.matches[0].contains("target line"));
}

#[tokio::test]
async fn test_grep_context_lines() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "line1\nline2\nMATCH\nline4\nline5\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("MATCH");
    args.output_mode = OutputMode::Content;
    args.context = 1;
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");
    assert_eq!(result.matches.len(), 3);
    assert!(result.matches[0].contains("line2"));
    assert!(result.matches[1].contains("MATCH"));
    assert!(result.matches[2].contains("line4"));
    assert!(result.matches[0].contains("a.rs-2-"));
    assert!(result.matches[1].contains("a.rs:3:"));
}

#[tokio::test]
async fn test_grep_files_with_matches_sorted_by_mtime() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("old.rs"), "needle\n")
        .await
        .expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("new.rs"), "needle\n")
        .await
        .expect("test operation should succeed");
    set_modified(&tmp.path().join("old.rs"), 1);
    set_modified(&tmp.path().join("new.rs"), 2);

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let result =
        tool.call(&mut context, grep_args("needle")).await.expect("test operation should succeed");
    assert_eq!(result.matches, vec!["new.rs".to_string(), "old.rs".to_string()]);
}

#[tokio::test]
async fn test_grep_files_with_matches_keeps_newest_file_under_limit() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("old.rs"), "needle\n")
        .await
        .expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("new.rs"), "needle\n")
        .await
        .expect("test operation should succeed");
    set_modified(&tmp.path().join("old.rs"), 1);
    set_modified(&tmp.path().join("new.rs"), 2);

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("needle");
    args.limit = 1;
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");
    assert_eq!(result.matches, vec!["new.rs".to_string()]);
    assert!(result.truncated);
}

#[tokio::test]
async fn test_grep_count_mode() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "hit\nmiss\nhit\nhit\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("hit");
    args.output_mode = OutputMode::Count;
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0], "a.rs:3");
}

#[tokio::test]
async fn test_grep_context_at_file_boundary() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "line1\nline2\nMATCH\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("MATCH");
    args.output_mode = OutputMode::Content;
    args.context = 5;
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");
    assert_eq!(result.matches, ["a.rs-1-line1", "a.rs-2-line2", "a.rs:3:MATCH"]);
}

#[tokio::test]
async fn test_grep_context_saturates_no_overflow() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "alpha\nMATCH\nomega\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("MATCH");
    args.output_mode = OutputMode::Content;
    args.context = usize::MAX;
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");
    assert_eq!(result.matches.len(), 3);
}

#[tokio::test]
async fn test_grep_limit_truncates_match_groups() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    for i in 0..10 {
        tokio::fs::write(tmp.path().join(format!("f{i}.rs")), format!("needle-{i}\n"))
            .await
            .expect("test operation should succeed");
    }

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("needle");
    args.output_mode = OutputMode::Content;
    args.limit = 3;
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");
    assert_eq!(result.matches.len(), 3);
    assert!(result.truncated);
    assert!(result.matches.iter().all(|line| line.contains("needle-")));
}

#[tokio::test]
async fn test_grep_content_limit_drops_context_for_truncated_match() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "a\nneedle\nc\nd\ne\nneedle\ng\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("needle");
    args.output_mode = OutputMode::Content;
    args.context = 1;
    args.limit = 1;
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");

    assert_eq!(result.matches, ["a.rs-1-a", "a.rs:2:needle", "a.rs-3-c"]);
    assert!(result.truncated);
}

#[tokio::test]
async fn test_grep_content_limit_keeps_shared_context_once() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "a\nneedle\nb\nneedle\nc\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("needle");
    args.output_mode = OutputMode::Content;
    args.context = 1;
    args.limit = 1;
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");

    assert_eq!(result.matches, ["a.rs-1-a", "a.rs:2:needle", "a.rs-3-b"]);
    assert!(result.truncated);
}

#[tokio::test]
async fn test_grep_content_limit_drops_touching_surrounding_context() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "a\nneedle\nb\nc\nneedle\nd\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    for (limit, expected, truncated) in [
        (1, vec!["a.rs-1-a", "a.rs:2:needle", "a.rs-3-b"], true),
        (
            0,
            vec!["a.rs-1-a", "a.rs:2:needle", "a.rs-3-b", "a.rs-4-c", "a.rs:5:needle", "a.rs-6-d"],
            false,
        ),
    ] {
        let mut args = grep_args("needle");
        args.output_mode = OutputMode::Content;
        args.context = 1;
        args.limit = limit;
        let result = tool.call(&mut context, args).await.expect("test operation should succeed");

        assert_eq!(result.matches, expected, "limit={limit}");
        assert_eq!(result.truncated, truncated, "limit={limit}");
    }
}

#[tokio::test]
async fn test_grep_content_limit_applies_within_single_file() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    let mut body = String::new();
    for i in 0..1000 {
        body.push_str(&format!("needle-{i}\nother\n"));
    }
    tokio::fs::write(tmp.path().join("a.rs"), body).await.expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("needle");
    args.output_mode = OutputMode::Content;
    args.limit = 3;
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");

    assert_eq!(result.matches.len(), 3);
    assert!(result.truncated);
}

#[tokio::test]
async fn test_grep_no_matches_found() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "nothing here\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let result = tool
        .call(&mut context, grep_args("absent_pattern"))
        .await
        .expect("test operation should succeed");
    assert!(result.matches.is_empty());
    assert!(!result.truncated);

    let raw = serde_json::to_value(&result).expect("test operation should succeed");
    assert_eq!(
        GrepOutput::render_output(raw).expect("test operation should succeed"),
        "No matches found"
    );
}

#[tokio::test]
async fn test_grep_respects_gitignore_by_default() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    Bash::new("git")
        .args(["init", "-q"])
        .current_dir(tmp.path())
        .output()
        .await
        .expect("test operation should succeed");
    tokio::fs::write(tmp.path().join(".gitignore"), "ignored/\n")
        .await
        .expect("test operation should succeed");
    tokio::fs::create_dir(tmp.path().join("ignored")).await.expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("ignored").join("a.rs"), "needle\n")
        .await
        .expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("visible.rs"), "needle\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let result =
        tool.call(&mut context, grep_args("needle")).await.expect("test operation should succeed");

    assert_eq!(result.matches, vec!["visible.rs".to_string()]);
}

#[tokio::test]
async fn test_grep_can_disable_gitignore_filtering() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    Bash::new("git")
        .args(["init", "-q"])
        .current_dir(tmp.path())
        .output()
        .await
        .expect("test operation should succeed");
    tokio::fs::write(tmp.path().join(".gitignore"), "ignored/\n")
        .await
        .expect("test operation should succeed");
    tokio::fs::create_dir(tmp.path().join("ignored")).await.expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("ignored").join("a.rs"), "needle\n")
        .await
        .expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("visible.rs"), "needle\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("needle");
    args.respect_ignore = false;
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");

    assert!(result.matches.contains(&"visible.rs".to_string()));
    assert!(result.matches.contains(&"ignored/a.rs".to_string()));
}

#[tokio::test]
async fn test_grep_include_filters_files() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::create_dir_all(tmp.path().join("src")).await.expect("test operation should succeed");
    tokio::fs::create_dir_all(tmp.path().join("docs"))
        .await
        .expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("src").join("main.rs"), "needle\n")
        .await
        .expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("docs").join("note.txt"), "needle\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("needle");
    args.include = Some("src/**/*.rs".to_string());
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");

    assert_eq!(result.matches, vec!["src/main.rs".to_string()]);
}

#[tokio::test]
async fn test_grep_include_keeps_nested_target_subtree() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::create_dir_all(tmp.path().join("src").join("nested"))
        .await
        .expect("test operation should succeed");
    tokio::fs::create_dir_all(tmp.path().join("vendor"))
        .await
        .expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("src").join("nested").join("lib.rs"), "needle\n")
        .await
        .expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("vendor").join("lib.rs"), "needle\n")
        .await
        .expect("test operation should succeed");

    let mut context = ToolContext::new();
    let tool = tool(tmp.path());
    let mut args = grep_args("needle");
    args.include = Some("src/**/*.rs".to_string());
    let result = tool.call(&mut context, args).await.expect("test operation should succeed");

    assert_eq!(result.matches, vec!["src/nested/lib.rs".to_string()]);
}

#[tokio::test]
async fn test_grep_content_exact_limit_is_not_truncated() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "needle\nneedle\nneedle\n")
        .await
        .expect("test operation should succeed");
    let mut args = grep_args("needle");
    args.output_mode = OutputMode::Content;
    args.limit = 3;

    let result = tool(tmp.path())
        .call(&mut ToolContext::new(), args)
        .await
        .expect("test operation should succeed");

    assert_eq!(result.matches.len(), 3);
    assert!(!result.truncated);
}

#[tokio::test]
async fn test_grep_count_exact_limit_is_not_truncated() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "needle\nneedle\n")
        .await
        .expect("test operation should succeed");
    let mut args = grep_args("needle");
    args.output_mode = OutputMode::Count;
    args.limit = 1;

    let result = tool(tmp.path())
        .call(&mut ToolContext::new(), args)
        .await
        .expect("test operation should succeed");

    assert_eq!(result.matches, ["a.rs:2"]);
    assert!(!result.truncated);
}

#[tokio::test]
async fn test_grep_missing_directory_is_an_error() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    let result =
        tool(tmp.path().join("missing")).call(&mut ToolContext::new(), grep_args("needle")).await;

    let error = result.expect_err("a missing search directory must not look like an empty result");
    assert!(error.to_string().contains("missing"));
}

#[tokio::test]
async fn test_grep_reports_file_search_errors() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("large.rs"), "x".repeat(11 * 1024 * 1024))
        .await
        .expect("test operation should succeed");
    let mut args = grep_args("needle");
    args.output_mode = OutputMode::Content;
    let result = tool(tmp.path()).call(&mut ToolContext::new(), args).await;

    let error = result.expect_err("exceeding the search buffer must not look like no matches");
    assert!(error.to_string().contains("large.rs"));
}

#[tokio::test]
async fn test_grep_include_accepts_dot_prefix() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::create_dir(tmp.path().join("src")).await.expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("src/lib.rs"), "needle\n")
        .await
        .expect("test operation should succeed");
    let mut args = grep_args("needle");
    args.include = Some("./src/**/*.rs".to_owned());

    let result = tool(tmp.path())
        .call(&mut ToolContext::new(), args)
        .await
        .expect("test operation should succeed");

    assert_eq!(result.matches, ["src/lib.rs"]);
}

#[tokio::test]
async fn test_grep_limit_preserves_after_context_containing_matches() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "needle\nneedle\nafter\nexcluded\n")
        .await
        .expect("test operation should succeed");
    let mut args = grep_args("needle");
    args.output_mode = OutputMode::Content;
    args.context = 2;
    args.limit = 1;

    let result = tool(tmp.path())
        .call(&mut ToolContext::new(), args)
        .await
        .expect("test operation should succeed");

    assert_eq!(result.matches, ["a.rs:1:needle", "a.rs-2-needle", "a.rs-3-after"]);
    assert!(result.truncated);
}

#[tokio::test]
async fn test_grep_anchors_match_each_line() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "before\nneedle\nafter\n")
        .await
        .expect("test operation should succeed");
    for (mode, expected) in [
        (OutputMode::Content, "a.rs:2:needle"),
        (OutputMode::Count, "a.rs:1"),
        (OutputMode::FilesWithMatches, "a.rs"),
    ] {
        let mut args = grep_args("^needle$");
        args.output_mode = mode;
        let result = tool(tmp.path())
            .call(&mut ToolContext::new(), args)
            .await
            .expect("test operation should succeed");
        assert_eq!(result.matches, [expected], "mode={mode:?}");
    }
}

#[tokio::test]
async fn test_grep_rejects_multiline_patterns() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "first\nsecond\n")
        .await
        .expect("test operation should succeed");
    let result = tool(tmp.path()).call(&mut ToolContext::new(), grep_args(r"first\nsecond")).await;
    assert!(matches!(result, Err(GrepError::Regex { pattern: _, source: _ })));
}

#[tokio::test]
async fn test_grep_limits_across_many_files_in_all_modes() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    for index in 0..24 {
        let path = tmp.path().join(format!("f{index:02}.rs"));
        tokio::fs::write(&path, "needle\n").await.expect("test operation should succeed");
        set_modified(&path, index + 1);
    }
    for mode in [OutputMode::Content, OutputMode::Count, OutputMode::FilesWithMatches] {
        for limit in [0, 1, 7, 24, 25, usize::MAX] {
            let mut args = grep_args("needle");
            args.output_mode = mode;
            args.limit = limit;
            let result = tool(tmp.path())
                .call(&mut ToolContext::new(), args)
                .await
                .expect("test operation should succeed");
            let count = if limit == 0 { 24 } else { limit.min(24) };
            assert_eq!(result.matches.len(), count, "mode={mode:?}, limit={limit}");
            assert_eq!(result.truncated, limit != 0 && limit < 24);
            let unique: std::collections::HashSet<_> = result.matches.iter().collect();
            assert_eq!(unique.len(), count);
            if mode == OutputMode::FilesWithMatches {
                let expected: Vec<_> =
                    (0..24).rev().take(count).map(|i| format!("f{i:02}.rs")).collect();
                assert_eq!(result.matches, expected);
            }
        }
    }
}

#[tokio::test]
async fn test_grep_hidden_files_and_directories() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::create_dir(tmp.path().join(".hidden")).await.expect("test operation should succeed");
    for name in ["visible.rs", ".hidden.rs", ".hidden/nested.rs"] {
        tokio::fs::write(tmp.path().join(name), "needle\n")
            .await
            .expect("test operation should succeed");
    }
    for hidden in [false, true] {
        let mut args = grep_args("needle");
        args.hidden = hidden;
        args.respect_ignore = false;
        args.include = Some("**/*.rs".to_owned());
        let mut result = tool(tmp.path())
            .call(&mut ToolContext::new(), args)
            .await
            .expect("test operation should succeed");
        result.matches.sort();
        if hidden {
            assert_eq!(result.matches, [".hidden.rs", ".hidden/nested.rs", "visible.rs"]);
        } else {
            assert_eq!(result.matches, ["visible.rs"]);
        }
    }
}

#[tokio::test]
async fn test_grep_context_handles_crlf_and_unterminated_last_line() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "before\r\nneedle\r\nafter")
        .await
        .expect("test operation should succeed");
    for (context, expected) in
        [(0, vec!["a.rs:needle"]), (1, vec!["a.rs-before", "a.rs:needle", "a.rs-after"])]
    {
        let mut args = grep_args("needle");
        args.output_mode = OutputMode::Content;
        args.show_line_numbers = false;
        args.context = context;
        let result = tool(tmp.path())
            .call(&mut ToolContext::new(), args)
            .await
            .expect("test operation should succeed");
        assert_eq!(result.matches, expected);
    }
}

#[tokio::test]
async fn test_grep_counts_matching_lines_not_occurrences() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "needle needle\nneedle")
        .await
        .expect("test operation should succeed");
    let mut args = grep_args("needle");
    args.output_mode = OutputMode::Count;
    let result = tool(tmp.path())
        .call(&mut ToolContext::new(), args)
        .await
        .expect("test operation should succeed");
    assert_eq!(result.matches, ["a.rs:2"]);
}

#[tokio::test]
async fn test_grep_reports_invalid_patterns() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    let result = tool(tmp.path()).call(&mut ToolContext::new(), grep_args("(")).await;
    assert!(matches!(result, Err(GrepError::Regex { pattern: _, source: _ })));
    let mut args = grep_args("needle");
    args.include = Some("[".to_owned());
    let result = tool(tmp.path()).call(&mut ToolContext::new(), args).await;
    assert!(matches!(result, Err(GrepError::IncludeGlob { pattern: _, source: _ })));
}

#[tokio::test]
async fn test_grep_reports_invalid_ignore_rules() {
    let tmp = tempfile::TempDir::new().expect("test operation should succeed");
    tokio::fs::write(tmp.path().join(".ignore"), "[z-a]\n")
        .await
        .expect("test operation should succeed");
    tokio::fs::write(tmp.path().join("a.rs"), "needle\n")
        .await
        .expect("test operation should succeed");
    let result = tool(tmp.path()).call(&mut ToolContext::new(), grep_args("needle")).await;
    let error = result.expect_err("broken ignore rules must not be silently discarded");
    assert!(error.to_string().contains(".ignore"));

    let mut args = grep_args("needle");
    args.respect_ignore = false;
    let result = tool(tmp.path())
        .call(&mut ToolContext::new(), args)
        .await
        .expect("disabled ignore rules must not affect the search");
    assert_eq!(result.matches, ["a.rs"]);
}

fn set_modified(path: &Path, seconds: u64) {
    std::fs::File::open(path)
        .expect("test file should exist")
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
        .expect("test modification time should be writable");
}
