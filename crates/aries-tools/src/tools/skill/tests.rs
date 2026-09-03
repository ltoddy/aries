// This file contains tests generated with AI assistance.

use std::fs;

use aries_extension::{SkillDefinition, SkillFrontmatter};
use rig::tool::{Tool, ToolContext};
use tempfile::TempDir;

use super::*;

#[tokio::test]
async fn test_args_title() {
    let args = SkillArgs { name: "test-skill".to_owned() };
    assert_eq!(args.title(), "Load skill test-skill");
}

/// 构造一个位于临时目录中的技能：`<tmp>/<name>/SKILL.md`。
fn make_skill(tmp: &TempDir, name: &str, frontmatter: SkillFrontmatter) -> SkillDefinition {
    let dir = tmp.path().join(name);
    fs::create_dir_all(&dir).unwrap();
    let location = dir.join("SKILL.md");
    fs::write(&location, "skill body").unwrap();

    SkillDefinition::new(location, frontmatter, "skill body")
}

#[tokio::test]
async fn test_call_loads_skill() {
    let tmp = TempDir::new().unwrap();
    let skill = make_skill(&tmp, "commit", SkillFrontmatter::new("commit", "desc"));
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
    let skill = make_skill(&tmp, "commit", SkillFrontmatter::new("commit", "desc"));
    let mut context = ToolContext::new();
    let tool = SkillTool::new(vec![skill]);

    let err = tool.call(&mut context, SkillArgs { name: "nope".to_owned() }).await.unwrap_err();
    assert!(matches!(err, SkillError::NotAllowed { .. }));
}

#[tokio::test]
async fn test_call_omits_allowed_tools_when_absent() {
    let tmp = TempDir::new().unwrap();
    let skill = make_skill(&tmp, "commit", SkillFrontmatter::new("commit", "desc"));
    let mut context = ToolContext::new();
    let tool = SkillTool::new(vec![skill]);

    let output = tool.call(&mut context, SkillArgs { name: "commit".to_owned() }).await.unwrap();
    assert!(!output.output.contains("Allowed tools for this skill"));
}
