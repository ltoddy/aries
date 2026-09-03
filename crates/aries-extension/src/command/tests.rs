// This file contains tests generated with AI assistance.

use std::fs;
use std::path::{Path, PathBuf};

use itertools::Itertools;
use tempfile::TempDir;

use super::*;

fn frontmatter() -> Frontmatter {
    Frontmatter {
        name: "fix-typo".to_owned(),
        description: "fix typos in the codebase".to_owned(),
        argument_hint: None,
        allowed_tools: Vec::new().into(),
    }
}

/// 在 `root/.agents/commands/` 下写入一个命令文件，返回其路径。
fn write_command(root: &Path, name: &str, description: &str) -> PathBuf {
    let dir = root.join(".agents").join("commands");
    fs::create_dir_all(&dir).unwrap();
    let location = dir.join(format!("{name}.md"));
    let content = format!("---\nname: {name}\ndescription: {description}\n---\nbody of {name}\n");
    fs::write(&location, content).unwrap();
    location
}

#[test]
fn deserializes_kebab_case_fields() {
    let yaml = "\
name: fix-typo
description: fix typos in the codebase
argument-hint: <file>
allowed-tools:
  - Read
  - Edit
";
    let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(fm.name, "fix-typo");
    assert_eq!(fm.description, "fix typos in the codebase");
    assert_eq!(fm.argument_hint.as_deref(), Some("<file>"));
    assert_eq!(fm.allowed_tools.as_slice(), &["Read".to_owned(), "Edit".to_owned()]);
}

#[test]
fn deserializes_string_allowed_tools() {
    let yaml = "\
name: fix-typo
description: fix typos in the codebase
allowed-tools: Bash(openspec:*)
";
    let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(fm.allowed_tools.as_slice(), &["Bash(openspec:*)".to_owned()]);
}

#[test]
fn deserializes_with_optional_fields_absent() {
    let yaml = "\
name: fix-typo
description: fix typos in the codebase
";
    let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap();
    assert!(fm.argument_hint.is_none());
    assert!(fm.allowed_tools.is_empty());
}

#[test]
fn serializes_with_kebab_case_keys() {
    let mut fm = frontmatter();
    fm.argument_hint = Some("<file>".to_owned());
    fm.allowed_tools = vec!["Read".to_owned()].into();

    let yaml = serde_yaml::to_string(&fm).unwrap();
    assert!(yaml.contains("argument-hint: <file>"));
    assert!(yaml.contains("allowed-tools:"));
    assert!(!yaml.contains("argument_hint"));
    assert!(!yaml.contains("allowed_tools"));
}

#[test]
fn new_stores_location_and_body() {
    let def = CommandDefinition::new("/tmp/fix-typo.md", frontmatter(), "body text");
    assert_eq!(def.location(), Path::new("/tmp/fix-typo.md"));
    assert_eq!(def.body, "body text");
    assert_eq!(def.frontmatter.name, "fix-typo");
}

#[test]
fn expands_positional_arguments() {
    let def = CommandDefinition::new("/tmp/x.md", frontmatter(), "run $1 with $2 and $3");
    assert_eq!(def.expand_arguments("alpha beta gamma"), "run alpha with beta and gamma");
}

#[test]
fn expands_high_index_positional_arguments() {
    let def = CommandDefinition::new("/tmp/x.md", frontmatter(), "use $10");
    let arguments = "a b c d e f g h i j";
    assert_eq!(def.expand_arguments(arguments), "use j");
}

#[test]
fn expands_arguments_placeholder() {
    let def = CommandDefinition::new("/tmp/x.md", frontmatter(), "echo $ARGUMENTS");
    assert_eq!(def.expand_arguments("a b c"), "echo a b c");
}

#[test]
fn expands_positional_and_arguments_together() {
    let def = CommandDefinition::new("/tmp/x.md", frontmatter(), "$1 <- first, $ARGUMENTS <- all");
    assert_eq!(def.expand_arguments("a b"), "a <- first, a b <- all");
}

#[test]
fn missing_positional_arguments_expand_to_empty() {
    let def = CommandDefinition::new("/tmp/x.md", frontmatter(), "run $1 $2 $3");
    assert_eq!(def.expand_arguments("only"), "run only  ");
}

#[test]
fn empty_arguments_expand_to_empty() {
    let def = CommandDefinition::new("/tmp/x.md", frontmatter(), "run $1 with $ARGUMENTS");
    assert_eq!(def.expand_arguments(""), "run  with ");
}

#[test]
fn leaves_unknown_placeholders_intact() {
    let def = CommandDefinition::new("/tmp/x.md", frontmatter(), "keep $foo and $ and $1x");
    assert_eq!(def.expand_arguments("a"), "keep $foo and $ and ax");
}

#[test]
fn body_without_placeholders_is_unchanged() {
    let def = CommandDefinition::new("/tmp/x.md", frontmatter(), "plain text");
    assert_eq!(def.expand_arguments("args"), "plain text");
    assert_eq!(def.expand_arguments(""), "plain text");
}

#[test]
fn arguments_containing_placeholder_text_are_not_re_expanded() {
    let def = CommandDefinition::new("/tmp/x.md", frontmatter(), "echo $ARGUMENTS");
    assert_eq!(def.expand_arguments("$2 $ARGUMENTS"), "echo $2 $ARGUMENTS");
}

#[test]
fn adjacent_placeholders_expand_independently() {
    let def = CommandDefinition::new("/tmp/x.md", frontmatter(), "$1$2");
    assert_eq!(def.expand_arguments("a b"), "ab");
}

#[test]
fn zero_index_expands_to_empty() {
    let def = CommandDefinition::new("/tmp/x.md", frontmatter(), "run $0");
    assert_eq!(def.expand_arguments("a"), "run ");
}

#[test]
fn extra_whitespace_in_arguments_is_collapsed() {
    let def = CommandDefinition::new("/tmp/x.md", frontmatter(), "$1 $2 $3");
    assert_eq!(def.expand_arguments("  a\t b   c "), "a b c");
}

#[test]
fn placeholder_names_are_case_sensitive() {
    let def =
        CommandDefinition::new("/tmp/x.md", frontmatter(), "$arguments $Arguments $ARGUMENTS");
    assert_eq!(def.expand_arguments("x"), "$arguments $Arguments x");
}

#[tokio::test]
async fn load_finds_commands_from_home_and_cwd() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

    write_command(&home, "fix-typo", "fix typos");
    write_command(&cwd, "summarize", "summarize the diff");

    let loader = CommandsLoader::new(&cwd, &home);
    let commands = loader.load().await;

    let names: Vec<_> = commands.iter().map(|c| c.frontmatter.name.as_str()).sorted().collect_vec();
    assert_eq!(names, vec!["fix-typo", "summarize"]);
    assert!(commands.iter().all(|c| c.body.contains("body of")));
}

#[tokio::test]
async fn load_ignores_non_md_files() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

    write_command(&home, "valid", "desc");
    let dir = cwd.join(".agents").join("commands");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("notes.txt"), "---\nname: ignored\ndescription: ignored\n---\n").unwrap();

    let loader = CommandsLoader::new(&cwd, &home);
    let commands = loader.load().await;
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].frontmatter.name, "valid");
}

#[tokio::test]
async fn load_returns_empty_when_no_roots_exist() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

    let loader = CommandsLoader::new(&cwd, &home);
    let commands = loader.load().await;
    assert!(commands.is_empty());
}
