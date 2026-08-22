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
        license: None,
        compatibility: None,
        metadata: None,
        allowed_tools: None,
    }
}

/// 在 `root/.agents/skills/` 下写入一个 SKILL.md，返回其路径。
fn write_skill(root: &Path, name: &str, description: &str) -> PathBuf {
    let dir = root.join(".agents").join("skills");
    fs::create_dir_all(&dir).unwrap();
    let location = dir.join("SKILL.md");
    let content = format!("---\nname: {name}\ndescription: {description}\n---\nbody of {name}\n");
    fs::write(&location, content).unwrap();
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
    let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(fm.name, "fix-typo");
    assert_eq!(fm.description, "fix typos in the codebase");
    assert_eq!(fm.license.as_deref(), Some("MIT"));
    assert_eq!(fm.compatibility.as_deref(), Some("any"));
    assert_eq!(fm.allowed_tools.as_deref(), Some(&["Read".to_owned(), "Edit".to_owned()][..]));
}

#[test]
fn deserializes_with_optional_fields_absent() {
    let yaml = "\
name: fix-typo
description: fix typos in the codebase
";
    let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap();
    assert!(fm.license.is_none());
    assert!(fm.compatibility.is_none());
    assert!(fm.metadata.is_none());
    assert!(fm.allowed_tools.is_none());
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
    let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap();
    let metadata = fm.metadata.expect("metadata should be parsed");
    assert_eq!(metadata["tags"].as_sequence().unwrap().len(), 2);
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
async fn load_finds_skills_from_home_and_cwd() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

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
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

    write_skill(&home, "fix-typo", "fix typos");
    let dir = cwd.join(".agents").join("skills");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("notes.md"), "---\nname: ignored\ndescription: ignored\n---\n").unwrap();

    let loader = SkillsLoader::new(&cwd, &home);
    let skills = loader.load().await;
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].frontmatter.name, "fix-typo");
}

#[tokio::test]
async fn load_returns_empty_when_no_roots_exist() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

    let loader = SkillsLoader::new(&cwd, &home);
    let skills = loader.load().await;
    assert!(skills.is_empty());
}
