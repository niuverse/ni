//! Offline integration tests for the MCP stdio client.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use niucode_mcp::{exposed_name, McpClient, McpServerSpec};
use serde_json::json;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixture_server.py")
}

fn spec() -> McpServerSpec {
    let mut env = BTreeMap::new();
    env.insert("ALLOWED".to_string(), "yes".to_string());
    env.insert(
        "PATH".to_string(),
        std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string()),
    );
    McpServerSpec {
        name: "fixture".to_string(),
        command: "python3".to_string(),
        args: vec![fixture().display().to_string()],
        env,
        read_only: true,
    }
}

#[tokio::test]
async fn mount_list_call_and_scrub_environment() {
    let limit = Duration::from_secs(5);
    let mut client = McpClient::connect(&spec(), limit).await.expect("mount");
    let tools = client.list_tools(limit).await.expect("list");
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
    assert!(names.contains(&"echo"));
    assert!(names.contains(&"hang"));

    let echo = client
        .call_tool("echo", json!({"text": "hello"}), limit)
        .await
        .expect("echo");
    assert!(echo.ok);
    assert_eq!(echo.output, "hello");

    // The parent's LEAK variable is not in the manifest, so it is scrubbed.
    let env = client
        .call_tool("env", json!({}), limit)
        .await
        .expect("env");
    assert_eq!(env.output, "|yes");

    let failed = client
        .call_tool("fail", json!({}), limit)
        .await
        .expect("tool error is a result");
    assert!(!failed.ok);
    assert_eq!(failed.output, "boom");

    assert_eq!(exposed_name("fixture", "echo"), "mcp__fixture__echo");
    client.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_hung_tool_times_out_and_kills_the_server() {
    let limit = Duration::from_secs(5);
    let mut client = McpClient::connect(&spec(), limit).await.expect("mount");
    let error = client
        .call_tool("hang", json!({}), Duration::from_millis(200))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("timed out"), "{error}");
    let _ = client.shutdown().await;
}

#[tokio::test]
async fn a_writable_server_is_refused_before_spawn() {
    let mut server = spec();
    server.read_only = false;
    let error = match McpClient::connect(&server, Duration::from_secs(1)).await {
        Ok(_) => panic!("a writable server must not spawn"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("read_only"), "{error}");
}
