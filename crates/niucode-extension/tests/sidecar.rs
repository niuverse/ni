use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use niucode_extension::discover;
use serde_json::json;

fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "niucode-ext-{tag}-{stamp}-{}",
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join(".niucode/extensions")).expect("dirs");
    dir
}

#[tokio::test]
async fn mounts_a_python_sidecar_and_calls_its_tool() {
    let dir = temp_dir("python");
    fs::write(
        dir.join(".niucode/extensions/demo.toml"),
        r#"
name = "demo"
command = "python3"
args = ["sidecar.py"]
timeout_secs = 10
"#,
    )
    .expect("manifest");
    fs::write(
        dir.join("sidecar.py"),
        r#"import json, sys
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {
            "name": "demo",
            "version": "0.1.0",
            "tools": [{
                "name": "demo_echo",
                "description": "Echo text from the demo extension.",
                "input_schema": {
                    "type": "object",
                    "properties": {"text": {"type": "string"}},
                    "required": ["text"]
                }
            }]
        }
    elif method == "tools/call":
        text = request.get("params", {}).get("arguments", {}).get("text", "")
        result = {"ok": True, "output": "echo: " + text}
    elif method == "shutdown":
        sys.exit(0)
    else:
        result = {"ok": False, "output": "unknown method"}
    print(json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}), flush=True)
"#,
    )
    .expect("sidecar");

    let report = discover(&dir, &["read".into(), "write".into()]).await;
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let host = report.host;
    assert_eq!(host.mounts().len(), 1);
    assert_eq!(host.mounts()[0].name, "demo");
    assert_eq!(host.mounts()[0].version, "0.1.0");
    assert!(host.has_tool("demo_echo"));

    let tool = host
        .specs()
        .into_iter()
        .find(|tool| tool.function.name == "demo_echo")
        .expect("demo tool");
    assert!(tool.function.description.contains("Echo text"));

    let result = host.execute("demo_echo", &json!({"text": "hi"})).await;
    assert!(result.ok, "{}", result.output);
    assert_eq!(result.output, "echo: hi");

    host.shutdown().await;
    fs::remove_dir_all(dir).expect("cleanup");
}

#[tokio::test]
async fn rejects_a_tool_name_that_collides_with_a_builtin() {
    let dir = temp_dir("collision");
    fs::write(
        dir.join(".niucode/extensions/demo.toml"),
        r#"
name = "demo"
command = "python3"
args = ["sidecar.py"]
"#,
    )
    .expect("manifest");
    fs::write(
        dir.join("sidecar.py"),
        r#"import json, sys
for line in sys.stdin:
    request = json.loads(line)
    if request.get("method") == "initialize":
        result = {"name": "demo", "version": "0.1.0", "tools": [{
            "name": "read",
            "description": "Collides with a builtin.",
            "input_schema": {"type": "object", "properties": {}}
        }]}
    elif request.get("method") == "shutdown":
        sys.exit(0)
    else:
        result = {"ok": False, "output": "unexpected"}
    print(json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}), flush=True)
"#,
    )
    .expect("sidecar");

    let report = discover(&dir, &["read".into()]).await;
    assert!(report.host.is_empty());
    assert!(report.errors.iter().any(|error| error.contains("collides")));
    report.host.shutdown().await;
    fs::remove_dir_all(dir).expect("cleanup");
}

#[tokio::test]
async fn sidecars_do_not_inherit_credentials() {
    let dir = temp_dir("env");
    std::env::set_var("NIUCODE_EXT_TEST_SECRET", "should-not-leak");
    fs::write(
        dir.join(".niucode/extensions/env.toml"),
        r#"
name = "env"
command = "python3"
args = ["sidecar.py"]
"#,
    )
    .expect("manifest");
    fs::write(
        dir.join("sidecar.py"),
        r#"import json, os, sys
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {"name": "env", "version": "0.1.0", "tools": [{
            "name": "env_check",
            "description": "Report whether a credential-like env var leaked.",
            "input_schema": {"type": "object", "properties": {}}
        }]}
    elif method == "tools/call":
        result = {"ok": True, "output": os.environ.get("NIUCODE_EXT_TEST_SECRET", "missing")}
    elif method == "shutdown":
        sys.exit(0)
    else:
        result = {"ok": False, "output": "unexpected"}
    print(json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}), flush=True)
"#,
    )
    .expect("sidecar");

    let report = discover(&dir, &[]).await;
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let result = report.host.execute("env_check", &json!({})).await;
    assert!(result.ok, "{}", result.output);
    assert_eq!(result.output, "missing");

    report.host.shutdown().await;
    fs::remove_dir_all(dir).expect("cleanup");
}

#[tokio::test]
async fn a_timed_out_tool_returns_an_error_instead_of_panicking() {
    let dir = temp_dir("timeout");
    fs::write(
        dir.join(".niucode/extensions/slow.toml"),
        r#"
name = "slow"
command = "python3"
args = ["sidecar.py"]
timeout_secs = 1
"#,
    )
    .expect("manifest");
    fs::write(
        dir.join("sidecar.py"),
        r#"import json, sys, time
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {"name": "slow", "version": "0.1.0", "tools": [{
            "name": "slow_tool",
            "description": "Sleeps longer than the timeout.",
            "input_schema": {"type": "object", "properties": {}}
        }]}
    elif method == "tools/call":
        time.sleep(3)
        result = {"ok": True, "output": "too late"}
    elif method == "shutdown":
        sys.exit(0)
    else:
        result = {"ok": False, "output": "unexpected"}
    print(json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}), flush=True)
"#,
    )
    .expect("sidecar");

    let report = discover(&dir, &[]).await;
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let result = report.host.execute("slow_tool", &json!({})).await;
    assert!(!result.ok);
    assert!(result.output.contains("timed out"), "{}", result.output);

    report.host.shutdown().await;
    fs::remove_dir_all(dir).expect("cleanup");
}
