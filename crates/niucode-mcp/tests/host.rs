//! Host-level tests: multiple servers, namespaced specs, and collisions.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use niucode_mcp::{McpHost, McpServerSpec};
use serde_json::json;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixture_server.py")
}

fn spec(name: &str) -> McpServerSpec {
    let mut env = BTreeMap::new();
    env.insert(
        "PATH".to_string(),
        std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string()),
    );
    McpServerSpec {
        name: name.to_string(),
        command: "python3".to_string(),
        args: vec![fixture().display().to_string()],
        env,
        read_only: true,
    }
}

#[tokio::test]
async fn host_mounts_two_servers_and_dispatches_namespaced_tools() {
    let limit = Duration::from_secs(5);
    let host = McpHost::mount(&[spec("a"), spec("b")], limit)
        .await
        .expect("mount");
    let names: Vec<String> = host
        .specs()
        .into_iter()
        .map(|spec| spec.function.name)
        .collect();
    assert!(names.contains(&"mcp__a__echo".to_string()));
    assert!(names.contains(&"mcp__b__echo".to_string()));
    assert_eq!(host.mounts().len(), 2);
    assert_eq!(host.mounts()[0].server, "a");
    assert_eq!(host.mounts()[0].tools.len(), 4);

    let call = host
        .call("mcp__b__echo", json!({"text": "from b"}), limit)
        .await
        .expect("call");
    assert!(call.ok);
    assert_eq!(call.output, "from b");
    host.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn duplicate_server_names_fail_before_mounting_second() {
    let error = match McpHost::mount(&[spec("same"), spec("same")], Duration::from_secs(5)).await {
        Ok(_) => panic!("duplicate names must fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("collision"), "{error}");
}
