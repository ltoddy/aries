// This file contains tests generated with AI assistance.

use std::fs;
use std::path::{Path, PathBuf};

use itertools::Itertools;
use tempfile::TempDir;

use super::*;
use crate::tool::ToolList;

fn frontmatter() -> Frontmatter {
    Frontmatter {
        name: "fix-typo".to_owned(),
        description: "fix typos in the codebase".to_owned(),
        license: None,
        compatibility: None,
        metadata: None,
        allowed_tools: ToolList::default(),
    }
}

/// 在 `root/.agents/skills/` 下写入一个 SKILL.md，返回其路径。
fn write_skill(root: &Path, name: &str, description: &str) -> PathBuf {
    let dir = root.join(".agents").join("skills");
    fs::create_dir_all(&dir).expect("test operation should succeed");
    let location = dir.join("SKILL.md");
    let content = format!("---\nname: {name}\ndescription: {description}\n---\nbody of {name}\n");
    fs::write(&location, content).expect("test operation should succeed");
    location
}

#[test]
fn deserializes_kebab_case_allowed_tools() {
    let yaml = "\
name: fix-typo
description: fix typos in the codebase
license: MIT
compatibility: any
allowed-tools:
  - Read
  - Edit
";
    let fm: Frontmatter = serde_yaml::from_str(yaml).expect("test operation should succeed");
    assert_eq!(fm.name, "fix-typo");
    assert_eq!(fm.description, "fix typos in the codebase");
    assert_eq!(fm.license.as_deref(), Some("MIT"));
    assert_eq!(fm.compatibility.as_deref(), Some("any"));
    assert_eq!(fm.allowed_tools.as_slice(), &["Read".to_owned(), "Edit".to_owned()]);
}

#[test]
fn deserializes_string_allowed_tools() {
    let yaml = "\
name: fix-typo
description: fix typos in the codebase
allowed-tools: Read, Edit
";
    let fm: Frontmatter = serde_yaml::from_str(yaml).expect("test operation should succeed");
    assert_eq!(fm.allowed_tools.as_slice(), &["Read".to_owned(), "Edit".to_owned()]);
}

#[test]
fn deserializes_with_optional_fields_absent() {
    let yaml = "\
name: fix-typo
description: fix typos in the codebase
";
    let fm: Frontmatter = serde_yaml::from_str(yaml).expect("test operation should succeed");
    assert!(fm.license.is_none());
    assert!(fm.compatibility.is_none());
    assert!(fm.metadata.is_none());
    assert!(fm.allowed_tools.is_empty());
}

#[test]
fn deserializes_metadata() {
    let yaml = "\
name: fix-typo
description: fix typos
metadata:
  tags: [rust, cli]
  license: MIT
";
    let fm: Frontmatter = serde_yaml::from_str(yaml).expect("test operation should succeed");
    let metadata = fm.metadata.expect("metadata should be parsed");
    assert_eq!(metadata["tags"].as_sequence().expect("test operation should succeed").len(), 2);
    assert_eq!(metadata["license"].as_str(), Some("MIT"));
}

#[test]
fn render_formats_skill_xml() {
    let rendered = frontmatter().render("/path/to/SKILL.md");
    assert!(rendered.contains("<skill>"));
    assert!(rendered.contains("<name>fix-typo</name>"));
    assert!(rendered.contains("<description>fix typos in the codebase</description>"));
    assert!(rendered.contains("<location>/path/to/SKILL.md</location>"));
    assert!(rendered.contains("</skill>"));
}

#[test]
fn new_stores_location_and_body() {
    let def = SkillDefinition::new("/tmp/SKILL.md", frontmatter(), "body text");
    assert_eq!(def.location, Path::new("/tmp/SKILL.md"));
    assert_eq!(def.body, "body text");
    assert_eq!(def.frontmatter.name, "fix-typo");
}

#[tokio::test]
async fn executor_expands_skill_content() {
    let tmp = TempDir::new().expect("test operation should succeed");
    let dir = tmp.path().join("skill");
    fs::create_dir_all(&dir).expect("test operation should succeed");
    let location = dir.join("SKILL.md");
    fs::write(&location, "body $ARGUMENTS from ${SKILL_DIR}")
        .expect("test operation should succeed");
    fs::write(dir.join("script.sh"), "#!/bin/sh").expect("test operation should succeed");

    let skill = SkillDefinition::new(&location, frontmatter(), "body $ARGUMENTS from ${SKILL_DIR}");
    let output = SkillExecutor::new(&[skill])
        .execute("fix-typo requested changes")
        .await
        .expect("skill should execute");

    assert!(output.contains(r#"<skill_content name="fix-typo">"#));
    assert!(output.contains("# Skill: fix-typo"));
    assert!(output.contains(&format!("body requested changes from {}", dir.display())));
    assert!(output.contains("Base directory for this skill: file://"));
    assert!(output.contains(&format!("<file>{}</file>", location.display())));
    assert!(output.contains(&format!("<file>{}</file>", dir.join("script.sh").display())));
    assert!(output.contains("</skill_content>"));
}

#[tokio::test]
async fn executor_trims_input_before_matching() {
    let tmp = TempDir::new().expect("test operation should succeed");
    let location = tmp.path().join("SKILL.md");
    fs::write(&location, "body").expect("test operation should succeed");
    let skill = SkillDefinition::new(&location, frontmatter(), "args:$ARGUMENTS");
    let output = SkillExecutor::new(&[skill])
        .execute("  fix-typo file.rs  ")
        .await
        .expect("skill should execute");

    assert!(output.contains("args:file.rs"));
}

#[tokio::test]
async fn executor_includes_allowed_tools_when_present() {
    let tmp = TempDir::new().expect("test operation should succeed");
    let location = tmp.path().join("SKILL.md");
    fs::write(&location, "body").expect("test operation should succeed");
    let mut frontmatter = frontmatter();
    frontmatter.allowed_tools = vec!["Read".to_owned(), "Edit".to_owned()].into();
    let skill = SkillDefinition::new(&location, frontmatter, "body");
    let output =
        SkillExecutor::new(&[skill]).execute("fix-typo").await.expect("skill should execute");

    assert!(output.contains("Allowed tools for this skill: Read, Edit"));
}

#[tokio::test]
async fn executor_omits_allowed_tools_when_empty() {
    let tmp = TempDir::new().expect("test operation should succeed");
    let location = tmp.path().join("SKILL.md");
    fs::write(&location, "body").expect("test operation should succeed");
    let skill = SkillDefinition::new(&location, frontmatter(), "body");
    let output =
        SkillExecutor::new(&[skill]).execute("fix-typo").await.expect("skill should execute");

    assert!(!output.contains("Allowed tools for this skill:"));
}

#[tokio::test]
async fn executor_returns_none_when_skill_location_has_no_parent() {
    let location = PathBuf::from(std::path::MAIN_SEPARATOR.to_string());
    let skill = SkillDefinition::new(location, frontmatter(), "body");
    let output = SkillExecutor::new(&[skill]).execute("fix-typo").await;

    assert!(output.is_none());
}

#[tokio::test]
async fn executor_uses_first_matching_skill() {
    let tmp = TempDir::new().expect("test operation should succeed");
    let first_location = tmp.path().join("first").join("SKILL.md");
    let second_location = tmp.path().join("second").join("SKILL.md");
    fs::create_dir_all(first_location.parent().expect("test path should have parent"))
        .expect("test operation should succeed");
    fs::create_dir_all(second_location.parent().expect("test path should have parent"))
        .expect("test operation should succeed");
    fs::write(&first_location, "first").expect("test operation should succeed");
    fs::write(&second_location, "second").expect("test operation should succeed");
    let first = SkillDefinition::new(&first_location, frontmatter(), "first");
    let second = SkillDefinition::new(&second_location, frontmatter(), "second");
    let output = SkillExecutor::new(&[first, second])
        .execute("fix-typo")
        .await
        .expect("skill should execute");

    assert!(output.contains("\nfirst\n"));
    assert!(!output.contains("\nsecond\n"));
}

#[tokio::test]
async fn executor_returns_none_for_unknown_skill() {
    let skill = SkillDefinition::new("/tmp/SKILL.md", frontmatter(), "body");
    let output = SkillExecutor::new(&[skill]).execute("review").await;

    assert!(output.is_none());
}

#[tokio::test]
async fn load_finds_skills_from_home_and_cwd() {
    let tmp = TempDir::new().expect("test operation should succeed");
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).expect("test operation should succeed");
    fs::create_dir_all(&cwd).expect("test operation should succeed");

    write_skill(&home, "fix-typo", "fix typos");
    write_skill(&cwd, "review", "review code");

    let loader = SkillsLoader::new(&cwd, &home);
    let skills = loader.load().await;

    let names: Vec<_> = skills.iter().map(|s| s.frontmatter.name.as_str()).sorted().collect_vec();
    assert_eq!(names, vec!["fix-typo", "review"]);
    assert!(skills.iter().all(|s| s.body.contains("body of")));
}

#[tokio::test]
async fn load_ignores_non_skill_files() {
    let tmp = TempDir::new().expect("test operation should succeed");
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).expect("test operation should succeed");
    fs::create_dir_all(&cwd).expect("test operation should succeed");

    write_skill(&home, "fix-typo", "fix typos");
    let dir = cwd.join(".agents").join("skills");
    fs::create_dir_all(&dir).expect("test operation should succeed");
    fs::write(dir.join("notes.md"), "---\nname: ignored\ndescription: ignored\n---\n")
        .expect("test operation should succeed");

    let loader = SkillsLoader::new(&cwd, &home);
    let skills = loader.load().await;
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].frontmatter.name, "fix-typo");
}

#[tokio::test]
async fn load_returns_empty_when_no_roots_exist() {
    let tmp = TempDir::new().expect("test operation should succeed");
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).expect("test operation should succeed");
    fs::create_dir_all(&cwd).expect("test operation should succeed");

    let loader = SkillsLoader::new(&cwd, &home);
    let skills = loader.load().await;
    assert!(skills.is_empty());
}
