//! Benchmark-only metered API gateway. Never linked into the agent kernel.
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, ensure, Context, Result};
use clap::Parser;
use futures_util::StreamExt;
use niucode_auth::{AuthStore, SecretKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const MODEL: &str = "deepseek-flash";
// CNY micro-units/token: peak uncached input 2 CNY/M, output 8 CNY/M.
// No cache/off-peak discounts assumed. Reservation uses the full 1M context.
const RESERVE: u64 = 1_048_576 * 2 + 8192 * 8;
const LIMIT: u64 = 300_000_000;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:18765")]
    listen: String,
    #[arg(long)]
    journal: PathBuf,
    #[arg(long)]
    probe: bool,
    /// Explicitly import only the provider's literal API-key assignment; never execute shell.
    #[arg(long)]
    credential_shell_file: Option<PathBuf>,
}

struct Lock(PathBuf);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct Budget {
    file: File,
    charges: BTreeMap<u64, u64>,
    run_counts: BTreeMap<String, u64>,
    next: u64,
    poisoned: bool,
}

impl Budget {
    fn open(path: &std::path::Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut charges = BTreeMap::new();
        let mut run_counts = BTreeMap::new();
        let mut next = 0;
        if path.exists() {
            for line in std::fs::read_to_string(path)?.lines() {
                // Unlike session replay, billing fails closed on a damaged tail.
                let v: Value =
                    serde_json::from_str(line).context("invalid budget journal; refusing calls")?;
                let id = v["request"].as_u64().context("missing request id")?;
                next = next.max(id + 1);
                match v["kind"].as_str() {
                    Some("reserved") => {
                        ensure!(
                            charges.insert(id, RESERVE).is_none(),
                            "duplicate reservation"
                        );
                        *run_counts
                            .entry(v["run"].as_str().context("missing run")?.to_string())
                            .or_insert(0) += 1;
                    }
                    Some("settled") => {
                        let charge = v["upper_micro_cny"].as_u64().context("missing charge")?;
                        ensure!(charges.contains_key(&id), "settlement without reservation");
                        ensure!(charge <= RESERVE, "cost exceeds reserved upper bound");
                        charges.insert(id, charge);
                    }
                    Some("unknown") => ensure!(charges.contains_key(&id), "unknown request"),
                    _ => bail!("unknown journal event"),
                }
            }
        }
        Ok(Self {
            file: OpenOptions::new().create(true).append(true).open(path)?,
            charges,
            run_counts,
            next,
            poisoned: false,
        })
    }

    fn append(&mut self, event: Value) -> Result<()> {
        ensure!(
            !self.poisoned,
            "budget journal disabled after write failure"
        );
        let result = (|| {
            writeln!(self.file, "{event}")?;
            self.file.sync_all()
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        Ok(result?)
    }

    fn reserve(&mut self, run: &str, protocol: &str, body: &[u8]) -> Result<u64> {
        ensure!(!self.poisoned, "budget disabled");
        ensure!(
            self.charges.values().sum::<u64>() + RESERVE <= LIMIT,
            "campaign budget exhausted"
        );
        ensure!(
            *self.run_counts.get(run).unwrap_or(&0) < 40,
            "run request limit reached"
        );
        let id = self.next;
        self.append(
            json!({"kind":"reserved","request":id,"run":run,"protocol":protocol,
            "model":MODEL,"thinking":"enabled","effort":"high","max_tokens":8192,
            "upper_micro_cny":RESERVE,"request_sha256":format!("{:x}",Sha256::digest(body)),
            "unix_seconds":SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()}),
        )?;
        self.charges.insert(id, RESERVE);
        *self.run_counts.entry(run.to_owned()).or_insert(0) += 1;
        self.next += 1;
        Ok(id)
    }

    fn settle(&mut self, id: u64, usage: &Usage) -> Result<()> {
        if !usage.complete {
            return self.append(json!({"kind":"unknown","request":id,
                "reason":"missing final usage; full reservation retained"}));
        }
        let charge = usage
            .input
            .saturating_mul(2)
            .saturating_add(usage.output.saturating_mul(8));
        if charge > RESERVE {
            self.poisoned = true;
            bail!("usage exceeds reserved bound; stop campaign");
        }
        self.append(
            json!({"kind":"settled","request":id,"upper_micro_cny":charge,
            "input_tokens":usage.input,"output_tokens":usage.output,"returned_model":usage.model}),
        )?;
        self.charges.insert(id, charge);
        Ok(())
    }
}

#[derive(Default)]
struct Usage {
    input: u64,
    output: u64,
    complete: bool,
    model: Option<String>,
}
impl Usage {
    fn observe(&mut self, v: &Value, anthropic: bool) {
        if let Some(model) = v["model"]
            .as_str()
            .or_else(|| v["message"]["model"].as_str())
        {
            self.model = Some(model.to_owned());
        }
        if anthropic {
            let u = if v["type"] == "message_start" {
                &v["message"]["usage"]
            } else {
                &v["usage"]
            };
            if let Some(input) = u["input_tokens"].as_u64() {
                self.input = input
                    + u["cache_read_input_tokens"].as_u64().unwrap_or(0)
                    + u["cache_creation_input_tokens"].as_u64().unwrap_or(0);
            }
            if let Some(output) = u["output_tokens"].as_u64() {
                self.output = output;
                if v["type"] == "message_delta" || v["type"] == "message" {
                    self.complete = true;
                }
            }
        } else if let (Some(input), Some(output)) = (
            v["usage"]["prompt_tokens"].as_u64(),
            v["usage"]["completion_tokens"].as_u64(),
        ) {
            self.input = input;
            self.output = output;
            self.complete = true;
        }
    }
}

async fn read_request(socket: &mut TcpStream) -> Result<(String, Vec<u8>)> {
    let mut data = Vec::new();
    let end = loop {
        let mut block = [0; 4096];
        let n = socket.read(&mut block).await?;
        ensure!(n > 0, "request closed");
        data.extend_from_slice(&block[..n]);
        if let Some(end) = data.windows(4).position(|b| b == b"\r\n\r\n") {
            break end + 4;
        }
        ensure!(data.len() <= 64 * 1024, "headers too large");
    };
    let headers = std::str::from_utf8(&data[..end])?;
    let first = headers.lines().next().context("missing request line")?;
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("").to_owned();
    ensure!(matches!(method, "GET" | "POST"), "unsupported method");
    let lower = headers.to_ascii_lowercase();
    ensure!(
        !lower.contains("transfer-encoding:"),
        "chunked requests not accepted"
    );
    let length: usize = lower
        .lines()
        .find_map(|s| s.strip_prefix("content-length:"))
        .unwrap_or("0")
        .trim()
        .parse()?;
    ensure!(length <= 512 * 1024, "request too large");
    while data.len() < end + length {
        let mut block = [0; 8192];
        let n = socket.read(&mut block).await?;
        ensure!(n > 0, "short body");
        data.extend_from_slice(&block[..n]);
    }
    Ok((path, data[end..end + length].to_vec()))
}

async fn response(socket: &mut TcpStream, status: &str, body: &str) -> Result<()> {
    socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await?;
    Ok(())
}

async fn serve(
    socket: &mut TcpStream,
    client: &reqwest::Client,
    key: &SecretKey,
    budget: &mut Budget,
) -> Result<()> {
    let (path, body) =
        tokio::time::timeout(Duration::from_secs(15), read_request(socket)).await??;
    let components: Vec<_> = path.trim_start_matches('/').split('/').collect();
    ensure!(components.len() >= 2, "run prefix required");
    let run = components[0];
    ensure!(
        !run.is_empty()
            && run.len() < 128
            && run
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "invalid run"
    );
    let endpoint = components[1..].join("/");
    if endpoint == "v1/models" || endpoint == "models" {
        return response(
            socket,
            "200 OK",
            &json!({"object":"list","data":[{"id":MODEL,"object":"model","owned_by":"deepseek"}]})
                .to_string(),
        )
        .await;
    }
    let anthropic = endpoint == "anthropic/v1/messages";
    ensure!(
        anthropic || endpoint == "v1/chat/completions" || endpoint == "chat/completions",
        "unsupported endpoint"
    );
    let mut body: Value = serde_json::from_slice(&body)?;
    ensure!(body["model"] == MODEL, "model does not match campaign");
    // These are explicit, shared benchmark overrides, recorded for every request.
    body["thinking"] = json!({"type":"enabled"});
    body["max_tokens"] = json!(8192);
    body.as_object_mut()
        .context("object required")?
        .remove("max_completion_tokens");
    if anthropic {
        body["output_config"] = json!({"effort":"high"});
    } else {
        body["reasoning_effort"] = json!("high");
        if body["stream"] == true {
            body["stream_options"] = json!({"include_usage":true});
        }
    }
    // Reject server-side tools; only local function tools may run.
    if let Some(tools) = body["tools"].as_array() {
        for tool in tools {
            if let Some(kind) = tool["type"].as_str() {
                ensure!(
                    kind == "function" || kind == "custom",
                    "server-side tools not allowed"
                );
            }
        }
    }
    let encoded = serde_json::to_vec(&body)?;
    let id = budget.reserve(
        run,
        if anthropic {
            "messages"
        } else {
            "chat-completions"
        },
        &encoded,
    )?;
    let operation = async {
        let url = if anthropic {
            "https://api.deepseek.com/anthropic/v1/messages"
        } else {
            "https://api.deepseek.com/chat/completions"
        };
        let mut request = client
            .post(url)
            .bearer_auth(key.expose())
            .header("content-type", "application/json");
        if anthropic {
            request = request
                .header("x-api-key", key.expose())
                .header("anthropic-version", "2023-06-01");
        }
        let upstream = request.body(encoded).send().await?;
        ensure!(
            upstream.status().is_success(),
            "upstream HTTP {}",
            upstream.status().as_u16()
        );
        let streaming = body["stream"] == true;
        let content = if streaming {
            "text/event-stream"
        } else {
            "application/json"
        };
        socket
            .write_all(
                format!("HTTP/1.1 200 OK\r\nContent-Type: {content}\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await?;
        let mut pending = Vec::new();
        let mut usage = Usage::default();
        let mut stream = upstream.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            socket.write_all(&chunk).await?;
            pending.extend_from_slice(&chunk);
            ensure!(pending.len() < 2 * 1024 * 1024, "response event too large");
            if streaming {
                while let Some(end) = pending.iter().position(|b| *b == b'\n') {
                    let line: Vec<_> = pending.drain(..=end).collect();
                    if let Ok(text) = std::str::from_utf8(&line) {
                        if let Some(data) = text.trim().strip_prefix("data:") {
                            if let Ok(value) = serde_json::from_str(data.trim()) {
                                usage.observe(&value, anthropic);
                            }
                        }
                    }
                }
            }
        }
        if !streaming {
            usage.observe(&serde_json::from_slice(&pending)?, anthropic);
        }
        Ok::<_, anyhow::Error>(usage)
    };
    match tokio::time::timeout(Duration::from_secs(180),operation).await {
        Ok(Ok(usage)) => budget.settle(id,&usage)?,
        _ => budget.append(json!({"kind":"unknown","request":id,"reason":"upstream failure, timeout or disconnect; reservation retained"}))?,
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let auth = AuthStore::new()?;
    let resolved = match &args.credential_shell_file {
        Some(path) => auth.resolve_with_shell_file("deepseek", path)?,
        None => auth.resolve("deepseek")?,
    };
    let key = resolved
        .context("DeepSeek credential unavailable via niucode-auth")?
        .key;
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    if args.probe {
        let result = client
            .get("https://api.deepseek.com/models")
            .bearer_auth(key.expose())
            .send()
            .await?;
        ensure!(
            result.status().is_success(),
            "models probe HTTP {}",
            result.status().as_u16()
        );
        let result: Value = result.json().await?;
        for model in result["data"].as_array().context("model list missing")? {
            if let Some(id) = model["id"].as_str() {
                println!("{id}");
            }
        }
        return Ok(());
    }
    if let Some(parent) = args.journal.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_path = args.journal.with_extension("lock");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .context("campaign already locked; stale lock requires inspection")?;
    let _lock = Lock(lock_path);
    let mut budget = Budget::open(&args.journal)?;
    let listener = TcpListener::bind(&args.listen).await?;
    println!(
        "benchmark gateway ready at {}; model={MODEL}; cap=300 CNY",
        args.listen
    );
    let work = async {
        loop {
            let (mut socket, _) = listener.accept().await?;
            if serve(&mut socket, &client, &key, &mut budget)
                .await
                .is_err()
            {
                // Error details may contain request text; only a fixed diagnostic leaves the gateway.
                let _=response(&mut socket,"429 Too Many Requests",r#"{"error":{"message":"benchmark gateway rejected request; inspect campaign journal","type":"budget_or_request_error"}}"#).await;
            }
            if budget.poisoned {
                bail!("budget journal failed; gateway stopped");
            }
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    };
    tokio::select! {
        result=work => result,
        result=tokio::signal::ctrl_c()=>{result?;Ok(())},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reservations_survive_restart_and_cap_is_enforced() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("billing.jsonl");
        let mut budget = Budget::open(&path).unwrap();
        let id = budget.reserve("a", "chat", b"{}").unwrap();
        drop(budget);
        let mut budget = Budget::open(&path).unwrap();
        assert_eq!(budget.charges[&id], RESERVE);
        budget
            .settle(
                id,
                &Usage {
                    input: 10,
                    output: 20,
                    complete: true,
                    model: None,
                },
            )
            .unwrap();
        assert_eq!(budget.charges[&id], 180);
        for n in 0..200 {
            if budget.reserve(&format!("r{n}"), "chat", b"{}").is_err() {
                break;
            }
        }
        assert!(budget.charges.values().sum::<u64>() <= LIMIT);
        assert!(budget.reserve("last", "chat", b"{}").is_err());
    }
    #[test]
    fn corrupt_billing_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("billing.jsonl");
        std::fs::write(&path, "{\"kind\":").unwrap();
        assert!(Budget::open(&path).is_err());
    }
    #[test]
    fn usage_counts_cached_input_and_reasoning_in_output() {
        let mut usage = Usage::default();
        usage.observe(&json!({"type":"message_start","message":{"usage":{"input_tokens":20,"cache_read_input_tokens":80},"model":MODEL}}),true);
        usage.observe(
            &json!({"type":"message_delta","usage":{"output_tokens":500}}),
            true,
        );
        assert!(usage.complete);
        assert_eq!(usage.input, 100);
        assert_eq!(usage.output, 500);
        let mut chat = Usage::default();
        chat.observe(&json!({"usage":{"prompt_tokens":100,"completion_tokens":500,"completion_tokens_details":{"reasoning_tokens":450}}}),false);
        assert_eq!(chat.output, 500);
    }
}
