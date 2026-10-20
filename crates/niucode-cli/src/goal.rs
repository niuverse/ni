//! Goal evaluator used by `/goal run`.
//!
//! The evaluator is a separate provider call with no tools. It sees only the
//! condition and the actor's last report, so it cannot be steered by the full
//! conversation or mutate state.

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use niucode_provider::{ChatMessage, CompletionRequest, Provider, StreamEvent};

pub const MAX_GOAL_TURNS: usize = 6;
const EVALUATOR_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalStatus {
    Met,
    NotMet,
    Impossible,
}

impl GoalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Met => "met",
            Self::NotMet => "not_met",
            Self::Impossible => "impossible",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalVerdict {
    pub status: GoalStatus,
    pub reason: String,
}

pub async fn evaluate(
    provider: &dyn Provider,
    model: &str,
    goal: &str,
    last_report: &str,
) -> Result<GoalVerdict> {
    let mut request = CompletionRequest::new(model.to_string());
    request.messages = vec![
        ChatMessage::system(
            "You are a strict goal evaluator. Return ONLY a JSON object with keys \
             \"status\" (\"met\", \"not_met\", or \"impossible\") and \"reason\" (one sentence). \
             Judge the condition against the last report. Do not use tools. Do not add prose.",
        ),
        ChatMessage::user(format!(
            "Goal / completion condition:\n{goal}\n\nLast report from the worker:\n{last_report}"
        )),
    ];
    request.max_tokens = Some(200);

    let text = tokio::time::timeout(
        std::time::Duration::from_secs(EVALUATOR_TIMEOUT_SECS),
        async {
            let mut stream = provider.stream(request).await?;
            let mut text = String::new();
            while let Some(event) = stream.next().await {
                if let StreamEvent::TextDelta(delta) = event? {
                    text.push_str(&delta);
                }
            }
            Ok::<_, anyhow::Error>(text)
        },
    )
    .await
    .context("goal evaluator timed out")??;
    parse_verdict(&text)
}

pub fn parse_verdict(text: &str) -> Result<GoalVerdict> {
    let start = text
        .find('{')
        .context("evaluator returned no JSON object")?;
    let end = text
        .rfind('}')
        .context("evaluator returned no complete JSON")?;
    if end < start {
        bail!("evaluator returned malformed JSON");
    }
    let value: serde_json::Value =
        serde_json::from_str(&text[start..=end]).context("decode evaluator JSON")?;
    let status = match value.get("status").and_then(serde_json::Value::as_str) {
        Some("met") => GoalStatus::Met,
        Some("not_met") => GoalStatus::NotMet,
        Some("impossible") => GoalStatus::Impossible,
        Some(other) => bail!("evaluator returned unknown status {other:?}"),
        None => bail!("evaluator returned no status"),
    };
    let reason = value
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("no reason provided")
        .trim()
        .chars()
        .take(500)
        .collect::<String>();
    Ok(GoalVerdict { status, reason })
}

pub fn start_prompt(goal: &str) -> String {
    format!(
        "Start working toward the active goal now. Goal: {goal}\n\
         Work directly, verify your changes, and report what remains."
    )
}

pub fn continue_prompt(goal: &str) -> String {
    format!(
        "Continue working toward the active goal. Goal: {goal}\n\
         Do not stop while the condition is unmet. Report what changed and what remains."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_and_fenced_verdicts() {
        let plain =
            parse_verdict(r#"{"status":"not_met","reason":"tests still fail"}"#).expect("plain");
        assert_eq!(plain.status, GoalStatus::NotMet);
        assert_eq!(plain.reason, "tests still fail");

        let fenced =
            parse_verdict("```json\n{\"status\":\"met\",\"reason\":\"all checks pass\"}\n```")
                .expect("fenced");
        assert_eq!(fenced.status, GoalStatus::Met);
    }

    #[test]
    fn rejects_unknown_or_missing_status() {
        assert!(parse_verdict(r#"{"status":"maybe","reason":"x"}"#).is_err());
        assert!(parse_verdict(r#"{"reason":"x"}"#).is_err());
        assert!(parse_verdict("not json").is_err());
    }

    #[test]
    fn goal_prompts_name_the_goal() {
        assert!(start_prompt("all tests pass").contains("all tests pass"));
        assert!(continue_prompt("all tests pass").contains("all tests pass"));
    }
}
