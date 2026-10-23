//! Local, read-only web surface for a session log.
//!
//! v0 serves one session and a small client. It binds loopback only; chat and
//! approvals are later slices (ADR-0036). `/api/events` is a chunked SSE tail
//! of the append-only log, so the browser can watch a live session.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use niucode_core::load;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const INDEX: &str = include_str!("../assets/web.html");

struct State {
    path: PathBuf,
    snapshot: Vec<u8>,
}

pub async fn serve(path: &Path, port: u16) -> Result<()> {
    let loaded = load(path).with_context(|| format!("load session {}", path.display()))?;
    let snapshot = serde_json::to_vec(&json!({
        "id": path.file_stem().and_then(|stem| stem.to_str()).unwrap_or("session"),
        "path": path.display().to_string(),
        "warnings": loaded.warnings,
        "events": loaded.events,
    }))?;
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .context("bind 127.0.0.1")?;
    let address = listener.local_addr()?;
    println!(
        "web: http://{address}/ · {} events · live tail · Ctrl-C to stop",
        loaded.events.len()
    );
    serve_listener(
        listener,
        Arc::new(State {
            path: path.to_path_buf(),
            snapshot,
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
    let mut buffer = [0_u8; 8192];
    let count = stream.read(&mut buffer).await?;
    let request = String::from_utf8_lossy(&buffer[..count]);
    let mut parts = request.lines().next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");
    if method == "GET" && path == "/api/events" {
        return stream_events(stream, state.path.clone()).await;
    }
    let (status, kind, body) = response(method, path, &state.snapshot);
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

    #[test]
    fn index_uses_sse_and_has_no_remote_assets() {
        assert!(INDEX.contains("EventSource('/api/events')"));
        assert!(INDEX.contains("Read-only local viewer"));
        assert!(!INDEX.contains("http://"));
        assert!(!INDEX.contains("https://"));
    }
}
