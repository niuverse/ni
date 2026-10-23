//! Computer-use protocol, permissions, and observation contracts.
//!
//! This crate deliberately contains no OS backend. The first backend is Linux
//! X11; macOS is a second milestone. Backends consume these types so permission
//! checks, stale-observation rejection, and audit metadata stay identical.
//!
//! The protocol is observation-bound: an input action must name the observation
//! it was derived from, the target window, and image-relative coordinates. The
//! runtime rechecks window geometry and expiration before dispatch.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Session-level computer permission. `off` is the only default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ComputerPolicy {
    #[default]
    Off,
    Observe,
    Control,
    Autonomous,
}

impl ComputerPolicy {
    pub const fn allows_observation(self) -> bool {
        !matches!(self, Self::Off)
    }

    pub const fn allows_input(self) -> bool {
        matches!(self, Self::Control | Self::Autonomous)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Observe => "observe",
            Self::Control => "control",
            Self::Autonomous => "autonomous",
        }
    }
}

impl fmt::Display for ComputerPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ComputerPolicy {
    type Err = ParseComputerPolicyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" => Ok(Self::Off),
            "observe" => Ok(Self::Observe),
            "control" => Ok(Self::Control),
            "autonomous" => Ok(Self::Autonomous),
            _ => Err(ParseComputerPolicyError {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("computer policy must be one of off, observe, control, autonomous (got {value:?})")]
pub struct ParseComputerPolicyError {
    pub value: String,
}

/// The outcome of applying policy to one action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    Allowed,
    NeedsApproval,
    Denied,
}

/// Backend classification of the target window. It is supplied by the OS
/// backend, not by the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    #[default]
    Normal,
    /// A terminal or console: desktop input there could bypass the shell tool's
    /// approval, so autonomous mode still asks.
    Terminal,
    /// Password managers, authentication dialogs, and security settings.
    Sensitive,
}

/// Why an input action is risky. Used to keep the autonomous red lines in one
/// place instead of duplicating them across backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputRisk {
    Ordinary,
    Terminal,
    SensitiveWindow,
    SystemShortcut,
}

impl InputRisk {
    pub const fn is_ordinary(self) -> bool {
        matches!(self, Self::Ordinary)
    }
}

/// Resolve policy for one action against a backend-classified window.
pub fn decide(
    policy: ComputerPolicy,
    action: &ComputerAction,
    window: WindowKind,
) -> PolicyDecision {
    if matches!(action, ComputerAction::Observe { .. }) {
        return if policy.allows_observation() {
            PolicyDecision::Allowed
        } else {
            PolicyDecision::Denied
        };
    }
    if !policy.allows_input() {
        return PolicyDecision::Denied;
    }
    if policy == ComputerPolicy::Control {
        return PolicyDecision::NeedsApproval;
    }
    if action.risk(window).is_ordinary() {
        PolicyDecision::Allowed
    } else {
        PolicyDecision::NeedsApproval
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObservationId(String);

impl ObservationId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ObservationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WindowId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: WindowId,
    pub app: String,
    pub title: String,
    pub class: Option<String>,
    pub kind: WindowKind,
    pub bounds: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageMeta {
    pub width: u32,
    pub height: u32,
    /// Capture scale in percent (100 = native pixels). Backends map image
    /// coordinates back to OS coordinates.
    pub scale_percent: u32,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub window: WindowInfo,
    pub image: ImageMeta,
    pub captured_at_ms: u64,
    pub expires_at_ms: u64,
}

impl Observation {
    /// Reject an action that was derived from a different, moved, or expired
    /// observation. This is the safety boundary, not a hint.
    pub fn validate(&self, action: &ComputerAction, now_ms: u64) -> Result<(), ActionError> {
        if self.id.as_str().is_empty() {
            return Err(ActionError::InvalidObservation);
        }
        if now_ms >= self.expires_at_ms {
            return Err(ActionError::ObservationExpired);
        }
        if let Some(observation_id) = action.observation_id() {
            if observation_id != &self.id {
                return Err(ActionError::StaleObservation);
            }
        }
        if let Some(window_id) = action.window_id() {
            if window_id != self.window.id {
                return Err(ActionError::WindowMismatch);
            }
        }
        if let Some(point) = action.point() {
            if point.x < 0
                || point.y < 0
                || point.x as u64 >= u64::from(self.image.width)
                || point.y as u64 >= u64::from(self.image.height)
            {
                return Err(ActionError::CoordinateOutOfBounds);
            }
        }
        match action {
            ComputerAction::Type { text, .. } if text.trim().is_empty() => {
                Err(ActionError::EmptyText)
            }
            ComputerAction::Key { key, .. } if key.key.trim().is_empty() => {
                Err(ActionError::EmptyKey)
            }
            ComputerAction::Scroll { amount, .. } if *amount == 0 || *amount > MAX_SCROLL_STEPS => {
                Err(ActionError::InvalidScroll)
            }
            _ => Ok(()),
        }
    }
}

pub const MAX_SCROLL_STEPS: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    #[default]
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Super,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyChord {
    #[serde(default)]
    pub modifiers: Vec<Modifier>,
    pub key: String,
}

impl KeyChord {
    pub fn new(modifiers: Vec<Modifier>, key: impl Into<String>) -> Self {
        Self {
            modifiers,
            key: key.into(),
        }
    }

    /// Red-line detection is backend-independent and intentionally narrow:
    /// only OS-level combinations, not normal application shortcuts.
    pub fn is_system_shortcut(&self) -> bool {
        let key = self.key.trim().to_ascii_lowercase();
        let ctrl = self.modifiers.contains(&Modifier::Ctrl);
        let alt = self.modifiers.contains(&Modifier::Alt);
        let super_key = self.modifiers.contains(&Modifier::Super);
        super_key
            || (alt && key == "f4")
            || (ctrl && alt && matches!(key.as_str(), "delete" | "escape"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

/// One model-proposed computer action. Input variants carry the observation
/// token and window they were derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ComputerAction {
    Observe {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window_id: Option<WindowId>,
    },
    Click {
        observation_id: ObservationId,
        window_id: WindowId,
        point: Point,
        #[serde(default)]
        button: MouseButton,
    },
    Type {
        observation_id: ObservationId,
        window_id: WindowId,
        text: String,
    },
    Key {
        observation_id: ObservationId,
        window_id: WindowId,
        key: KeyChord,
    },
    Scroll {
        observation_id: ObservationId,
        window_id: WindowId,
        point: Point,
        direction: ScrollDirection,
        amount: u32,
    },
}

impl ComputerAction {
    pub fn observation_id(&self) -> Option<&ObservationId> {
        match self {
            Self::Observe { .. } => None,
            Self::Click { observation_id, .. }
            | Self::Type { observation_id, .. }
            | Self::Key { observation_id, .. }
            | Self::Scroll { observation_id, .. } => Some(observation_id),
        }
    }

    pub fn window_id(&self) -> Option<WindowId> {
        match self {
            Self::Observe { window_id } => *window_id,
            Self::Click { window_id, .. }
            | Self::Type { window_id, .. }
            | Self::Key { window_id, .. }
            | Self::Scroll { window_id, .. } => Some(*window_id),
        }
    }

    pub fn point(&self) -> Option<Point> {
        match self {
            Self::Click { point, .. } | Self::Scroll { point, .. } => Some(*point),
            _ => None,
        }
    }

    pub fn risk(&self, window: WindowKind) -> InputRisk {
        match window {
            WindowKind::Terminal => return InputRisk::Terminal,
            WindowKind::Sensitive => return InputRisk::SensitiveWindow,
            WindowKind::Normal => {}
        }
        if let Self::Key { key, .. } = self {
            if key.is_system_shortcut() {
                return InputRisk::SystemShortcut;
            }
        }
        InputRisk::Ordinary
    }
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("computer policy is off; arm the session first")]
    PolicyOff,
    #[error("unknown observation {0}")]
    UnknownObservation(String),
    #[error(transparent)]
    Action(#[from] ActionError),
}

#[derive(Debug, Error)]
pub enum InputError {
    #[error("missing or invalid {0}")]
    Field(&'static str),
    #[error("unsupported computer action {0:?}")]
    Action(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActionError {
    #[error("observation id is empty")]
    InvalidObservation,
    #[error("observation had expired; take a new observation")]
    ObservationExpired,
    #[error("observation token does not match this action")]
    StaleObservation,
    #[error("action targets a different window than the observation")]
    WindowMismatch,
    #[error("point is outside the observed image")]
    CoordinateOutOfBounds,
    #[error("typed text is empty")]
    EmptyText,
    #[error("key name is empty")]
    EmptyKey,
    #[error("scroll amount must be between 1 and {MAX_SCROLL_STEPS}")]
    InvalidScroll,
}

/// One observation plus the PNG used to derive it. Backends return this so the
/// runtime can attach the image to the provider request without re-capturing.
pub struct Capture {
    pub observation: Observation,
    pub png: Vec<u8>,
}

/// OS backend contract. Errors are boxed so the protocol crate stays free of
/// platform dependencies; the agent surfaces them as tool failures.
pub trait ComputerBackend: Send {
    fn observe(
        &mut self,
        window: Option<WindowId>,
    ) -> Result<Capture, Box<dyn std::error::Error + Send + Sync>>;

    fn perform(
        &mut self,
        action: &ComputerAction,
        observation: &Observation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Release held input and stop accepting actions. Backends should make
    /// this best-effort; the session is disarmed regardless.
    fn emergency_stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

/// JSON schema advertised to the model. The model never sees window kinds or
/// backend-specific handles; it only proposes actions and purposes.
pub fn tool_spec() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["observe", "click", "type", "key", "scroll"]
            },
            "purpose": {"type": "string", "description": "Why this action is needed"},
            "window_id": {"type": "integer", "minimum": 1},
            "observation_id": {"type": "string"},
            "x": {"type": "integer"},
            "y": {"type": "integer"},
            "button": {"type": "string", "enum": ["left", "right", "middle"]},
            "text": {"type": "string"},
            "key": {"type": "string"},
            "modifiers": {"type": "array", "items": {"type": "string", "enum": ["ctrl", "alt", "shift", "super"]}},
            "direction": {"type": "string", "enum": ["up", "down", "left", "right"]},
            "amount": {"type": "integer", "minimum": 1, "maximum": MAX_SCROLL_STEPS}
        },
        "required": ["action"]
    })
}

/// Parse a model tool call into a typed action. Unknown fields are ignored;
/// missing required fields fail with a stable message.
pub fn parse_action(input: &serde_json::Value) -> Result<ComputerAction, InputError> {
    let action = input
        .get("action")
        .and_then(serde_json::Value::as_str)
        .ok_or(InputError::Field("action"))?;
    match action {
        "observe" => Ok(ComputerAction::Observe {
            window_id: optional_u64(input, "window_id")?.map(WindowId),
        }),
        "click" => Ok(ComputerAction::Click {
            observation_id: observation_id(input)?,
            window_id: window_id(input)?,
            point: point(input)?,
            button: match input.get("button").and_then(serde_json::Value::as_str) {
                Some("right") => MouseButton::Right,
                Some("middle") => MouseButton::Middle,
                _ => MouseButton::Left,
            },
        }),
        "type" => Ok(ComputerAction::Type {
            observation_id: observation_id(input)?,
            window_id: window_id(input)?,
            text: required_str(input, "text")?.to_string(),
        }),
        "key" => Ok(ComputerAction::Key {
            observation_id: observation_id(input)?,
            window_id: window_id(input)?,
            key: KeyChord {
                modifiers: parse_modifiers(input)?,
                key: required_str(input, "key")?.to_string(),
            },
        }),
        "scroll" => Ok(ComputerAction::Scroll {
            observation_id: observation_id(input)?,
            window_id: window_id(input)?,
            point: point(input)?,
            direction: match required_str(input, "direction")? {
                "down" => ScrollDirection::Down,
                "left" => ScrollDirection::Left,
                "right" => ScrollDirection::Right,
                "up" => ScrollDirection::Up,
                other => {
                    return Err(InputError::Field(match other {
                        "down" | "left" | "right" => "direction",
                        _ => "direction",
                    }))
                }
            },
            amount: input
                .get("amount")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or(InputError::Field("amount"))?,
        }),
        other => Err(InputError::Action(other.to_string())),
    }
}

/// In-memory session state for computer use. The event log is the durable
/// source; observations expire quickly and are not persisted.
#[derive(Debug, Default)]
pub struct ComputerSession {
    policy: ComputerPolicy,
    armed: bool,
    observations: std::collections::HashMap<String, Observation>,
}

impl ComputerSession {
    pub fn new(policy: ComputerPolicy) -> Self {
        Self {
            policy,
            armed: false,
            observations: std::collections::HashMap::new(),
        }
    }

    pub fn policy(&self) -> ComputerPolicy {
        self.policy
    }

    pub fn set_policy(&mut self, policy: ComputerPolicy) {
        self.policy = policy;
        if policy == ComputerPolicy::Off {
            self.armed = false;
        }
    }

    pub fn is_armed(&self) -> bool {
        self.armed
    }

    pub fn arm(&mut self) -> Result<(), SessionError> {
        if self.policy == ComputerPolicy::Off {
            return Err(SessionError::PolicyOff);
        }
        self.armed = true;
        Ok(())
    }

    pub fn disarm(&mut self) {
        self.armed = false;
    }

    pub fn remember(&mut self, observation: Observation) {
        self.observations
            .insert(observation.id.as_str().to_string(), observation);
        if self.observations.len() > 8 {
            let oldest = self
                .observations
                .values()
                .min_by_key(|observation| observation.captured_at_ms)
                .map(|observation| observation.id.as_str().to_string());
            if let Some(oldest) = oldest {
                self.observations.remove(&oldest);
            }
        }
    }

    pub fn observation(&self, id: &ObservationId) -> Result<&Observation, SessionError> {
        self.observations
            .get(id.as_str())
            .ok_or_else(|| SessionError::UnknownObservation(id.to_string()))
    }

    /// Resolve policy for an action, using the observation's window kind for
    /// input actions. Unarmed sessions allow nothing.
    pub fn decide(&self, action: &ComputerAction) -> Result<PolicyDecision, SessionError> {
        if !self.armed {
            return Ok(PolicyDecision::Denied);
        }
        match action {
            ComputerAction::Observe { .. } => Ok(decide(self.policy, action, WindowKind::Normal)),
            _ => {
                let observation =
                    self.observation(action.observation_id().ok_or(SessionError::PolicyOff)?)?;
                Ok(decide(self.policy, action, observation.window.kind))
            }
        }
    }

    pub fn validate(
        &self,
        action: &ComputerAction,
        now_ms: u64,
    ) -> Result<&Observation, SessionError> {
        let observation =
            self.observation(action.observation_id().ok_or(SessionError::PolicyOff)?)?;
        observation.validate(action, now_ms)?;
        Ok(observation)
    }
}

fn required_str<'a>(
    input: &'a serde_json::Value,
    key: &'static str,
) -> Result<&'a str, InputError> {
    input
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or(InputError::Field(key))
}

fn optional_u64(input: &serde_json::Value, key: &'static str) -> Result<Option<u64>, InputError> {
    match input.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or(InputError::Field(key)),
    }
}

fn window_id(input: &serde_json::Value) -> Result<WindowId, InputError> {
    optional_u64(input, "window_id")?
        .map(WindowId)
        .ok_or(InputError::Field("window_id"))
}

fn observation_id(input: &serde_json::Value) -> Result<ObservationId, InputError> {
    let value = required_str(input, "observation_id")?;
    if value.is_empty() {
        return Err(InputError::Field("observation_id"));
    }
    Ok(ObservationId::new(value))
}

fn point(input: &serde_json::Value) -> Result<Point, InputError> {
    let x = input
        .get("x")
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or(InputError::Field("x"))?;
    let y = input
        .get("y")
        .and_then(serde_json::Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or(InputError::Field("y"))?;
    Ok(Point::new(x, y))
}

fn parse_modifiers(input: &serde_json::Value) -> Result<Vec<Modifier>, InputError> {
    let mut modifiers = Vec::new();
    for value in input
        .get("modifiers")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(value) = value.as_str() else {
            return Err(InputError::Field("modifiers"));
        };
        modifiers.push(match value {
            "ctrl" => Modifier::Ctrl,
            "alt" => Modifier::Alt,
            "shift" => Modifier::Shift,
            "super" => Modifier::Super,
            _ => return Err(InputError::Field("modifiers")),
        });
    }
    Ok(modifiers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation() -> Observation {
        Observation {
            id: ObservationId::new("obs-1"),
            window: WindowInfo {
                id: WindowId(42),
                app: "editor".into(),
                title: "notes.txt".into(),
                class: Some("Editor".into()),
                kind: WindowKind::Normal,
                bounds: Rect {
                    x: 10,
                    y: 20,
                    width: 100,
                    height: 50,
                },
            },
            image: ImageMeta {
                width: 100,
                height: 50,
                scale_percent: 100,
                sha256: "abc".into(),
                bytes: 7,
            },
            captured_at_ms: 1_000,
            expires_at_ms: 61_000,
        }
    }

    fn click() -> ComputerAction {
        ComputerAction::Click {
            observation_id: ObservationId::new("obs-1"),
            window_id: WindowId(42),
            point: Point::new(10, 20),
            button: MouseButton::Left,
        }
    }

    #[test]
    fn policy_parsing_and_default_are_strict() {
        assert_eq!(ComputerPolicy::default(), ComputerPolicy::Off);
        for (value, expected) in [
            ("off", ComputerPolicy::Off),
            ("observe", ComputerPolicy::Observe),
            ("control", ComputerPolicy::Control),
            ("autonomous", ComputerPolicy::Autonomous),
            ("  CONTROL ", ComputerPolicy::Control),
        ] {
            assert_eq!(value.parse::<ComputerPolicy>(), Ok(expected), "{value}");
        }
        assert!("auto".parse::<ComputerPolicy>().is_err());
    }

    #[test]
    fn policy_matrix_keeps_input_gated() {
        let observe = ComputerAction::Observe { window_id: None };
        assert_eq!(
            decide(ComputerPolicy::Off, &observe, WindowKind::Normal),
            PolicyDecision::Denied
        );
        for policy in [
            ComputerPolicy::Observe,
            ComputerPolicy::Control,
            ComputerPolicy::Autonomous,
        ] {
            assert_eq!(
                decide(policy, &observe, WindowKind::Normal),
                PolicyDecision::Allowed
            );
        }
        assert_eq!(
            decide(ComputerPolicy::Observe, &click(), WindowKind::Normal),
            PolicyDecision::Denied
        );
        assert_eq!(
            decide(ComputerPolicy::Control, &click(), WindowKind::Normal),
            PolicyDecision::NeedsApproval
        );
        assert_eq!(
            decide(ComputerPolicy::Autonomous, &click(), WindowKind::Normal),
            PolicyDecision::Allowed
        );
    }

    #[test]
    fn autonomous_red_lines_require_approval() {
        for window in [WindowKind::Terminal, WindowKind::Sensitive] {
            assert_eq!(
                decide(ComputerPolicy::Autonomous, &click(), window),
                PolicyDecision::NeedsApproval
            );
        }
        let system = ComputerAction::Key {
            observation_id: ObservationId::new("obs-1"),
            window_id: WindowId(42),
            key: KeyChord::new(vec![Modifier::Super], "l"),
        };
        assert_eq!(
            decide(ComputerPolicy::Autonomous, &system, WindowKind::Normal),
            PolicyDecision::NeedsApproval
        );
        let ordinary = ComputerAction::Key {
            observation_id: ObservationId::new("obs-1"),
            window_id: WindowId(42),
            key: KeyChord::new(vec![Modifier::Ctrl], "l"),
        };
        assert_eq!(
            decide(ComputerPolicy::Autonomous, &ordinary, WindowKind::Normal),
            PolicyDecision::Allowed
        );
        assert!(KeyChord::new(vec![Modifier::Alt], "F4").is_system_shortcut());
        assert!(KeyChord::new(vec![Modifier::Ctrl, Modifier::Alt], "Delete").is_system_shortcut());
        assert!(!KeyChord::new(vec![Modifier::Ctrl], "c").is_system_shortcut());
    }

    #[test]
    fn actions_are_bound_to_the_observation() {
        let observation = observation();
        assert_eq!(observation.validate(&click(), 5_000), Ok(()));

        let mut stale = click();
        if let ComputerAction::Click { observation_id, .. } = &mut stale {
            *observation_id = ObservationId::new("obs-2");
        }
        assert_eq!(
            observation.validate(&stale, 5_000),
            Err(ActionError::StaleObservation)
        );

        let mut wrong_window = click();
        if let ComputerAction::Click { window_id, .. } = &mut wrong_window {
            *window_id = WindowId(7);
        }
        assert_eq!(
            observation.validate(&wrong_window, 5_000),
            Err(ActionError::WindowMismatch)
        );

        let mut outside = click();
        if let ComputerAction::Click { point, .. } = &mut outside {
            *point = Point::new(100, 0);
        }
        assert_eq!(
            observation.validate(&outside, 5_000),
            Err(ActionError::CoordinateOutOfBounds)
        );

        assert_eq!(
            observation.validate(&click(), 61_000),
            Err(ActionError::ObservationExpired)
        );
    }

    #[test]
    fn content_and_scroll_limits_are_validated() {
        let observation = observation();
        let empty_type = ComputerAction::Type {
            observation_id: ObservationId::new("obs-1"),
            window_id: WindowId(42),
            text: "   ".into(),
        };
        assert_eq!(
            observation.validate(&empty_type, 5_000),
            Err(ActionError::EmptyText)
        );
        let zero_scroll = ComputerAction::Scroll {
            observation_id: ObservationId::new("obs-1"),
            window_id: WindowId(42),
            point: Point::new(1, 1),
            direction: ScrollDirection::Down,
            amount: 0,
        };
        assert_eq!(
            observation.validate(&zero_scroll, 5_000),
            Err(ActionError::InvalidScroll)
        );
    }

    #[test]
    fn tool_input_parses_actions_and_modifiers() {
        let observe =
            parse_action(&serde_json::json!({"action":"observe","window_id":42})).unwrap();
        assert_eq!(
            observe,
            ComputerAction::Observe {
                window_id: Some(WindowId(42))
            }
        );
        let click = parse_action(&serde_json::json!({
            "action":"click","observation_id":"obs-1","window_id":42,"x":3,"y":4,"button":"right"
        }))
        .unwrap();
        assert!(matches!(
            click,
            ComputerAction::Click {
                button: MouseButton::Right,
                ..
            }
        ));
        let key = parse_action(&serde_json::json!({
            "action":"key","observation_id":"obs-1","window_id":42,"key":"l","modifiers":["ctrl","shift"]
        }))
        .unwrap();
        assert!(matches!(
            key,
            ComputerAction::Key { key: KeyChord { ref modifiers, .. }, .. }
                if modifiers == &[Modifier::Ctrl, Modifier::Shift]
        ));
        assert!(parse_action(&serde_json::json!({"action":"click"})).is_err());
        assert!(parse_action(&serde_json::json!({"action":"teleport"})).is_err());
        let spec = tool_spec();
        assert_eq!(spec["properties"]["action"]["enum"][0], "observe");
    }

    #[test]
    fn session_requires_arm_and_known_observation() {
        let mut session = ComputerSession::new(ComputerPolicy::Control);
        let observe = ComputerAction::Observe { window_id: None };
        assert_eq!(session.decide(&observe).unwrap(), PolicyDecision::Denied);
        session.arm().unwrap();
        assert_eq!(session.decide(&observe).unwrap(), PolicyDecision::Allowed);
        assert!(matches!(
            session.decide(&click()),
            Err(SessionError::UnknownObservation(_))
        ));
        session.remember(observation());
        assert_eq!(
            session.decide(&click()).unwrap(),
            PolicyDecision::NeedsApproval
        );
        assert!(session.validate(&click(), 5_000).is_ok());
        assert!(matches!(
            session.validate(&click(), 99_000),
            Err(SessionError::Action(ActionError::ObservationExpired))
        ));
        let mut off = ComputerSession::new(ComputerPolicy::Off);
        assert!(matches!(off.arm(), Err(SessionError::PolicyOff)));
    }

    #[test]
    fn autonomous_uses_the_observation_window_kind() {
        let mut session = ComputerSession::new(ComputerPolicy::Autonomous);
        session.arm().unwrap();
        let mut terminal = observation();
        terminal.window.kind = WindowKind::Terminal;
        session.remember(terminal);
        assert_eq!(
            session.decide(&click()).unwrap(),
            PolicyDecision::NeedsApproval
        );
    }

    #[test]
    fn actions_serialize_with_the_expected_tag() {
        let value = serde_json::to_value(click()).unwrap();
        assert_eq!(value["action"], "click");
        assert_eq!(value["window_id"], 42);
        assert_eq!(value["observation_id"], "obs-1");

        let observe = serde_json::to_value(ComputerAction::Observe { window_id: None }).unwrap();
        assert_eq!(observe["action"], "observe");
        assert!(observe.get("window_id").is_none());
    }
}
