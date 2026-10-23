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
    /// Optional live context. When set, `/api/context` calls it per request.
    pub context_provider: Option<Arc<dyn Fn() -> Value + Send + Sync>>,
    /// Allow `POST /api/new` to start a fresh session in the same directory.
    pub allow_new: bool,
    /// Optional embedder hook for product-specific new-session semantics.
    pub new_session_provider: Option<Arc<dyn Fn() -> Result<PathBuf> + Send + Sync>>,
}

struct State {
    path: Mutex<PathBuf>,
    snapshot: Mutex<Vec<u8>>,
    provider: Option<Arc<dyn Provider>>,
    preset: String,
    model: String,
    cwd: PathBuf,
    max_steps: usize,
    system_prompt: Option<String>,
    context: Vec<u8>,
    context_provider: Option<Arc<dyn Fn() -> Value + Send + Sync>>,
    brand: String,
    allow_new: bool,
    new_session_provider: Option<Arc<dyn Fn() -> Result<PathBuf> + Send + Sync>>,
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
            path: Mutex::new(path),
            snapshot: Mutex::new(snapshot),
            provider: Some(provider),
            preset: options.preset,
            model,
            cwd: options.cwd,
            max_steps: options.max_steps,
            system_prompt: options.system_prompt,
            context: serde_json::to_vec(&options.context.unwrap_or_else(
                || json!({"brand": options.brand.clone().unwrap_or_else(|| "niucode".into())}),
            ))?,
            context_provider: options.context_provider,
            brand: options.brand.unwrap_or_else(|| "niucode".into()),
            allow_new: options.allow_new,
            new_session_provider: options.new_session_provider,
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
    let current_path = state.path.lock().unwrap().clone();
    if method == "GET" && path == "/api/events" {
        return stream_events(stream, current_path, state.live.subscribe()).await;
    }
    if method == "POST" && path == "/api/prompt" {
        return prompt(stream, state, body).await;
    }
    if method == "GET" && path == "/api/context" {
        let body = if let Some(provider) = &state.context_provider {
            serde_json::to_vec(&provider())?
        } else {
            state.context.clone()
        };
        return send(&mut stream, "200 OK", "application/json", body).await;
    }
    if method == "GET" && path == "/api/export" {
        let markdown = match load(&current_path) {
            Ok(loaded) => export_markdown(&loaded.events),
            Err(error) => {
                return send(
                    &mut stream,
                    "500 Internal Server Error",
                    "application/json",
                    serde_json::to_vec(&json!({"error": error.to_string()}))?,
                )
                .await;
            }
        };
        return send_download(&mut stream, markdown).await;
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
            "allow_new": state.allow_new,
        });
        return send(
            &mut stream,
            "200 OK",
            "application/json",
            serde_json::to_vec(&status)?,
        )
        .await;
    }
    if method == "POST" && path == "/api/new" {
        return new_session(stream, state).await;
    }
    if method == "POST" && path == "/api/approval" {
        return approval(stream, state, body).await;
    }
    let snapshot = state.snapshot.lock().unwrap().clone();
    let (status, kind, body) = response(method, path, &snapshot);
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
        let _ = turn.live.send(json!({"type": "context_changed"}));
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

async fn new_session(mut stream: TcpStream, state: Arc<State>) -> Result<()> {
    if !state.allow_new {
        return send(
            &mut stream,
            "403 Forbidden",
            "application/json",
            br#"{"error":"new sessions are not enabled"}"#.to_vec(),
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
    let result = create_session(&state);
    state.busy.store(false, Ordering::SeqCst);
    match result {
        Ok(path) => {
            let body = serde_json::to_vec(&json!({
                "id": path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("session"),
                "path": path.display().to_string(),
            }))?;
            send(&mut stream, "200 OK", "application/json", body).await
        }
        Err(error) => {
            send(
                &mut stream,
                "500 Internal Server Error",
                "application/json",
                serde_json::to_vec(&json!({"error": error.to_string()}))?,
            )
            .await
        }
    }
}

fn create_session(state: &State) -> Result<PathBuf> {
    if let Some(provider) = &state.new_session_provider {
        let path = provider()?;
        *state.path.lock().unwrap() = path.clone();
        *state.snapshot.lock().unwrap() = serde_json::to_vec(&json!({
            "id": path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("session"),
            "path": path.display().to_string(),
            "warnings": [],
            "events": [],
        }))?;
        return Ok(path);
    }
    let current = state.path.lock().unwrap().clone();
    let parent = current
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let log = SessionLog::create(parent)?;
    let id = log.id().to_string();
    log.append(&Event::SessionStart {
        id: id.clone(),
        cwd: state.cwd.display().to_string(),
        model: state.model.clone(),
        provider: state.preset.clone(),
        ts: now_ms(),
    })?;
    log.append(&Event::Policy {
        sandbox: "workspace-write".into(),
        approval: "ask".into(),
        network: "off".into(),
        backend: "web".into(),
        ts: now_ms(),
    })?;
    let path = log.path().to_path_buf();
    drop(log);
    *state.path.lock().unwrap() = path.clone();
    *state.snapshot.lock().unwrap() = serde_json::to_vec(&json!({
        "id": id,
        "path": path.display().to_string(),
        "warnings": [],
        "events": [],
    }))?;
    Ok(path)
}

async fn run_prompt(state: Arc<State>, provider: Arc<dyn Provider>, prompt: String) -> Result<()> {
    let live = state.live.clone();
    let approval_state = state.clone();
    let path = state.path.lock().unwrap().clone();
    let loaded = load(&path)?;
    let history = project_history(&loaded.events)?;
    let mut log = SessionLog::open(&path)?;
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
            "preview": preview_json(request.preview.as_ref()),
        }));
        let allowed = receiver
            .recv_timeout(Duration::from_secs(300))
            .unwrap_or(false);
        approval_state.pending.lock().unwrap().remove(&id);
        allowed
    };
    let mut streamed = String::new();
    let mut observer = move |event: niucode_agent::AgentEvent| {
        match &event {
            niucode_agent::AgentEvent::TextDelta(text) => {
                streamed.push_str(text);
                // Local turns are short; re-rendering the accumulated text keeps
                // partial Markdown visible and is bounded by the turn size.
                let _ = live.send(json!({
                    "type": "text_delta",
                    "text": text,
                    "html": markdown_html(&streamed),
                }));
            }
            _ => {
                let _ = live.send(crate::event_json(&event));
            }
        }
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

fn preview_json(preview: Option<&niucode_tools::ChangePreview>) -> Value {
    let Some(preview) = preview else {
        return Value::Null;
    };
    let body = match &preview.body {
        niucode_tools::PreviewBody::Diff { files } => json!({
            "kind": "diff",
            "files": files.iter().map(|file| json!({
                "path": file.path,
                "is_new": file.is_new,
                "added": file.added,
                "removed": file.removed,
                "hunks": file.hunks.iter().map(|hunk| json!({
                    "old_start": hunk.old_start,
                    "old_len": hunk.old_len,
                    "new_start": hunk.new_start,
                    "new_len": hunk.new_len,
                    "lines": hunk.lines.iter().map(|line| json!({
                        "kind": match line.kind {
                            niucode_tools::LineKind::Context => "context",
                            niucode_tools::LineKind::Add => "add",
                            niucode_tools::LineKind::Remove => "remove",
                        },
                        "old_line": line.old_line,
                        "new_line": line.new_line,
                        "text": line.text,
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        }),
        niucode_tools::PreviewBody::Command { command } => json!({
            "kind": "command",
            "command": command,
        }),
        niucode_tools::PreviewBody::Note { message } => json!({
            "kind": "note",
            "message": message,
        }),
    };
    json!({
        "body": body,
        "added": preview.added,
        "removed": preview.removed,
        "warning": preview.warning,
    })
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
    let (text, code) = extract_code(text);
    let (text, math) = prepare_math(&text);
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
    for (index, rendered) in code.into_iter().enumerate() {
        let token = format!("@@NIUCODE{index}@@");
        output = output
            .replace(&format!("<p>{token}</p>"), &rendered)
            .replace(&token, &rendered);
    }
    output
        .replace(
            "href=\"javascript:",
            "href=\"#\" data-blocked=\"javascript:",
        )
        .replace("<a href=", "<a rel=\"noreferrer noopener\" href=")
}

fn extract_code(text: &str) -> (String, Vec<String>) {
    let mut output = String::new();
    let mut blocks: Vec<String> = Vec::new();
    let mut language = String::new();
    let mut body = String::new();
    let mut in_code = false;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if !in_code && trimmed.starts_with("```") {
            in_code = true;
            language = trimmed.trim_start_matches("```").trim().to_string();
            body.clear();
            blocks.push(String::new());
            output.push_str(&format!("\n@@NIUCODE{}@@\n", blocks.len() - 1));
            continue;
        }
        if in_code && trimmed.starts_with("```") {
            let rendered = render_code(&language, &body);
            let index = blocks.len() - 1;
            blocks[index] = rendered;
            in_code = false;
            continue;
        }
        if in_code {
            body.push_str(line);
        } else {
            output.push_str(line);
        }
    }
    if in_code {
        let rendered = render_code(&language, &body);
        let index = blocks.len() - 1;
        blocks[index] = rendered;
    }
    (output, blocks)
}

fn render_code(language: &str, body: &str) -> String {
    let language = language
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .take(24)
        .collect::<String>();
    let label = if language.is_empty() {
        "code".to_string()
    } else {
        language
    };
    let mut html = format!("<pre class=\"code\"><code class=\"language-{label}\">");
    let lines: Vec<&str> = body.split('\n').collect();
    let mut highlighter = crate::syntax::Highlighter::new();
    for (index, line) in lines.iter().enumerate() {
        for token in highlighter.line(line) {
            let class = match token.kind {
                crate::syntax::TokenKind::Keyword => "tok-keyword",
                crate::syntax::TokenKind::String => "tok-string",
                crate::syntax::TokenKind::Comment => "tok-comment",
                crate::syntax::TokenKind::Number => "tok-number",
                crate::syntax::TokenKind::Function => "tok-function",
                crate::syntax::TokenKind::Plain => "",
            };
            if class.is_empty() {
                html.push_str(&escape_html(&token.text));
            } else {
                html.push_str(&format!(
                    "<span class=\"{class}\">{}</span>",
                    escape_html(&token.text)
                ));
            }
        }
        if index + 1 < lines.len() {
            html.push('\n');
        }
    }
    html.push_str("</code></pre>");
    html
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

fn export_markdown(events: &[Event]) -> String {
    let mut out = String::from("# niucode session\n\n");
    for event in events {
        match event {
            Event::SessionStart {
                cwd,
                model,
                provider,
                ..
            } => {
                out.push_str(&format!(
                    "- cwd: `{cwd}`\n- model: `{model}`\n- provider: `{provider}`\n\n"
                ));
            }
            Event::UserMessage { text, .. } => {
                out.push_str("## User\n\n");
                out.push_str(text);
                out.push_str("\n\n");
            }
            Event::AssistantMessage { text, .. } => {
                if !text.trim().is_empty() {
                    out.push_str("## Assistant\n\n");
                    out.push_str(text);
                    out.push_str("\n\n");
                }
            }
            Event::ToolResult { ok, output, .. } => {
                out.push_str(&format!(
                    "### Tool result ({})\n\n````text\n",
                    if *ok { "ok" } else { "error" }
                ));
                out.push_str(output);
                out.push_str("\n````\n\n");
            }
            Event::TurnEnd { usage, .. } => {
                out.push_str(&format!(
                    "_tokens: {} in / {} out / {} cached_\n\n",
                    usage.input_tokens, usage.output_tokens, usage.cached_tokens
                ));
            }
            Event::Compaction { summary, .. } => {
                out.push_str("## Compaction summary\n\n");
                out.push_str(summary);
                out.push_str("\n\n");
            }
            _ => {}
        }
    }
    out
}

async fn send_download(stream: &mut TcpStream, markdown: String) -> Result<()> {
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/markdown; charset=utf-8\r\nContent-Disposition: attachment; filename=\"niucode-session.md\"\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\n\r\n",
        markdown.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(markdown.as_bytes()).await?;
    stream.shutdown().await?;
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

    fn state_with(context: Vec<u8>) -> Arc<State> {
        Arc::new(State {
            path: Mutex::new(PathBuf::new()),
            snapshot: Mutex::new(br#"{"events":[]}"#.to_vec()),
            provider: None,
            preset: "deepseek".into(),
            model: "deepseek-flash".into(),
            cwd: PathBuf::from("."),
            max_steps: 1,
            system_prompt: None,
            context,
            context_provider: None,
            brand: "niucode".into(),
            allow_new: true,
            new_session_provider: None,
            busy: AtomicBool::new(false),
            live: broadcast::channel(8).0,
            pending: Mutex::new(HashMap::new()),
            next_approval: AtomicU64::new(1),
        })
    }

    fn state() -> Arc<State> {
        state_with(br#"{}"#.to_vec())
    }

    fn state_at(path: PathBuf, allow_new: bool) -> Arc<State> {
        Arc::new(State {
            path: Mutex::new(path),
            snapshot: Mutex::new(Vec::new()),
            provider: None,
            preset: "deepseek".into(),
            model: "deepseek-flash".into(),
            cwd: PathBuf::from("."),
            max_steps: 1,
            system_prompt: None,
            context: br#"{}"#.to_vec(),
            context_provider: None,
            brand: "niucode".into(),
            allow_new,
            new_session_provider: None,
            busy: AtomicBool::new(false),
            live: broadcast::channel(8).0,
            pending: Mutex::new(HashMap::new()),
            next_approval: AtomicU64::new(1),
        })
    }

    #[test]
    fn export_markdown_keeps_user_assistant_and_tool_output() {
        let events = vec![
            Event::SessionStart {
                id: "s".into(),
                cwd: "/w".into(),
                model: "m".into(),
                provider: "deepseek".into(),
                ts: 0,
            },
            Event::UserMessage {
                text: "hello".into(),
                ts: 0,
            },
            Event::AssistantMessage {
                text: "world".into(),
                reasoning: None,
                tool_calls: vec![],
                ts: 0,
            },
            Event::ToolResult {
                call_id: "c".into(),
                ok: true,
                output: "output".into(),
                ts: 0,
            },
        ];
        let markdown = export_markdown(&events);
        assert!(markdown.starts_with("# niucode session"));
        assert!(markdown.contains("## User\n\nhello"));
        assert!(markdown.contains("## Assistant\n\nworld"));
        assert!(markdown.contains("````text\noutput\n````"));
        assert!(markdown.contains("cwd: `/w`"));
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
    fn fenced_triple_strings_keep_their_state() {
        let html = markdown_html("```python\ntext = \"\"\"start\nstill inside\nend\"\"\" + 1\n```");
        let strings = html.matches("tok-string").count();
        assert!(strings >= 2, "{html}");
        assert!(html.contains("tok-number"), "{html}");
    }

    #[test]
    fn fenced_block_comments_keep_their_state() {
        let html = markdown_html("```rust\n/* start\nstill */ let x = 1;\n```");
        let comments = html.matches("tok-comment").count();
        assert!(comments >= 2, "{html}");
        assert!(html.contains("tok-keyword"), "{html}");
    }

    #[test]
    fn fenced_code_gets_token_classes_and_stays_escaped() {
        let html = markdown_html("```rust\nlet x = \"hi\"; // note\n<script>\n```");
        assert!(html.contains("<pre class=\"code\">"), "{html}");
        assert!(html.contains("tok-keyword"), "{html}");
        assert!(html.contains("tok-string"), "{html}");
        assert!(html.contains("tok-comment"), "{html}");
        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
        assert!(!html.contains("```"), "{html}");
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
    async fn new_session_creates_another_log() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.jsonl");
        std::fs::write(&path, "").unwrap();
        let state = Arc::new(State {
            path: Mutex::new(path.clone()),
            snapshot: Mutex::new(Vec::new()),
            provider: None,
            preset: "deepseek".into(),
            model: "deepseek-flash".into(),
            cwd: directory.path().to_path_buf(),
            max_steps: 1,
            system_prompt: None,
            context: br#"{}"#.to_vec(),
            context_provider: None,
            brand: "niucode".into(),
            allow_new: true,
            new_session_provider: None,
            busy: AtomicBool::new(false),
            live: broadcast::channel(8).0,
            pending: Mutex::new(HashMap::new()),
            next_approval: AtomicU64::new(1),
        });
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(serve_listener(listener, state.clone()));
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream
            .write_all(b"POST /api/new HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).await.unwrap();
        assert!(
            String::from_utf8_lossy(&reply).starts_with("HTTP/1.1 200"),
            "{}",
            String::from_utf8_lossy(&reply)
        );
        let new_path = state.path.lock().unwrap().clone();
        assert_ne!(new_path, path);
        assert!(new_path.exists());
        task.abort();
    }

    #[tokio::test]
    async fn live_context_provider_is_called_per_request() {
        use std::sync::atomic::AtomicUsize;
        let mut state = state_with(Vec::new());
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        Arc::get_mut(&mut state).unwrap().context_provider = Some(Arc::new(move || {
            let n = count.fetch_add(1, Ordering::SeqCst) + 1;
            json!({"brand": format!("live-{n}")})
        }));
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(serve_listener(listener, state));
        for expected in ["live-1", "live-2"] {
            let mut stream = TcpStream::connect(address).await.unwrap();
            stream
                .write_all(b"GET /api/context HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .await
                .unwrap();
            let mut reply = Vec::new();
            stream.read_to_end(&mut reply).await.unwrap();
            let text = String::from_utf8_lossy(&reply);
            assert!(text.contains(expected), "{text}");
        }
        task.abort();
    }

    #[tokio::test]
    async fn exports_markdown_with_an_attachment_header() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.jsonl");
        std::fs::write(
            &path,
            "{\"type\":\"user_message\",\"text\":\"hello export\",\"ts\":0}\n",
        )
        .unwrap();
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(serve_listener(listener, state_at(path, true)));
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream
            .write_all(b"GET /api/export HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).await.unwrap();
        let text = String::from_utf8_lossy(&reply);
        assert!(text.starts_with("HTTP/1.1 200"), "{text}");
        assert!(text.contains("text/markdown"), "{text}");
        assert!(
            text.contains("attachment; filename=\"niucode-session.md\""),
            "{text}"
        );
        assert!(text.contains("# niucode session"), "{text}");
        assert!(text.contains("hello export"), "{text}");
        task.abort();
    }

    #[tokio::test]
    async fn new_session_can_be_disabled() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(serve_listener(
            listener,
            state_at(PathBuf::from("unused.jsonl"), false),
        ));
        let mut stream = TcpStream::connect(address).await.unwrap();
        stream
            .write_all(b"POST /api/new HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).await.unwrap();
        assert!(
            String::from_utf8_lossy(&reply).starts_with("HTTP/1.1 403"),
            "{}",
            String::from_utf8_lossy(&reply)
        );
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
                path: Mutex::new(path.clone()),
                snapshot: Mutex::new(Vec::new()),
                provider: None,
                preset: "deepseek".into(),
                model: "deepseek-flash".into(),
                cwd: PathBuf::from("."),
                max_steps: 1,
                system_prompt: None,
                context: br#"{}"#.to_vec(),
                context_provider: None,
                brand: "niucode".into(),
                allow_new: true,
                new_session_provider: None,
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
                path: Mutex::new(path.clone()),
                snapshot: Mutex::new(Vec::new()),
                provider: Some(provider),
                preset: "scripted".into(),
                model: "scripted".into(),
                cwd: directory.path().to_path_buf(),
                max_steps: 1,
                system_prompt: None,
                context: br#"{}"#.to_vec(),
                context_provider: None,
                brand: "niucode".into(),
                allow_new: true,
                new_session_provider: None,
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
        assert!(String::from_utf8_lossy(&live).contains("\"html\":"));
        let changed = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if String::from_utf8_lossy(&live).contains("context_changed") {
                    return;
                }
                let mut chunk = [0_u8; 1024];
                let count = events.read(&mut chunk).await.unwrap();
                live.extend_from_slice(&chunk[..count]);
            }
        })
        .await;
        changed.expect("context_changed after turn");
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
                path: Mutex::new(path.clone()),
                snapshot: Mutex::new(Vec::new()),
                provider: Some(provider),
                preset: "scripted".into(),
                model: "scripted".into(),
                cwd: directory.path().to_path_buf(),
                max_steps: 2,
                system_prompt: None,
                context: br#"{}"#.to_vec(),
                context_provider: None,
                brand: "niucode".into(),
                allow_new: true,
                new_session_provider: None,
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
        assert!(INDEX.contains("Enter to send"));
        assert!(INDEX.contains("fetch('/api/prompt'"));
        assert!(INDEX.contains("text_delta"));
        assert!(INDEX.contains("renderApproval"));
        assert!(INDEX.contains("renderPreview"));
        assert!(INDEX.contains("renderToolCall"));
        assert!(INDEX.contains("tool-card"));
        assert!(INDEX.contains("textarea"));
        assert!(INDEX.contains("Shift+Enter"));
        assert!(INDEX.contains("prompt.history"));
        assert!(INDEX.contains("copy-code"));
        assert!(INDEX.contains("context_changed"));
        assert!(INDEX.contains("'/help'"));
        assert!(INDEX.contains("unknown command"));
        assert!(INDEX.contains("cmd-menu"));
        assert!(INDEX.contains("session log unchanged"));
        assert!(INDEX.contains("usageTotals"));
        assert!(INDEX.contains("/api/export"));
        assert!(INDEX.contains("filterTranscript"));
        assert!(INDEX.contains("scrollToLatest"));
        assert!(INDEX.contains("prefers-reduced-motion"));
        assert!(INDEX.contains("niucode.theme"));
        assert!(INDEX.contains("/new"));
        assert!(INDEX.contains("diff-line"));
        assert!(INDEX.contains("e.html"));
        assert!(INDEX.contains("math"));
        assert!(INDEX.contains("tok-keyword"));
        assert!(INDEX.contains("/api/context"));
        assert!(INDEX.contains("/api/status"));
        assert!(INDEX.contains("beginTurn"));
        assert!(INDEX.contains("toggle-context"));
        assert!(INDEX.contains("ctx-filter"));
        assert!(INDEX.contains("ctx-section"));
        assert!(INDEX.contains("loadContext"));
        assert!(INDEX.contains("localStorage"));
        assert!(INDEX.contains("textarea"));
        assert!(INDEX.contains("Shift+Enter"));
        assert!(INDEX.contains("prompt.history"));
        assert!(INDEX.contains("copy-code"));
        assert!(INDEX.contains("context_changed"));
        assert!(INDEX.contains("'/help'"));
        assert!(INDEX.contains("unknown command"));
        assert!(INDEX.contains("cmd-menu"));
        assert!(INDEX.contains("session log unchanged"));
        assert!(INDEX.contains("usageTotals"));
        assert!(INDEX.contains("/api/export"));
        assert!(INDEX.contains("filterTranscript"));
        assert!(INDEX.contains("scrollToLatest"));
        assert!(INDEX.contains("prefers-reduced-motion"));
        assert!(INDEX.contains("niucode.theme"));
        assert!(INDEX.contains("/new"));
        assert!(!INDEX.contains("http://"));
        assert!(!INDEX.contains("https://"));
    }
}
