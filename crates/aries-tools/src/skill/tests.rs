use super::*;

#[tokio::test]
async fn test_args_title() {
    let args = SkillArgs { name: "test-skill".to_owned() };
    assert_eq!(args.title(), "Load skill test-skill");
}
