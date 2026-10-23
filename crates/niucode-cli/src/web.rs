//! Local, read-only web surface for a session log.
//!
//! v0 serves one session snapshot and a small client. It binds loopback only;
//! chat, approvals, and SSE are later slices (ADR-0036).

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use niucode_core::load;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const INDEX: &str = include_str!("../assets/web.html");

pub async fn serve(path: &Path, port: u16) -> Result<()> {
    let loaded = load(path).with_context(|| format!("load session {}", path.display()))?;
    let body = serde_json::to_vec(&json!({
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
        "web: http://{address}/ · {} events · Ctrl-C to stop",
        loaded.events.len()
    );
    serve_listener(listener, Arc::new(body)).await
}

pub async fn serve_listener(listener: TcpListener, session: Arc<Vec<u8>>) -> Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let session = session.clone();
        tokio::spawn(async move {
            let _ = handle(stream, session).await;
        });
    }
}

async fn handle(mut stream: TcpStream, session: Arc<Vec<u8>>) -> Result<()> {
    let mut buffer = [0_u8; 8192];
    let count = stream.read(&mut buffer).await?;
    let request = String::from_utf8_lossy(&buffer[..count]);
    let mut parts = request.lines().next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");
    let (status, kind, body) = response(method, path, session.as_ref());
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&body).await?;
    stream.shutdown().await?;
    Ok(())
}

fn response(method: &str, path: &str, session: &[u8]) -> (&'static str, &'static str, Vec<u8>) {
    match (method, path) {
        ("GET", "/") => (
            "200 OK",
            "text/html; charset=utf-8",
            INDEX.as_bytes().to_vec(),
        ),
        ("GET", "/api/session") => ("200 OK", "application/json", session.to_vec()),
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
        let task = tokio::spawn(serve_listener(
            listener,
            Arc::new(br#"{"events":[]}"#.to_vec()),
        ));
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

    #[test]
    fn index_is_a_static_local_client_with_no_remote_assets() {
        assert!(INDEX.contains("fetch('/api/session')"));
        assert!(INDEX.contains("Read-only local viewer"));
        assert!(!INDEX.contains("http://"));
        assert!(!INDEX.contains("https://"));
    }
}
