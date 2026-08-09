use serde_json::json;

use super::tavily::TavilySearchRequest;
use super::*;

#[test]
fn test_render_args_parses_query_and_domains() {
    let raw = json!({
        "query": "latest React docs",
        "num": 10,
        "allowed_domains": ["react.dev"],
        "blocked_domains": ["example.com"]
    })
    .to_string();

    let (first, second) = WebSearchArgs::render_args(&raw).unwrap();
    assert_eq!(first, "latest React docs");
    assert_eq!(second, None);
}

#[test]
fn test_render_args_minimal() {
    let raw = json!({ "query": "rust async" }).to_string();
    let (first, _) = WebSearchArgs::render_args(&raw).unwrap();
    assert_eq!(first, "rust async");
}

#[test]
fn test_render_args_missing_query_fails() {
    let raw = json!({}).to_string();
    assert!(WebSearchArgs::render_args(&raw).is_err());
}

#[test]
fn test_render_output_formats_results_as_markdown() {
    let output = json!({
        "query": "rust async",
        "results": [
            {
                "title": "Async Book",
                "url": "https://rust-lang.github.io/async-book/",
                "description": "An online version of the Async Book."
            }
        ],
        "duration_seconds": 0.5
    });

    let rendered = WebSearchOutput::render_output(output).unwrap();
    assert!(rendered.contains("Query: rust async"));
    assert!(rendered.contains("1. [Async Book](https://rust-lang.github.io/async-book/)"));
    assert!(rendered.contains("An online version of the Async Book."));
}

#[test]
fn test_render_output_empty_results() {
    let output = json!({
        "query": "nothing found",
        "results": [],
        "duration_seconds": 0.3
    });

    let rendered = WebSearchOutput::render_output(output).unwrap();
    assert!(rendered.contains("No search results found"));
}

#[test]
fn test_args_round_trip_with_domains() {
    let args = WebSearchArgs {
        query: "test".to_owned(),
        num: Some(7),
        allowed_domains: Some(vec!["a.com".to_owned()]),
        blocked_domains: None,
    };

    let serialized = serde_json::to_string(&args).unwrap();
    let deserialized: WebSearchArgs = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.query, "test");
    assert_eq!(deserialized.num, Some(7));
    assert_eq!(deserialized.allowed_domains, Some(vec!["a.com".to_owned()]));
    assert_eq!(deserialized.blocked_domains, None);
}

#[test]
fn test_tavily_request_serializes_domains() {
    let request = TavilySearchRequest {
        query: "rust".to_owned(),
        max_results: 15,
        include_answer: false,
        include_domains: Some(vec!["rust-lang.org".to_owned()]),
        exclude_domains: Some(vec!["reddit.com".to_owned()]),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["query"], "rust");
    assert_eq!(json["max_results"], 15);
    assert_eq!(json["include_answer"], false);
    assert_eq!(json["include_domains"], json!(["rust-lang.org"]));
    assert_eq!(json["exclude_domains"], json!(["reddit.com"]));
}

#[test]
fn test_tavily_request_omits_empty_domains() {
    let request = TavilySearchRequest {
        query: "rust".to_owned(),
        max_results: 15,
        include_answer: false,
        include_domains: None,
        exclude_domains: None,
    };

    let json = serde_json::to_value(&request).unwrap();
    assert!(json.get("include_domains").is_none());
    assert!(json.get("exclude_domains").is_none());
}
