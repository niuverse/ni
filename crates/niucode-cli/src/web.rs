//! Local web surface for a session log.
//!
//! Loopback only. `/api/events` is a chunked SSE tail of the append-only log.
//! `POST /api/prompt` runs one read-only, approval-free agent turn; browser
//! approvals and streaming deltas are later slices (ADR-0036).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use niucode_agent::{project_history, run_with_history, AgentConfig, ApprovalMode};
use niucode_core::{load, now_ms, Event, SessionLog};
use niucode_provider::Provider;
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy};
use pulldown_cmark::{html, Options as MarkdownOptions, Parser};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

use crate::provider_factory::ProviderFactory;

const INDEX: &str = include_str!("../assets/web.html");

pub struct Options {
    pub session: PathBuf,
    pub port: u16,
    pub preset: String,
    pub model: Option<String>,
    pub cwd: PathBuf,
    pub max_steps: usize,
    /// Override the agent system prompt; used by niuse's product shell.
    pub system_prompt: Option<String>,
    /// Product brand shown in the page header.
    pub brand: Option<String>,
    /// Optional product context: title, brand, and sidebar sections.
    pub context: Option<Value>,
}

struct State {
    path: PathBuf,
    snapshot: Vec<u8>,
    provider: Option<Arc<dyn Provider>>,
    preset: String,
    model: String,
    cwd: PathBuf,
    max_steps: usize,
    system_prompt: Option<String>,
    context: Vec<u8>,
    brand: String,
    busy: AtomicBool,
    live: broadcast::Sender<Value>,
    pending: Mutex<HashMap<String, mpsc::Sender<bool>>>,
    next_approval: AtomicU64,
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
    let events = loaded
        .events
        .iter()
        .map(|event| {
            let mut value = serde_json::to_value(event).unwrap_or(Value::Null);
            add_html(&mut value);
            value
        })
        .collect::<Vec<_>>();
    let snapshot = serde_json::to_vec(&json!({
        "id": path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("session"),
        "path": path.display().to_string(),
        "warnings": loaded.warnings,
        "events": events,
    }))?;
    let (live, _) = broadcast::channel(256);
    let listener = TcpListener::bind(("127.0.0.1", options.port))
        .await
        .context("bind 127.0.0.1")?;
    let address = listener.local_addr()?;
    println!(
        "web: http://{address}/ · {} events · workspace-write + ask · Ctrl-C to stop",
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
            system_prompt: options.system_prompt,
            context: serde_json::to_vec(&options.context.unwrap_or_else(
                || json!({"brand": options.brand.clone().unwrap_or_else(|| "niucode".into())}),
            ))?,
            brand: options.brand.unwrap_or_else(|| "niucode".into()),
            busy: AtomicBool::new(false),
            live,
            pending: Mutex::new(HashMap::new()),
            next_approval: AtomicU64::new(1),
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
        return stream_events(stream, state.path.clone(), state.live.subscribe()).await;
    }
    if method == "POST" && path == "/api/prompt" {
        return prompt(stream, state, body).await;
    }
    if method == "GET" && path == "/api/context" {
        return send(
            &mut stream,
            "200 OK",
            "application/json",
            state.context.clone(),
        )
        .await;
    }
    if method == "GET" && path == "/api/status" {
        let status = json!({
            "brand": state.brand,
            "provider": state.preset,
            "model": state.model,
            "cwd": state.cwd.display().to_string(),
            "sandbox": "workspace-write",
            "approval": "ask",
            "network": "off",
            "busy": state.busy.load(Ordering::Relaxed),
        });
        return send(
            &mut stream,
            "200 OK",
            "application/json",
            serde_json::to_vec(&status)?,
        )
        .await;
    }
    if method == "POST" && path == "/api/approval" {
        return approval(stream, state, body).await;
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

async fn approval(mut stream: TcpStream, state: Arc<State>, body: &str) -> Result<()> {
    let value = serde_json::from_str::<Value>(body).unwrap_or(Value::Null);
    let id = value.get("id").and_then(Value::as_str).unwrap_or("");
    let allowed = value
        .get("allowed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let sender = state.pending.lock().unwrap().remove(id);
    match sender {
        Some(sender) => {
            let _ = sender.send(allowed);
            send(
                &mut stream,
                "200 OK",
                "application/json",
                br#"{"ok":true}"#.to_vec(),
            )
            .await
        }
        None => {
            send(
                &mut stream,
                "404 Not Found",
                "application/json",
                br#"{"error":"unknown approval"}"#.to_vec(),
            )
            .await
        }
    }
}

async fn run_prompt(state: Arc<State>, provider: Arc<dyn Provider>, prompt: String) -> Result<()> {
    let live = state.live.clone();
    let approval_state = state.clone();
    let loaded = load(&state.path)?;
    let history = project_history(&loaded.events)?;
    let mut log = SessionLog::open(&state.path)?;
    let mut config = AgentConfig::new(state.model.clone(), state.cwd.clone());
    config.provider = state.preset.clone();
    config.max_steps = state.max_steps;
    config.sandbox = SandboxPolicy::new(SandboxMode::WorkspaceWrite, NetworkMode::Off);
    config.approval = ApprovalMode::Ask;
    config.allow_unsandboxed = false;
    if let Some(prompt) = &state.system_prompt {
        config.system_prompt = prompt.clone();
    }
    let mut approver = |request: &niucode_agent::ApprovalRequest| {
        let id = format!(
            "a-{}",
            approval_state.next_approval.fetch_add(1, Ordering::SeqCst)
        );
        let (sender, receiver) = mpsc::channel();
        approval_state
            .pending
            .lock()
            .unwrap()
            .insert(id.clone(), sender);
        let _ = approval_state.live.send(json!({
            "type": "approval",
            "id": id,
            "tool": request.tool,
            "summary": request.summary,
            "preview": request.preview,
        }));
        let allowed = receiver
            .recv_timeout(Duration::from_secs(300))
            .unwrap_or(false);
        approval_state.pending.lock().unwrap().remove(&id);
        allowed
    };
    let mut observer = move |event: niucode_agent::AgentEvent| {
        let _ = live.send(crate::event_json(&event));
    };
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

async fn stream_events(
    mut stream: TcpStream,
    path: PathBuf,
    mut live: broadcast::Receiver<Value>,
) -> Result<()> {
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-store\r\nConnection: keep-alive\r\nX-Content-Type-Options: nosniff\r\nTransfer-Encoding: chunked\r\n\r\n";
    stream.write_all(head.as_bytes()).await?;
    let mut offset = 0_u64;
    let mut partial = String::new();
    let mut tick = tokio::time::interval(Duration::from_millis(350));
    loop {
        tokio::select! {
            _ = tick.tick() => {
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
            event = live.recv() => {
                if let Ok(value) = event {
                    write_chunk(&mut stream, &format!("data: {value}\n\n")).await?;
                }
            }
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

fn add_html(value: &mut Value) {
    if value.get("type").and_then(Value::as_str) != Some("assistant_message") {
        return;
    }
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        value["html"] = Value::String(markdown_html(text));
    }
}

fn markdown_html(text: &str) -> String {
    let (text, math) = prepare_math(text);
    let mut options = MarkdownOptions::empty();
    options.insert(MarkdownOptions::ENABLE_TABLES);
    options.insert(MarkdownOptions::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(&text, options).map(|event| match event {
        pulldown_cmark::Event::Html(raw) => pulldown_cmark::Event::Text(raw),
        pulldown_cmark::Event::InlineHtml(raw) => pulldown_cmark::Event::Text(raw),
        event => event,
    });
    let mut output = String::new();
    html::push_html(&mut output, parser);
    for (index, rendered) in math.into_iter().enumerate() {
        output = output.replace(&format!("@@NIUMATH{index}@@"), &rendered);
    }
    output
        .replace(
            "href=\"javascript:",
            "href=\"#\" data-blocked=\"javascript:",
        )
        .replace("<a href=", "<a rel=\"noreferrer noopener\" href=")
}

fn prepare_math(text: &str) -> (String, Vec<String>) {
    let mut rendered = Vec::new();
    let mut output = String::new();
    let mut code = false;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            code = !code;
            output.push_str(line);
            continue;
        }
        if code {
            output.push_str(line);
            continue;
        }
        let mut index = 0;
        let chars: Vec<char> = line.chars().collect();
        while index < chars.len() {
            if chars[index] == '\\' && index + 1 < chars.len() {
                output.push(chars[index]);
                output.push(chars[index + 1]);
                index += 2;
                continue;
            }
            if chars[index] != '$' {
                output.push(chars[index]);
                index += 1;
                continue;
            }
            let display = chars.get(index + 1) == Some(&'$');
            let marker = if display { "$$" } else { "$" };
            let offset = marker.len();
            if let Some(end) = line[index + offset..].find(marker) {
                let end = index + offset + end;
                let expression: String = chars[index + offset..end].iter().collect();
                if !expression.trim().is_empty() && !expression.contains('\n') {
                    let style = if display {
                        latex2mathml::DisplayStyle::Block
                    } else {
                        latex2mathml::DisplayStyle::Inline
                    };
                    let value = latex2mathml::latex_to_mathml(expression.trim(), style)
                        .unwrap_or_else(|_| {
                            format!(
                                "<code class=\"math\">{}</code>",
                                escape_html(expression.trim())
                            )
                        });
                    rendered.push(value);
                    output.push_str(&format!("@@NIUMATH{}@@", rendered.len() - 1));
                    index = end + marker.len();
                    continue;
                }
            }
            output.push('$');
            index += 1;
        }
    }
    (output, rendered)
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

    fn state_with(context: Vec<u8>) -> Arc<State> {
        Arc::new(State {
            path: PathBuf::new(),
            snapshot: br#"{"events":[]}"#.to_vec(),
            provider: None,
            preset: "deepseek".into(),
            model: "deepseek-flash".into(),
            cwd: PathBuf::from("."),
            max_steps: 1,
            system_prompt: None,
            context,
            brand: "niucode".into(),
            busy: AtomicBool::new(false),
            live: broadcast::channel(8).0,
            pending: Mutex::new(HashMap::new()),
            next_approval: AtomicU64::new(1),
        })
    }

    fn state() -> Arc<State> {
        state_with(br#"{}"#.to_vec())
    }

    #[test]
    fn markdown_html_escapes_raw_html_and_renders_tables() {
        let html = markdown_html(
            "**bold**\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n<script>alert(1)</script>",
        );
        assert!(html.contains("<strong>bold</strong>"), "{html}");
        assert!(html.contains("<table>"), "{html}");
        assert!(html.contains("<td>1</td>"), "{html}");
        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
    }

    #[test]
    fn math_is_rendered_as_mathml() {
        let html = markdown_html("energy $E = mc^2$ and\\n\\n$$\\n\\alpha + \\beta\\n$$");
        assert!(html.contains("<math"), "{html}");
        assert!(!html.contains('$') || html.contains("energy"), "{html}");
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
    async fn serves_product_context_over_loopback() {
        let state = state_with(
            br#"{"brand":"niuse","sections":[{"title":"identity","lines":["persona"]}]}"#.to_vec(),
        );
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(serve_listener(listener, state));
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream
            .write_all(b"GET /api/context HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).await.unwrap();
        let text = String::from_utf8_lossy(&reply);
        assert!(text.starts_with("HTTP/1.1 200"), "{text}");
        assert!(text.contains("\"brand\":\"niuse\""), "{text}");
        assert!(text.contains("persona"), "{text}");
        task.abort();
    }

    #[tokio::test]
    async fn serves_model_and_policy_status() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(serve_listener(listener, state()));
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream
            .write_all(b"GET /api/status HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).await.unwrap();
        let text = String::from_utf8_lossy(&reply);
        assert!(text.starts_with("HTTP/1.1 200"), "{text}");
        assert!(text.contains("\"model\":\"deepseek-flash\""), "{text}");
        assert!(text.contains("\"sandbox\":\"workspace-write\""), "{text}");
        assert!(text.contains("\"approval\":\"ask\""), "{text}");
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
                system_prompt: None,
                context: br#"{}"#.to_vec(),
                brand: "niucode".into(),
                busy: AtomicBool::new(false),
                live: broadcast::channel(8).0,
                pending: Mutex::new(HashMap::new()),
                next_approval: AtomicU64::new(1),
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
                system_prompt: None,
                context: br#"{}"#.to_vec(),
                brand: "niucode".into(),
                busy: AtomicBool::new(false),
                live: broadcast::channel(8).0,
                pending: Mutex::new(HashMap::new()),
                next_approval: AtomicU64::new(1),
            }),
        ));
        let mut events = TcpStream::connect(address).await.unwrap();
        events
            .write_all(b"GET /api/events HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut headers = [0_u8; 512];
        let _ = events.read(&mut headers).await.unwrap();
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
        let mut live = Vec::new();
        let delta = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let mut chunk = [0_u8; 1024];
                let count = events.read(&mut chunk).await.unwrap();
                live.extend_from_slice(&chunk[..count]);
                if String::from_utf8_lossy(&live).contains("text_delta") {
                    return;
                }
            }
        })
        .await;
        delta.expect("text delta over SSE");
        assert!(String::from_utf8_lossy(&live).contains("\"type\":\"phase\""));
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

    #[tokio::test]
    async fn approval_post_releases_a_pending_turn() {
        let state = state();
        let (sender, receiver) = mpsc::channel();
        state.pending.lock().unwrap().insert("a-1".into(), sender);
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(serve_listener(listener, state));
        let mut stream = TcpStream::connect(address).await.unwrap();
        let body = br#"{"id":"a-1","allowed":true}"#;
        stream
            .write_all(
                format!(
                    "POST /api/approval HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
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
            String::from_utf8_lossy(&reply).starts_with("HTTP/1.1 200"),
            "{}",
            String::from_utf8_lossy(&reply)
        );
        assert!(receiver.recv_timeout(Duration::from_secs(1)).unwrap());
        task.abort();
    }

    #[tokio::test]
    async fn browser_approval_allows_a_write_turn() {
        use niucode_provider::{ScriptedProvider, StreamEvent, ToolCallDelta};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.jsonl");
        std::fs::write(&path, "").unwrap();
        let provider: Arc<dyn Provider> = Arc::new(ScriptedProvider::from_turns(vec![
            vec![
                StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("write-1".into()),
                    name: Some("write".into()),
                    arguments: serde_json::json!({"path":"approved.txt","content":"yes"})
                        .to_string(),
                }),
                StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                },
            ],
            vec![
                StreamEvent::TextDelta("written".into()),
                StreamEvent::Done {
                    finish_reason: Some("stop".into()),
                },
            ],
        ]));
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(serve_listener(
            listener,
            Arc::new(State {
                path: path.clone(),
                snapshot: Vec::new(),
                provider: Some(provider),
                preset: "scripted".into(),
                model: "scripted".into(),
                cwd: directory.path().to_path_buf(),
                max_steps: 2,
                system_prompt: None,
                context: br#"{}"#.to_vec(),
                brand: "niucode".into(),
                busy: AtomicBool::new(false),
                live: broadcast::channel(8).0,
                pending: Mutex::new(HashMap::new()),
                next_approval: AtomicU64::new(1),
            }),
        ));
        let mut events = TcpStream::connect(address).await.unwrap();
        events
            .write_all(b"GET /api/events HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut headers = [0_u8; 512];
        let _ = events.read(&mut headers).await.unwrap();
        let mut prompt = TcpStream::connect(address).await.unwrap();
        let body = br#"{"prompt":"write it"}"#;
        prompt.write_all(format!("POST /api/prompt HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes()).await.unwrap();
        prompt.write_all(body).await.unwrap();
        let mut reply = Vec::new();
        prompt.read_to_end(&mut reply).await.unwrap();
        assert!(String::from_utf8_lossy(&reply).starts_with("HTTP/1.1 202"));
        let mut seen = Vec::new();
        let approval = "a-1".to_string();
        let seen_approval = tokio::time::timeout(Duration::from_secs(4), async {
            loop {
                let mut chunk = [0_u8; 1024];
                let count = events.read(&mut chunk).await.unwrap();
                seen.extend_from_slice(&chunk[..count]);
                if String::from_utf8_lossy(&seen).contains("\"type\":\"approval\"") {
                    return;
                }
            }
        })
        .await;
        seen_approval.expect("approval event");
        let mut approve = TcpStream::connect(address).await.unwrap();
        let body = format!(r#"{{"id":"{approval}","allowed":true}}"#);
        approve.write_all(format!("POST /api/approval HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes()).await.unwrap();
        approve.write_all(body.as_bytes()).await.unwrap();
        let mut reply = Vec::new();
        approve.read_to_end(&mut reply).await.unwrap();
        assert!(
            String::from_utf8_lossy(&reply).starts_with("HTTP/1.1 200"),
            "{}",
            String::from_utf8_lossy(&reply)
        );
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if std::fs::read_to_string(directory.path().join("approved.txt"))
                .ok()
                .as_deref()
                == Some("yes")
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "approved write missing"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        task.abort();
    }

    #[test]
    fn index_uses_sse_and_has_no_remote_assets() {
        assert!(INDEX.contains("EventSource('/api/events')"));
        assert!(INDEX.contains("Read-only local chat"));
        assert!(INDEX.contains("fetch('/api/prompt'"));
        assert!(INDEX.contains("text_delta"));
        assert!(INDEX.contains("renderApproval"));
        assert!(INDEX.contains("e.html"));
        assert!(INDEX.contains("math"));
        assert!(INDEX.contains("/api/context"));
        assert!(INDEX.contains("/api/status"));
        assert!(INDEX.contains("beginTurn"));
        assert!(INDEX.contains("toggle-context"));
        assert!(!INDEX.contains("http://"));
        assert!(!INDEX.contains("https://"));
    }
}
