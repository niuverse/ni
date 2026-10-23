//! Optional real-browser smoke test for the single-file web client.
//!
//! Chrome is not required for the normal suite; when unavailable the test
//! skips. SSE is stubbed so the static page can settle without a server.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn embedded_client_parses_in_a_headless_browser() {
    let chrome = std::env::var("CHROME").unwrap_or_else(|_| "google-chrome".to_string());
    if Command::new(&chrome)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_err()
    {
        eprintln!("skipping browser smoke test: {chrome} not available");
        return;
    }
    let html = include_str!("../assets/web.html").replace(
        "const source = new EventSource('/api/events');",
        "const source = { set onopen(v){}, set onerror(v){}, set onmessage(v){}, close(){} };",
    );
    let directory = tempfile::tempdir().expect("tempdir");
    let index = directory.path().join("index.html");
    std::fs::write(&index, html).expect("write fixture");
    let profile = directory.path().join("profile");
    let url = format!("file://{}", index.display());
    let mut child = Command::new(&chrome)
        .args([
            "--headless=new",
            "--no-sandbox",
            "--disable-gpu",
            "--disable-dev-shm-usage",
            "--no-first-run",
            "--no-zygote",
            "--hide-scrollbars",
            "--virtual-time-budget=1200",
            "--dump-dom",
            &format!("--user-data-dir={}", profile.display()),
            &url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn chrome");
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if child.try_wait().expect("wait chrome").is_some() {
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("headless chrome did not exit in 60s");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let output = child.wait_with_output().expect("chrome output");
    let dom = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "chrome failed: {dom}");
    assert!(dom.contains("role=\"log\""), "transcript log role missing");
    assert!(dom.contains("niucode"), "brand missing");
    assert!(dom.contains("Prompt"), "composer missing");
    assert!(Path::new(&index).exists());
}
