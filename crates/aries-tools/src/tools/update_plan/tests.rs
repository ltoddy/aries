// This file contains tests generated with AI assistance.

use rig_agent::tool::{Tool, ToolContext};

use super::*;

fn entry(content: &str, active_form: &str, status: PlanEntryStatus) -> PlanEntry {
    PlanEntry {
        content: content.to_owned(),
        active_form: active_form.to_owned(),
        priority: PlanEntryPriority::Medium,
        status,
    }
}

#[tokio::test]
async fn test_args_title() {
    let args = UpdatePlanArgs {
        items: vec![PlanEntry {
            content: "Fix bug".to_owned(),
            active_form: "Fixing bug".to_owned(),
            priority: PlanEntryPriority::High,
            status: PlanEntryStatus::Pending,
        }],
    };
    assert_eq!(args.title(), "Update plan with 1 items");
}

#[tokio::test]
async fn test_empty_plan_title() {
    let args = UpdatePlanArgs { items: vec![] };
    assert_eq!(args.title(), "Clear plan");
}

#[tokio::test]
async fn test_call_echoes_items() {
    let mut context = ToolContext::new();
    let tool = UpdatePlanTool::new();
    let args =
        UpdatePlanArgs { items: vec![entry("Test", "Testing", PlanEntryStatus::InProgress)] };
    let output = tool.call(&mut context, args).await.unwrap();
    assert_eq!(output.items.len(), 1);
    assert_eq!(output.items[0].active_form, "Testing");
}

#[tokio::test]
async fn test_call_clears_when_all_completed() {
    // 全部 completed 时应清空计划（对齐 OpenClaude allDone -> []）。
    let mut context = ToolContext::new();
    let tool = UpdatePlanTool::new();
    let args = UpdatePlanArgs {
        items: vec![
            entry("A", "Doing A", PlanEntryStatus::Completed),
            entry("B", "Doing B", PlanEntryStatus::Completed),
        ],
    };
    let output = tool.call(&mut context, args).await.unwrap();
    assert!(output.items.is_empty());
}

#[tokio::test]
async fn test_call_keeps_items_when_partially_done() {
    let mut context = ToolContext::new();
    let tool = UpdatePlanTool::new();
    let args = UpdatePlanArgs {
        items: vec![
            entry("A", "Doing A", PlanEntryStatus::Completed),
            entry("B", "Doing B", PlanEntryStatus::InProgress),
        ],
    };
    let output = tool.call(&mut context, args).await.unwrap();
    assert_eq!(output.items.len(), 2);
}

#[tokio::test]
async fn test_call_rejects_empty_content() {
    let mut context = ToolContext::new();
    let tool = UpdatePlanTool::new();
    let args = UpdatePlanArgs { items: vec![entry("   ", "Doing", PlanEntryStatus::Pending)] };
    assert!(tool.call(&mut context, args).await.is_err());
}

#[tokio::test]
async fn test_call_rejects_empty_active_form() {
    let mut context = ToolContext::new();
    let tool = UpdatePlanTool::new();
    let args = UpdatePlanArgs { items: vec![entry("Do it", "", PlanEntryStatus::Pending)] };
    assert!(tool.call(&mut context, args).await.is_err());
}
