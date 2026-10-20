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

#[tokio::test]
async fn extension_network_is_off_by_default() {
    let dir = temp_dir("network");
    fs::write(
        dir.join(".niucode/extensions/net.toml"),
        r#"
name = "net"
command = "python3"
args = ["sidecar.py"]
timeout_secs = 10
"#,
    )
    .expect("manifest");
    fs::write(
        dir.join("sidecar.py"),
        r#"import json, socket, sys
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {"name": "net", "version": "0.1.0", "tools": [{
            "name": "net_probe", "description": "Probe network",
            "input_schema": {"type": "object", "properties": {}}
        }]}
    elif method == "tools/call":
        try:
            socket.create_connection(("1.1.1.1", 80), timeout=1).close()
            result = {"ok": True, "output": "network available"}
        except OSError as error:
            result = {"ok": False, "output": "network blocked: " + type(error).__name__}
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
    assert!(report.host.mounts()[0].read_only);
    assert!(report.host.mounts()[0].capabilities.is_empty());
    let result = report.host.execute("net_probe", &json!({})).await;
    assert!(
        !result.ok,
        "network unexpectedly available: {}",
        result.output
    );
    assert!(
        result.output.contains("network blocked"),
        "{}",
        result.output
    );
    report.host.shutdown().await;
    fs::remove_dir_all(dir).expect("cleanup");
}

#[tokio::test]
async fn worker_pool_uses_more_than_one_sidecar() {
    let dir = temp_dir("pool");
    fs::write(
        dir.join(".niucode/extensions/pool.toml"),
        r#"
name = "pool"
command = "python3"
args = ["sidecar.py"]
timeout_secs = 10
workers = 2
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
        result = {"name": "pool", "version": "0.1.0", "tools": [{
            "name": "pool_pid", "description": "Return worker pid",
            "input_schema": {"type": "object", "properties": {}}
        }]}
    elif method == "tools/call":
        result = {"ok": True, "output": os.environ.get("NIUCODE_EXTENSION_WORKER", "?")}
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
    assert_eq!(report.host.mounts()[0].workers, 2);
    let args = json!({});
    let (left, right) = tokio::join!(
        report.host.execute("pool_pid", &args),
        report.host.execute("pool_pid", &args)
    );
    assert!(left.ok && right.ok, "{} / {}", left.output, right.output);
    assert_ne!(left.output, right.output, "worker pool reused one process");
    report.host.shutdown().await;
    fs::remove_dir_all(dir).expect("cleanup");
}

#[tokio::test]
async fn fs_write_requires_the_capability() {
    let dir = temp_dir("fswrite");
    let sidecar = r#"import json, sys
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if method == "initialize":
        result = {"name": "write", "version": "0.1.0", "tools": [{
            "name": "write_probe", "description": "Write probe",
            "input_schema": {"type": "object", "properties": {}}
        }]}
    elif method == "tools/call":
        try:
            with open("probe.txt", "w") as handle:
                handle.write("ok")
            result = {"ok": True, "output": "wrote"}
        except OSError as error:
            result = {"ok": False, "output": "write blocked: " + type(error).__name__}
    elif method == "shutdown":
        sys.exit(0)
    else:
        result = {"ok": False, "output": "unexpected"}
    print(json.dumps({"jsonrpc": "2.0", "id": request.get("id"), "result": result}), flush=True)
"#;
    fs::write(dir.join("sidecar.py"), sidecar).expect("sidecar");
    let read_only_manifest = r#"
name = "write"
command = "python3"
args = ["sidecar.py"]
timeout_secs = 10
"#;
    fs::write(
        dir.join(".niucode/extensions/write.toml"),
        read_only_manifest,
    )
    .expect("manifest");
    let report = discover(&dir, &[]).await;
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let blocked = report.host.execute("write_probe", &json!({})).await;
    assert!(
        !blocked.ok,
        "read-only extension wrote a file: {}",
        blocked.output
    );
    report.host.shutdown().await;

    fs::write(
        dir.join(".niucode/extensions/write.toml"),
        r#"
name = "write"
command = "python3"
args = ["sidecar.py"]
timeout_secs = 10
capabilities = ["fs_write"]
"#,
    )
    .expect("manifest");
    let report = discover(&dir, &[]).await;
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert_eq!(report.host.mounts()[0].capabilities, ["fs_write"]);
    let allowed = report.host.execute("write_probe", &json!({})).await;
    assert!(
        allowed.ok,
        "fs_write capability was not applied: {}",
        allowed.output
    );
    report.host.shutdown().await;
    fs::remove_dir_all(dir).expect("cleanup");
}
