// This file contains tests generated with AI assistance.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::*;

fn stdio_config(command: &str) -> McpServerConfig {
    McpServerConfig::stdio(command, vec![], HashMap::new())
}

/// 在 `root/.agents/mcps/` 下写入一个 mcp.json，包含名为 `name` 的 server。
fn write_mcp_json(root: &Path, name: &str) {
    let dir = root.join(".agents").join("mcps");
    fs::create_dir_all(&dir).unwrap();
    let content =
        format!(r#"{{"mcpServers": {{"{name}": {{"type": "stdio", "command": "echo"}}}}}}"#);
    fs::write(dir.join("mcp.json"), content).unwrap();
}

#[test]
fn deserializes_mcp_definition() {
    let json = r#"{
        "mcpServers": {
            "fs": {
                "type": "stdio",
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-filesystem"],
                "env": {"FOO": "bar"}
            },
            "remote": {
                "type": "http",
                "url": "https://example.com/mcp",
                "headers": {"Authorization": "Bearer token"}
            }
        }
    }"#;
    let def: McpDefinition = serde_json::from_str(json).unwrap();
    assert_eq!(def.mcp_servers.len(), 2);

    match def.mcp_servers.get("fs") {
        Some(McpServerConfig::Stdio(s)) => {
            assert_eq!(s.command, "npx");
            assert_eq!(s.args, vec!["-y", "@modelcontextprotocol/server-filesystem"]);
            assert_eq!(s.env.get("FOO").map(String::as_str), Some("bar"));
        },
        _ => panic!("expected stdio config"),
    }

    match def.mcp_servers.get("remote") {
        Some(McpServerConfig::Http(h)) => {
            assert_eq!(h.url, "https://example.com/mcp");
            assert_eq!(h.headers.get("Authorization").map(String::as_str), Some("Bearer token"));
        },
        _ => panic!("expected http config"),
    }
}

#[test]
fn deserializes_sse_config() {
    let json = r#"{"mcpServers": {"s": {"type": "sse", "url": "https://example.com/sse"}}}"#;
    let def: McpDefinition = serde_json::from_str(json).unwrap();
    match def.mcp_servers.get("s") {
        Some(McpServerConfig::Sse(s)) => assert_eq!(s.url, "https://example.com/sse"),
        _ => panic!("expected sse config"),
    }
}

#[test]
fn deserializes_with_default_empty_fields() {
    let json = r#"{"mcpServers": {"s": {"type": "stdio", "command": "echo"}}}"#;
    let def: McpDefinition = serde_json::from_str(json).unwrap();
    match def.mcp_servers.get("s") {
        Some(McpServerConfig::Stdio(s)) => {
            assert!(s.args.is_empty());
            assert!(s.env.is_empty());
        },
        _ => panic!("expected stdio config"),
    }
}

#[test]
fn empty_and_new() {
    assert!(McpDefinition::empty().mcp_servers.is_empty());

    let mut map = HashMap::new();
    map.insert("s".to_owned(), stdio_config("echo"));
    let def = McpDefinition::new(map);
    assert_eq!(def.mcp_servers.len(), 1);
}

#[test]
fn update_merges_servers() {
    let mut a = McpDefinition::empty();
    a.mcp_servers.insert("a".to_owned(), stdio_config("echo"));
    let b = McpDefinition { mcp_servers: HashMap::from([("b".to_owned(), stdio_config("cat"))]) };

    a.update(b);
    assert_eq!(a.mcp_servers.len(), 2);
    assert!(a.mcp_servers.contains_key("a"));
    assert!(a.mcp_servers.contains_key("b"));
}

#[test]
fn config_constructors() {
    let stdio = McpServerConfig::stdio("echo", vec!["hi".to_owned()], HashMap::new());
    match stdio {
        McpServerConfig::Stdio(s) => assert_eq!(s.args, vec!["hi"]),
        _ => panic!("expected stdio"),
    }

    let sse = McpServerConfig::sse("https://example.com", HashMap::new());
    assert!(matches!(sse, McpServerConfig::Sse(_)));

    let http = McpServerConfig::http("https://example.com", HashMap::new());
    assert!(matches!(http, McpServerConfig::Http(_)));
}

#[tokio::test]
async fn parse_reads_file() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("mcp.json");
    fs::write(&file, r#"{"mcpServers": {"s": {"type": "stdio", "command": "echo"}}}"#).unwrap();

    let def = McpDefinition::parse(&file).await.unwrap();
    assert_eq!(def.mcp_servers.len(), 1);
}

#[tokio::test]
async fn parse_reports_error_for_missing_file() {
    let err = McpDefinition::parse("/nonexistent/mcp.json").await.unwrap_err();
    assert!(matches!(err, McpParseError::Io(_)));
}

#[tokio::test]
async fn load_finds_mcps_from_home_and_cwd() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

    write_mcp_json(&home, "home-server");
    write_mcp_json(&cwd, "cwd-server");

    let loader = McpsLoader::new(&cwd, &home);
    let mcps = loader.load().await;
    assert_eq!(mcps.len(), 2);
    assert!(mcps.iter().any(|m| m.mcp_servers.contains_key("home-server")));
    assert!(mcps.iter().any(|m| m.mcp_servers.contains_key("cwd-server")));
}

#[tokio::test]
async fn load_ignores_non_mcp_json() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

    write_mcp_json(&home, "valid");
    let dir = cwd.join(".agents").join("mcps");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("other.json"), r#"{"mcpServers": {}}"#).unwrap();

    let loader = McpsLoader::new(&cwd, &home);
    let mcps = loader.load().await;
    assert_eq!(mcps.len(), 1);
}

#[tokio::test]
async fn load_returns_empty_when_no_roots_exist() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&cwd).unwrap();

    let loader = McpsLoader::new(&cwd, &home);
    let mcps = loader.load().await;
    assert!(mcps.is_empty());
}
