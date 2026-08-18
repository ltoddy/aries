// This file contains tests generated with AI assistance.

use std::fs;

use aries_extension::skill::definition::{Frontmatter, SkillDefinition};
use rig::tool::{Tool, ToolContext};
use tempfile::TempDir;

use super::*;

#[tokio::test]
async fn test_args_title() {
    let args = SkillArgs { name: "test-skill".to_owned() };
    assert_eq!(args.title(), "Load skill test-skill");
}

/// 构造一个位于临时目录中的技能：`<tmp>/<name>/SKILL.md`。
fn make_skill(tmp: &TempDir, name: &str, allowed_tools: Option<Vec<String>>) -> SkillDefinition {
    let dir = tmp.path().join(name);
    fs::create_dir_all(&dir).unwrap();
    let location = dir.join("SKILL.md");
    fs::write(&location, "skill body").unwrap();

    let frontmatter = Frontmatter {
        name: name.to_owned(),
        description: "desc".to_owned(),
        license: None,
        compatibility: None,
        metadata: None,
        allowed_tools,
    };

    SkillDefinition::new(location, frontmatter, "skill body")
}

#[tokio::test]
async fn test_call_loads_skill() {
    let tmp = TempDir::new().unwrap();
    let skill = make_skill(&tmp, "commit", None);
    let mut context = ToolContext::new();
    let tool = SkillTool::new(vec![skill]);

    let output = tool.call(&mut context, SkillArgs { name: "commit".to_owned() }).await.unwrap();
    assert_eq!(output.metadata.name, "commit");
    assert!(output.output.contains("<skill_content name=\"commit\">"));
    assert!(output.output.contains("# Skill: commit"));
}

#[tokio::test]
async fn test_call_rejects_unknown_skill() {
    let tmp = TempDir::new().unwrap();
    let skill = make_skill(&tmp, "commit", None);
    let mut context = ToolContext::new();
    let tool = SkillTool::new(vec![skill]);

    let err = tool.call(&mut context, SkillArgs { name: "nope".to_owned() }).await.unwrap_err();
    assert!(matches!(err, SkillError::NotAllowed { .. }));
}

#[tokio::test]
async fn test_call_includes_allowed_tools() {
    let tmp = TempDir::new().unwrap();
    let skill = make_skill(&tmp, "review", Some(vec!["Read".to_owned(), "Grep".to_owned()]));
    let mut context = ToolContext::new();
    let tool = SkillTool::new(vec![skill]);

    let output = tool.call(&mut context, SkillArgs { name: "review".to_owned() }).await.unwrap();
    assert!(output.output.contains("Allowed tools for this skill: Read, Grep"));
}

#[tokio::test]
async fn test_call_omits_allowed_tools_when_absent() {
    let tmp = TempDir::new().unwrap();
    let skill = make_skill(&tmp, "commit", None);
    let mut context = ToolContext::new();
    let tool = SkillTool::new(vec![skill]);

    let output = tool.call(&mut context, SkillArgs { name: "commit".to_owned() }).await.unwrap();
    assert!(!output.output.contains("Allowed tools for this skill"));
}
