// This file contains tests generated with AI assistance.

use std::collections::HashSet;

use super::*;

fn frontmatter(tools: Option<&[&str]>, disallowed: Option<&[&str]>) -> Frontmatter {
    Frontmatter {
        name: "reviewer".to_owned(),
        description: "review code".to_owned(),
        tools: tools.map(|t| t.iter().map(|s| s.to_string()).collect()),
        disallowed_tools: disallowed.map(|t| t.iter().map(|s| s.to_string()).collect()),
        model: None,
    }
}

const ALL: &[&str] = &["Read", "Write", "Edit", "Bash"];

#[test]
fn tools_description_no_restrictions() {
    assert_eq!(frontmatter(None, None).tools_description(), "All tools");
}

#[test]
fn tools_description_allowlist_only() {
    assert_eq!(frontmatter(Some(&["Read", "Grep"]), None).tools_description(), "Read, Grep");
}

#[test]
fn tools_description_denylist_only() {
    assert_eq!(
        frontmatter(None, Some(&["Write", "Edit"])).tools_description(),
        "All tools except Write, Edit"
    );
}

#[test]
fn tools_description_allow_and_deny() {
    assert_eq!(frontmatter(Some(&["Read", "Write"]), Some(&["Write"])).tools_description(), "Read");
}

#[test]
fn tools_description_deny_empties_allowlist() {
    assert_eq!(frontmatter(Some(&["Write"]), Some(&["Write"])).tools_description(), "None");
}

#[test]
fn resolve_no_restrictions_inherits_all() {
    let fm = frontmatter(None, None);
    let result: HashSet<_> = fm.filter_tool_names(ALL).into_iter().collect();
    let expected: HashSet<_> = ALL.iter().copied().collect();
    assert_eq!(result, expected);
}

#[test]
fn resolve_allowlist_only() {
    let fm = frontmatter(Some(&["Read", "Bash"]), None);
    let result: HashSet<_> = fm.filter_tool_names(ALL).into_iter().collect();
    assert_eq!(result, HashSet::from(["Read", "Bash"]));
}

#[test]
fn resolve_denylist_filters_universe() {
    let fm = frontmatter(None, Some(&["Write", "Edit"]));
    let result: HashSet<_> = fm.filter_tool_names(ALL).into_iter().collect();
    let expected: HashSet<_> = ["Read", "Bash"].into();
    assert_eq!(result, expected);
}

#[test]
fn resolve_denylist_overrides_allowlist() {
    assert_eq!(
        frontmatter(Some(&["Read", "Write"]), Some(&["Write"])).filter_tool_names(ALL),
        vec!["Read"]
    );
}

#[test]
fn deserializes_kebab_case_disallowed_tools() {
    let yaml = "\
name: reviewer
description: review code
tools:
  - Read
disallowed-tools:
  - Write
model: sonnet
";
    let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(fm.tools.as_deref(), Some(&["Read".to_owned()][..]));
    assert_eq!(fm.disallowed_tools.as_deref(), Some(&["Write".to_owned()][..]));
    assert_eq!(fm.model.as_deref(), Some("sonnet"));
}

#[test]
fn deserializes_with_optional_fields_absent() {
    let yaml = "\
name: reviewer
description: review code
";
    let fm: Frontmatter = serde_yaml::from_str(yaml).unwrap();
    assert!(fm.tools.is_none());
    assert!(fm.disallowed_tools.is_none());
    assert!(fm.model.is_none());
}
