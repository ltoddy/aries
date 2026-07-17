use rig_core::tool::Tool;

use super::*;

#[tokio::test]
async fn test_args_title() {
    let args = UpdatePlanArgs {
        items: vec![PlanEntry {
            content: "Fix bug".to_owned(),
            priority: PlanEntryPriority::High,
            status: PlanEntryStatus::Pending,
        }],
    };
    assert_eq!(args.title(), "Update plan with 1 items");
}

#[tokio::test]
async fn test_empty_plan() {
    let args = UpdatePlanArgs { items: vec![] };
    assert_eq!(args.title(), "Clear plan");
}

#[tokio::test]
async fn test_call_sends_update() {

    let tool = UpdatePlanTool::new(|_items| Ok(()));
    let args = UpdatePlanArgs {
        items: vec![PlanEntry {
            content: "Test".to_owned(),
            priority: PlanEntryPriority::Medium,
            status: PlanEntryStatus::InProgress,
        }],
    };
    let output = tool.call(args).await.unwrap();
    assert!(output.ok);
}
