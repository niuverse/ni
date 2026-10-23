//! End-to-end computer tool path against a real X11 display.
//!
//! Skips when Xvfb is unavailable. CI installs it on Linux.

#![cfg(target_os = "linux")]

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use niucode_agent::{
    run_with_history, AgentConfig, ApprovalMode, ApprovalRequest, ComputerRuntime,
};
use niucode_computer::ComputerPolicy;
use niucode_computer_x11::X11Backend;
use niucode_core::{load, Event, SessionLog};
use niucode_provider::{
    ChatMessage, CompletionRequest, ContentPart, EventStream, MessageContent, Provider,
    ProviderError, StreamEvent, ToolCallDelta,
};
use niucode_tools::{NetworkMode, SandboxMode, SandboxPolicy};
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{
    ChangeWindowAttributesAux, ConnectionExt as _, CreateWindowAux, EventMask, WindowClass,
};

struct Xvfb(Child);
impl Drop for Xvfb {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_xvfb() -> Option<(Xvfb, String)> {
    let mut child = Command::new("Xvfb")
        .args(["-displayfd", "1", "-screen", "0", "320x240x24"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    use std::io::{BufRead, BufReader};
    let stdout = child.stdout.take()?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let display = format!(":{}", line.trim());
    std::thread::sleep(Duration::from_millis(250));
    Some((Xvfb(child), display))
}

fn message_text(message: &ChatMessage) -> Option<String> {
    match message.content.as_ref()? {
        MessageContent::Text(text) => Some(text.clone()),
        MessageContent::Parts(parts) => {
            let mut out = String::new();
            for part in parts {
                if let ContentPart::Text { text } = part {
                    out.push_str(text);
                }
            }
            Some(out)
        }
    }
}

struct ComputerProvider {
    window: u64,
    calls: AtomicUsize,
    observation: Mutex<Option<String>>,
    saw_image: AtomicBool,
}

#[async_trait]
impl Provider for ComputerProvider {
    fn name(&self) -> &str {
        "computer-e2e"
    }

    async fn stream(&self, request: CompletionRequest) -> Result<EventStream, ProviderError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            return Ok(Box::pin(futures_util::stream::iter(vec![
                Ok(StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("observe".into()),
                    name: Some("computer".into()),
                    arguments: format!(
                        r#"{{"action":"observe","window_id":{},"purpose":"verify"}}"#,
                        self.window
                    ),
                })),
                Ok(StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                }),
            ])));
        }
        if call == 1 {
            let tool = request
                .messages
                .iter()
                .rev()
                .find(|message| message.role == niucode_provider::Role::Tool)
                .expect("tool result");
            if tool.has_images() {
                self.saw_image.store(true, Ordering::SeqCst);
            }
            let text = message_text(tool).expect("tool text");
            let observation = text
                .split_whitespace()
                .find(|word| word.starts_with("obs-"))
                .map(|word| word.trim_end_matches('·').to_string())
                .or_else(|| {
                    text.split_whitespace()
                        .skip_while(|word| *word != "observation")
                        .nth(1)
                        .map(|word| word.trim_end_matches('·').to_string())
                })
                .expect("observation id");
            *self.observation.lock().unwrap() = Some(observation.clone());
            return Ok(Box::pin(futures_util::stream::iter(vec![
                Ok(StreamEvent::ToolCallDelta(ToolCallDelta {
                    index: 0,
                    id: Some("click".into()),
                    name: Some("computer".into()),
                    arguments: format!(
                        r#"{{"action":"click","observation_id":"{observation}","window_id":{},"x":60,"y":40,"purpose":"e2e"}}"#,
                        self.window
                    ),
                })),
                Ok(StreamEvent::Done {
                    finish_reason: Some("tool_calls".into()),
                }),
            ])));
        }
        Ok(Box::pin(futures_util::stream::iter(vec![
            Ok(StreamEvent::TextDelta("done".into())),
            Ok(StreamEvent::Done {
                finish_reason: Some("stop".into()),
            }),
        ])))
    }
}

#[tokio::test]
async fn observe_image_and_approved_click_reach_the_window() {
    let Some((_guard, display)) = start_xvfb() else {
        eprintln!("skipping: Xvfb is not installed");
        return;
    };
    let (connection, screen) = x11rb::connect(Some(&display)).expect("test connection");
    let root = connection.setup().roots[screen].root;
    let window = connection.generate_id().expect("window");
    let width = 120_u16;
    let height = 80_u16;
    connection
        .create_window(
            0,
            window,
            root,
            10,
            10,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new().background_pixel(0x0000ff),
        )
        .expect("create");
    connection
        .change_window_attributes(
            window,
            &ChangeWindowAttributesAux::new()
                .event_mask(EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE),
        )
        .expect("select events");
    connection.map_window(window).expect("map");
    connection.flush().expect("flush");
    std::thread::sleep(Duration::from_millis(250));

    let backend = X11Backend::connect(Some(&display)).expect("backend");
    let mut runtime = ComputerRuntime::new(Box::new(backend), ComputerPolicy::Control);
    runtime.arm().expect("arm");
    let directory = tempfile::tempdir().unwrap();
    let mut log = SessionLog::create(directory.path()).unwrap();
    let mut config = AgentConfig::new("computer-e2e", directory.path());
    config.provider = "computer-e2e".into();
    config.approval = ApprovalMode::Ask;
    config.sandbox = SandboxPolicy::new(SandboxMode::ReadOnly, NetworkMode::Off);
    config.computer = Some(Arc::new(std::sync::Mutex::new(runtime)));
    let provider = ComputerProvider {
        window: u64::from(window),
        calls: AtomicUsize::new(0),
        observation: Mutex::new(None),
        saw_image: AtomicBool::new(false),
    };
    let mut approvals = 0;
    run_with_history(
        &provider,
        &mut log,
        &config,
        "click the blue window",
        Vec::new(),
        &mut |_request: &ApprovalRequest| {
            approvals += 1;
            true
        },
        &mut |_event| {},
    )
    .await
    .expect("computer turn");

    assert!(
        provider.saw_image.load(Ordering::SeqCst),
        "observe image missing"
    );
    assert!(provider.observation.lock().unwrap().is_some());
    assert_eq!(approvals, 1, "control click must ask once");
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut clicked = false;
    while Instant::now() < deadline {
        if let Some(x11rb::protocol::Event::ButtonPress(_)) =
            connection.poll_for_event().expect("poll")
        {
            clicked = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(clicked, "XTEST click did not reach the window");
    let loaded = load(log.path()).unwrap();
    assert!(loaded.events.iter().any(|event| matches!(
        event,
        Event::Approval { tool, allowed: true, .. } if tool == "computer"
    )));
}
