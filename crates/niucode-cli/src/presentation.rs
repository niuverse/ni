//! Task facts to compact transcript and full evidence. No terminal or log writes.
use crate::text_layout::{clean_text, clip};
use niucode_agent::TaskReview;
use niucode_core::ProcessOutcome;

pub fn startup(
    project: &str,
    model: &str,
    sandbox: &str,
    network: &str,
    approval: &str,
    width: usize,
) -> Vec<String> {
    vec![
        clip(
            &format!(
                "niucode tui · {} · {}",
                clip(&clean_text(project), 20),
                clip(&clean_text(model), 32)
            ),
            width,
        ),
        clip(
            &format!("{sandbox} · net {network} · {approval} · /help"),
            width,
        ),
    ]
}

pub fn completion(review: &TaskReview, width: usize) -> String {
    let failed = review
        .checks
        .iter()
        .filter(|c| !matches!(c.outcome, ProcessOutcome::Exited { code: 0 }))
        .count();
    let warnings = failed + review.tool_failures + review.denied + review.conflicts.len();
    let label = match review.status.as_deref() {
        Some("complete") if warnings == 0 => "✓ done",
        Some("complete") => "! finished with issues",
        Some("cancelled") => "! cancelled",
        Some("error") => "✗ failed",
        _ => "! interrupted",
    };
    let mut lines = vec![format!(
        "{label} · {:.1}s",
        review.elapsed_ms as f64 / 1000.0
    )];
    let relevant = !review.changed.is_empty()
        || review.writes > 0
        || !review.checks.is_empty()
        || warnings > 0;
    if relevant {
        let checks = if review.checks.is_empty() {
            "no checks".into()
        } else {
            format!("{} commands, {failed} failed", review.checks.len())
        };
        lines.push(if review.changed.is_empty() && review.writes > 0 {
            format!("{} recorded writes · {checks}", review.writes)
        } else {
            format!("{} confirmed changes · {checks}", review.changed.len())
        });
        let note = if !review.conflicts.is_empty() {
            "conflicts"
        } else if review.denied > 0 {
            "approval denied"
        } else if !review.unknown.is_empty() {
            "partial coverage"
        } else if review.checks.iter().any(|c| !c.fresh) {
            "checks may be stale"
        } else {
            "evidence"
        };
        lines.push(format!("{note} · Ctrl-O details"));
    }
    lines
        .iter()
        .map(|s| clip(s, width))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn evidence(review: &TaskReview) -> String {
    let mut body = format!(
        "# Task {}\nStatus: {} · {:.1}s\ncwd: {}\n\n",
        review.id,
        review.status.as_deref().unwrap_or("interrupted"),
        review.elapsed_ms as f64 / 1000.0,
        review.cwd
    );
    for file in &review.changed {
        body.push_str(&format!(
            "## {} · {}\n{}\n",
            file.path,
            if file.attributed {
                "recorded file tool"
            } else {
                "workspace change · origin unknown"
            },
            file.diff.as_deref().unwrap_or(
                "Full text diff unavailable (binary, missing or outside captured coverage)."
            )
        ));
    }
    body.push_str("# Command evidence\n");
    if review.checks.is_empty() {
        body.push_str("No recorded command checks.\n");
    }
    for check in &review.checks {
        let result = match check.outcome {
            ProcessOutcome::Exited { code } => format!("exit {code}"),
            ProcessOutcome::Signaled { signal } => format!("signal {signal:?}"),
            ProcessOutcome::TimedOut => "timed out".into(),
        };
        body.push_str(&format!(
            "{} · {result} · {}\n$ {}\n",
            check.call_id,
            if check.fresh {
                "captured workspace unchanged since command"
            } else {
                "final workspace not verified / result may be stale"
            },
            check.command
        ));
    }
    body.push_str(
        "Command exit status is evidence for that command, not proof of all acceptance criteria.\n",
    );
    if !review.conflicts.is_empty() {
        body.push_str(&format!("\n# Conflicts\n{}\n", review.conflicts.join("\n")));
    }
    body.push_str(&format!(
        "\nTool failures: {} · denied approvals: {}\n",
        review.tool_failures, review.denied
    ));
    if !review.unknown.is_empty() {
        body.push_str(&format!(
            "\n# Coverage unknown\n{}\n",
            review.unknown.join("\n")
        ));
    }
    body.push_str(
        "\nMissing evidence is not proof of no changes. No automatic retry of uncertain writes.\n",
    );
    body
}

pub fn activity(
    state: &str,
    tools: usize,
    elapsed_ms: u128,
    animate: bool,
    width: usize,
    streaming: bool,
) -> String {
    const FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    let frame = if animate {
        FRAMES[(elapsed_ms / 125 % FRAMES.len() as u128) as usize]
    } else {
        "·"
    };
    let state = clip(&clean_text(state), width.saturating_sub(34).max(8));
    let caret = if streaming {
        if animate && (elapsed_ms / 250).is_multiple_of(2) {
            " ▍"
        } else {
            " ▏"
        }
    } else {
        ""
    };
    clip(
        &format!(
            "{frame} {state}{caret} · {}s · {tools} tools · Ctrl-C cancel",
            elapsed_ms / 1000
        ),
        width,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use niucode_agent::ReviewedFile;
    #[test]
    fn quiet_chat_and_large_reviews_have_bounded_transcripts() {
        let mut review = TaskReview {
            status: Some("complete".into()),
            unknown: vec!["unobserved".into(); 130],
            ..Default::default()
        };
        assert_eq!(completion(&review, 80), "✓ done · 0.0s");
        for count in [1, 100, 1000] {
            review.changed = (0..count)
                .map(|_| ReviewedFile {
                    path: "long路径".repeat(100),
                    attributed: false,
                    diff: None,
                })
                .collect();
            for width in [20, 40, 80, 120] {
                let text = completion(&review, width);
                assert!(crate::text_layout::wrap(&text, width).len() <= 3);
                assert!(!text.contains("long"));
            }
        }
        assert!(evidence(&review).contains(&review.changed[0].path));
    }
    #[test]
    fn long_commands_remain_in_details_and_never_expand_completion() {
        let command = "cargo test ".repeat(5000);
        let review = TaskReview {
            status: Some("complete".into()),
            checks: vec![niucode_agent::CommandCheck {
                call_id: "check".into(),
                command: command.clone(),
                outcome: ProcessOutcome::Exited { code: 1 },
                fresh: false,
            }],
            ..Default::default()
        };
        for width in [20, 40, 80, 120] {
            let summary = completion(&review, width);
            assert!(crate::text_layout::wrap(&summary, width).len() <= 3);
            assert!(!summary.contains("✓"));
            assert!(!summary.contains("cargo"));
        }
        assert!(evidence(&review).contains(&command));
    }

    #[test]
    fn failures_denials_and_truncation_never_present_success() {
        let mut review = TaskReview {
            status: Some("complete".into()),
            denied: 1,
            ..Default::default()
        };
        assert!(!completion(&review, 80).contains("✓"));
        review.denied = 0;
        for status in ["cancelled", "error", "unknown"] {
            review.status = Some(status.into());
            assert!(!completion(&review, 80).contains("✓"));
        }
    }
    #[test]
    fn animation_is_fixed_width_and_has_a_static_fallback() {
        assert_ne!(
            activity("thinking", 0, 0, true, 80, false),
            activity("thinking", 0, 125, true, 80, false)
        );
        assert_eq!(
            activity("thinking", 0, 0, false, 80, false),
            activity("thinking", 0, 125, false, 80, false)
        );
        let first = activity("responding", 0, 0, true, 80, true);
        let second = activity("responding", 0, 250, true, 80, true);
        assert!(first.contains('▍') || first.contains('▏'), "{first}");
        assert_ne!(first, second);
        assert!(!activity("responding", 0, 0, false, 80, false).contains('▍'));
    }
}
