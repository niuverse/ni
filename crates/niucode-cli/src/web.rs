//! Local web surface for a session log.
//!
//! Loopback only. `/api/events` is a chunked SSE tail of the append-only log.
//! `POST /api/prompt` runs one read-only, approval-free agent turn; browser
//! approvals and streaming deltas are later slices (ADR-0036).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use niucode_agent::{project_history, run_with_history, AgentConfig, ApprovalMode};
use niucode_core::{load, now_ms, Event, SessionLog};
use niucode_provider::Provider;
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::provider_factory::ProviderFactory;

const INDEX: &str = include_str!("../assets/web.html");

pub struct Options {
    pub session: PathBuf,
    pub port: u16,
    pub preset: String,
    pub model: Option<String>,
    pub cwd: PathBuf,
    pub max_steps: usize,
}

struct State {
    path: PathBuf,
    snapshot: Vec<u8>,
    provider: Option<Arc<dyn Provider>>,
    preset: String,
    model: String,
    cwd: PathBuf,
    max_steps: usize,
    busy: AtomicBool,
}

pub async fn serve(options: Options) -> Result<()> {
    let path = options.session;
    let model = match options.model {
        Some(model) => model,
        None => ProviderFactory::default_model(&options.preset)
            .unwrap_or_else(|_| "deepseek-flash".into()),
    };
    let provider = ProviderFactory::new(false, None)?
        .build(&options.preset, &model)
        .context("build web provider")?;
    if !path.exists() {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty());
        let log = match parent {
            Some(parent) => SessionLog::create(parent)?,
            None => SessionLog::create(Path::new("."))?,
        };
        log.append(&Event::SessionStart {
            id: log.id().to_string(),
            cwd: options.cwd.display().to_string(),
            model: model.clone(),
            provider: options.preset.clone(),
            ts: now_ms(),
        })?;
        log.append(&Event::Policy {
            sandbox: "read-only".into(),
            approval: "never".into(),
            network: "off".into(),
            backend: "web".into(),
            ts: now_ms(),
        })?;
        std::fs::rename(log.path(), &path)
            .with_context(|| format!("move session into {}", path.display()))?;
    }
    let loaded = load(&path).with_context(|| format!("load session {}", path.display()))?;
    let snapshot = serde_json::to_vec(&json!({
        "id": path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("session"),
        "path": path.display().to_string(),
        "warnings": loaded.warnings,
        "events": loaded.events,
    }))?;
    let listener = TcpListener::bind(("127.0.0.1", options.port))
        .await
        .context("bind 127.0.0.1")?;
    let address = listener.local_addr()?;
    println!(
        "web: http://{address}/ · {} events · read-only chat · Ctrl-C to stop",
        loaded.events.len()
    );
    serve_listener(
        listener,
        Arc::new(State {
            path,
            snapshot,
            provider: Some(provider),
            preset: options.preset,
            model,
            cwd: options.cwd,
            max_steps: options.max_steps,
            busy: AtomicBool::new(false),
        }),
    )
    .await
}

async fn serve_listener(listener: TcpListener, state: Arc<State>) -> Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        tokio::spawn(async move {
            let _ = handle(stream, state).await;
        });
    }
}

async fn handle(mut stream: TcpStream, state: Arc<State>) -> Result<()> {
    let mut buffer = [0_u8; 16384];
    let count = stream.read(&mut buffer).await?;
    let request = String::from_utf8_lossy(&buffer[..count]);
    let (head, body) = request.split_once("\r\n\r\n").unwrap_or((&request, ""));
    let mut parts = head.lines().next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");
    if method == "GET" && path == "/api/events" {
        return stream_events(stream, state.path.clone()).await;
    }
    if method == "POST" && path == "/api/prompt" {
        return prompt(stream, state, body).await;
    }
    let (status, kind, body) = response(method, path, &state.snapshot);
    send(&mut stream, status, kind, body).await
}

async fn prompt(mut stream: TcpStream, state: Arc<State>, body: &str) -> Result<()> {
    let Some(provider) = state.provider.clone() else {
        return send(
            &mut stream,
            "503 Service Unavailable",
            "application/json",
            br#"{"error":"no provider"}"#.to_vec(),
        )
        .await;
    };
    let prompt = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("prompt")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    if prompt.trim().is_empty() {
        return send(
            &mut stream,
            "400 Bad Request",
            "application/json",
            br#"{"error":"prompt is required"}"#.to_vec(),
        )
        .await;
    }
    if state.busy.swap(true, Ordering::SeqCst) {
        return send(
            &mut stream,
            "409 Conflict",
            "application/json",
            br#"{"error":"a turn is already running"}"#.to_vec(),
        )
        .await;
    }
    let turn = state.clone();
    // A web turn uses the callback engine, whose future is not `Send`; run it
    // on a dedicated current-thread runtime instead of the connection task.
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("web: runtime: {error:#}");
                turn.busy.store(false, Ordering::SeqCst);
                return;
            }
        };
        let result = runtime.block_on(run_prompt(turn.clone(), provider, prompt));
        turn.busy.store(false, Ordering::SeqCst);
        if let Err(error) = result {
            eprintln!("web: prompt failed: {error:#}");
        }
    });
    send(
        &mut stream,
        "202 Accepted",
        "application/json",
        br#"{"ok":true}"#.to_vec(),
    )
    .await
}

async fn run_prompt(state: Arc<State>, provider: Arc<dyn Provider>, prompt: String) -> Result<()> {
    let loaded = load(&state.path)?;
    let history = project_history(&loaded.events)?;
    let mut log = SessionLog::open(&state.path)?;
    let mut config = AgentConfig::new(state.model.clone(), state.cwd.clone());
    config.provider = state.preset.clone();
    config.max_steps = state.max_steps;
    config.sandbox = SandboxPolicy::new(SandboxMode::ReadOnly, NetworkMode::Off);
    config.approval = ApprovalMode::Never;
    config.allow_unsandboxed = false;
    let mut approver = |_request: &niucode_agent::ApprovalRequest| false;
    let mut observer = |_event| {};
    run_with_history(
        provider.as_ref(),
        &mut log,
        &config,
        &prompt,
        history,
        &mut approver,
        &mut observer,
    )
    .await?;
    Ok(())
}

async fn send(stream: &mut TcpStream, status: &str, kind: &str, body: Vec<u8>) -> Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.shutdown().await?;
    Ok(())
}

async fn stream_events(mut stream: TcpStream, path: PathBuf) -> Result<()> {
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-store\r\nConnection: keep-alive\r\nX-Content-Type-Options: nosniff\r\nTransfer-Encoding: chunked\r\n\r\n";
    stream.write_all(head.as_bytes()).await?;
    let mut offset = 0_u64;
    let mut partial = String::new();
    let mut tick = tokio::time::interval(Duration::from_millis(350));
    loop {
        tick.tick().await;
        let bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        if bytes.len() as u64 > offset {
            let fresh = &bytes[offset as usize..];
            offset = bytes.len() as u64;
            partial.push_str(&String::from_utf8_lossy(fresh));
            while let Some(index) = partial.find('\n') {
                let rest = partial.split_off(index + 1);
                let line = std::mem::replace(&mut partial, rest);
                let line = line.trim_end_matches('\r');
                if line.is_empty() {
                    continue;
                }
                write_chunk(&mut stream, &format!("data: {line}\n\n")).await?;
            }
        } else {
            write_chunk(&mut stream, ": ping\n\n").await?;
        }
    }
}

async fn write_chunk(stream: &mut TcpStream, text: &str) -> Result<()> {
    let head = format!("{:x}\r\n", text.len());
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(text.as_bytes()).await?;
    stream.write_all(b"\r\n").await?;
    Ok(())
}

fn response(method: &str, path: &str, snapshot: &[u8]) -> (&'static str, &'static str, Vec<u8>) {
    match (method, path) {
        ("GET", "/") => (
            "200 OK",
            "text/html; charset=utf-8",
            INDEX.as_bytes().to_vec(),
        ),
        ("GET", "/api/session") => ("200 OK", "application/json", snapshot.to_vec()),
        ("GET", "/api/health") => ("200 OK", "application/json", br#"{"ok":true}"#.to_vec()),
        ("POST", "/api/prompt") => (
            "501 Not Implemented",
            "application/json",
            br#"{"error":"interactive chat is not implemented in v0"}"#.to_vec(),
        ),
        _ => (
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"not found\n".to_vec(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn state() -> Arc<State> {
        Arc::new(State {
            path: PathBuf::new(),
            snapshot: br#"{"events":[]}"#.to_vec(),
            provider: None,
            preset: "deepseek".into(),
            model: "deepseek-flash".into(),
            cwd: PathBuf::from("."),
            max_steps: 1,
            busy: AtomicBool::new(false),
        })
    }

    #[test]
    fn routes_are_small_and_explicit() {
        let session = br#"{"events":[]}"#;
        assert_eq!(response("GET", "/api/health", session).0, "200 OK");
        assert_eq!(response("GET", "/api/session", session).2, session.to_vec());
        assert_eq!(response("GET", "/", session).1, "text/html; charset=utf-8");
        assert_eq!(response("GET", "/nope", session).0, "404 Not Found");
        assert_eq!(
            response("POST", "/api/prompt", session).0,
            "501 Not Implemented"
        );
    }

    #[tokio::test]
    async fn serves_a_session_over_loopback_http() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(serve_listener(listener, state()));
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream
            .write_all(b"GET /api/session HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).await.unwrap();
        let text = String::from_utf8_lossy(&reply);
        assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");
        assert!(text.contains("application/json"), "{text}");
        assert!(text.ends_with("{\"events\":[]}"), "{text}");
        task.abort();
    }

    #[tokio::test]
    async fn sse_streams_the_initial_log_and_new_events() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.jsonl");
        std::fs::write(&path, "{\"type\":\"session_start\",\"id\":\"one\"}\n").unwrap();
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(serve_listener(
            listener,
            Arc::new(State {
                path: path.clone(),
                snapshot: Vec::new(),
                provider: None,
                preset: "deepseek".into(),
                model: "deepseek-flash".into(),
                cwd: PathBuf::from("."),
                max_steps: 1,
                busy: AtomicBool::new(false),
            }),
        ));
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream
            .write_all(b"GET /api/events HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut seen = Vec::new();
        let first = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let mut chunk = [0_u8; 1024];
                let count = stream.read(&mut chunk).await.unwrap();
                seen.extend_from_slice(&chunk[..count]);
                if String::from_utf8_lossy(&seen).contains("session_start") {
                    return;
                }
            }
        })
        .await;
        first.expect("initial SSE event");
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"type\":\"assistant_message\",\"text\":\"two\"}\n")
            .unwrap();
        let second = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let mut chunk = [0_u8; 1024];
                let count = stream.read(&mut chunk).await.unwrap();
                seen.extend_from_slice(&chunk[..count]);
                if String::from_utf8_lossy(&seen).contains("assistant_message") {
                    return;
                }
            }
        })
        .await;
        second.expect("appended SSE event");
        task.abort();
    }

    #[tokio::test]
    async fn browser_prompt_runs_a_read_only_turn() {
        use niucode_provider::{ScriptedProvider, StreamEvent};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.jsonl");
        std::fs::write(&path, "").unwrap();
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let provider: Arc<dyn Provider> = Arc::new(ScriptedProvider::from_turns(vec![vec![
            StreamEvent::TextDelta("from the browser".into()),
            StreamEvent::Done {
                finish_reason: Some("stop".into()),
            },
        ]]));
        let task = tokio::spawn(serve_listener(
            listener,
            Arc::new(State {
                path: path.clone(),
                snapshot: Vec::new(),
                provider: Some(provider),
                preset: "scripted".into(),
                model: "scripted".into(),
                cwd: directory.path().to_path_buf(),
                max_steps: 1,
                busy: AtomicBool::new(false),
            }),
        ));
        let mut stream = TcpStream::connect(address).await.unwrap();
        let body = br#"{"prompt":"say hello"}"#;
        stream
            .write_all(
                format!(
                    "POST /api/prompt HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        stream.write_all(body).await.unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).await.unwrap();
        assert!(
            String::from_utf8_lossy(&reply).starts_with("HTTP/1.1 202"),
            "{}",
            String::from_utf8_lossy(&reply)
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            if text.contains("from the browser") {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "turn did not finish");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        task.abort();
    }

    #[test]
    fn index_uses_sse_and_has_no_remote_assets() {
        assert!(INDEX.contains("EventSource('/api/events')"));
        assert!(INDEX.contains("Read-only local chat"));
        assert!(INDEX.contains("fetch('/api/prompt'"));
        assert!(!INDEX.contains("http://"));
        assert!(!INDEX.contains("https://"));
    }
}
