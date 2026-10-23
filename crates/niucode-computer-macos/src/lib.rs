//! macOS native backend for computer use.
//!
//! Observe uses CoreGraphics window snapshots; input uses CGEvent. Screen
//! Recording and Accessibility are TCC permissions and cannot be granted by
//! the process itself. This backend fails closed when Screen Recording is not
//! preflight-approved.

#![cfg(target_os = "macos")]

use std::time::{SystemTime, UNIX_EPOCH};

use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_graphics::access::ScreenCaptureAccess;
use core_graphics::display::{CGDisplay, CGPoint};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton,
    ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGRect;
use core_graphics::window::{
    copy_window_info, kCGNullWindowID, kCGWindowImageBestResolution,
    kCGWindowImageBoundsIgnoreFraming, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionIncludingWindow, kCGWindowListOptionOnScreenOnly,
};
use niucode_computer::{
    Capture, ComputerAction, ComputerBackend, ImageMeta, KeyChord, Modifier, MouseButton,
    Observation, ObservationId, Point, Rect, ScrollDirection, WindowId, WindowInfo, WindowKind,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

const MAX_WINDOWS: usize = 256;
const OBSERVATION_TTL_MS: u64 = 60_000;

#[derive(Debug, Error)]
pub enum MacError {
    #[error("Screen Recording permission is not granted; enable it in System Settings > Privacy & Security > Screen Recording")]
    ScreenPermission,
    #[error("Accessibility permission is required for input; enable it in System Settings > Privacy & Security > Accessibility")]
    AccessibilityPermission,
    #[error("window {0} is not visible")]
    NotVisible(u64),
    #[error("no visible windows")]
    NoWindows,
    #[error("capture failed")]
    Capture,
    #[error("image encoding failed: {0}")]
    Encode(String),
    #[error("input event failed")]
    Event,
    #[error("unsupported text character {0:?}")]
    UnsupportedText(char),
    #[error("invalid key {0}")]
    InvalidKey(String),
    #[error("computer action rejected: {0}")]
    Action(String),
    #[error("window moved since observation")]
    StaleWindow,
}

pub struct MacBackend;

impl MacBackend {
    pub fn connect(_display: Option<&str>) -> Result<Self, MacError> {
        if !ScreenCaptureAccess.preflight() {
            return Err(MacError::ScreenPermission);
        }
        let _ = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| MacError::Event)?;
        Ok(Self)
    }

    pub fn list_windows(&self) -> Result<Vec<WindowInfo>, MacError> {
        let raw = copy_window_info(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            kCGNullWindowID,
        )
        .ok_or(MacError::NoWindows)?;
        let array: CFArray<CFDictionary<CFString, CFType>> =
            unsafe { TCFType::wrap_under_get_rule(raw.as_concrete_TypeRef()) };
        let mut windows = Vec::new();
        for dict in array.iter().take(MAX_WINDOWS) {
            let Some(id) = number(&dict, "kCGWindowNumber") else {
                continue;
            };
            let Some(bounds) = bounds(&dict) else {
                continue;
            };
            if bounds.size.width < 1.0 || bounds.size.height < 1.0 {
                continue;
            }
            let app = string(&dict, "kCGWindowOwnerName").unwrap_or_default();
            let title = string(&dict, "kCGWindowName").unwrap_or_default();
            let class = (!app.is_empty()).then_some(app.clone());
            windows.push(WindowInfo {
                id: WindowId(id as u64),
                kind: classify(&app, &title),
                app,
                title,
                class,
                bounds: Rect {
                    x: bounds.origin.x.round() as i32,
                    y: bounds.origin.y.round() as i32,
                    width: bounds.size.width.round() as u32,
                    height: bounds.size.height.round() as u32,
                },
            });
        }
        if windows.is_empty() {
            return Err(MacError::NoWindows);
        }
        windows.sort_by_key(|window| (window.bounds.y, window.bounds.x));
        Ok(windows)
    }

    pub fn observe(&self, window: Option<WindowId>) -> Result<Capture, MacError> {
        let windows = self.list_windows()?;
        let selected = match window {
            Some(id) => windows
                .iter()
                .find(|window| window.id == id)
                .cloned()
                .ok_or(MacError::NotVisible(id.0))?,
            None => windows.first().cloned().ok_or(MacError::NoWindows)?,
        };
        let bounds = CGRect::new(
            &CGPoint::new(f64::from(selected.bounds.x), f64::from(selected.bounds.y)),
            &core_graphics::geometry::CGSize::new(
                f64::from(selected.bounds.width),
                f64::from(selected.bounds.height),
            ),
        );
        let image = CGDisplay::screenshot(
            bounds,
            kCGWindowListOptionIncludingWindow,
            selected.id.0 as u32,
            kCGWindowImageBoundsIgnoreFraming | kCGWindowImageBestResolution,
        )
        .ok_or(MacError::Capture)?;
        let png = encode_png(&image)?;
        let hash = hex(&Sha256::digest(&png));
        let now = now_ms();
        Ok(Capture {
            observation: Observation {
                id: ObservationId::new(format!("mac-{hash}")),
                window: selected,
                image: ImageMeta {
                    width: image.width() as u32,
                    height: image.height() as u32,
                    scale_percent: 100,
                    sha256: hash,
                    bytes: png.len() as u64,
                },
                captured_at_ms: now,
                expires_at_ms: now + OBSERVATION_TTL_MS,
            },
            png,
        })
    }

    pub fn perform(
        &self,
        action: &ComputerAction,
        observation: &Observation,
    ) -> Result<(), MacError> {
        observation
            .validate(action, now_ms())
            .map_err(|error| MacError::Action(error.to_string()))?;
        if matches!(action, ComputerAction::Observe { .. }) {
            return Ok(());
        }
        if !accessibility_granted() {
            return Err(MacError::AccessibilityPermission);
        }
        let current = self
            .list_windows()?
            .into_iter()
            .find(|window| window.id == observation.window.id)
            .ok_or(MacError::NotVisible(observation.window.id.0))?;
        if current.bounds != observation.window.bounds {
            return Err(MacError::StaleWindow);
        }
        match action {
            ComputerAction::Observe { .. } => {}
            ComputerAction::Click { point, button, .. } => {
                let location = self.map_point(*point, observation);
                let button = match button {
                    MouseButton::Left => CGMouseButton::Left,
                    MouseButton::Right => CGMouseButton::Right,
                    MouseButton::Middle => CGMouseButton::Center,
                };
                let down = match button {
                    CGMouseButton::Left => CGEventType::LeftMouseDown,
                    CGMouseButton::Right => CGEventType::RightMouseDown,
                    CGMouseButton::Center => CGEventType::OtherMouseDown,
                };
                let up = match button {
                    CGMouseButton::Left => CGEventType::LeftMouseUp,
                    CGMouseButton::Right => CGEventType::RightMouseUp,
                    CGMouseButton::Center => CGEventType::OtherMouseUp,
                };
                self.mouse(down, location, button)?;
                self.mouse(up, location, button)?;
            }
            ComputerAction::Type { text, .. } => self.type_text(text)?,
            ComputerAction::Key { key, .. } => self.key_chord(key)?,
            ComputerAction::Scroll {
                point,
                direction,
                amount,
                ..
            } => {
                let location = self.map_point(*point, observation);
                self.move_pointer(location)?;
                let (vertical, horizontal) = match direction {
                    ScrollDirection::Up => (*amount as i32, 0),
                    ScrollDirection::Down => (-(*amount as i32), 0),
                    ScrollDirection::Left => (0, -(*amount as i32)),
                    ScrollDirection::Right => (0, *amount as i32),
                };
                let event = CGEvent::new_scroll_event(
                    self.source()?,
                    ScrollEventUnit::PIXEL,
                    2,
                    vertical,
                    horizontal,
                    0,
                )
                .map_err(|_| MacError::Event)?;
                event.post(CGEventTapLocation::HID);
            }
        }
        Ok(())
    }

    pub fn emergency_stop(&self) -> Result<(), MacError> {
        for keycode in [56, 59, 58, 55] {
            if let Ok(event) = CGEvent::new_keyboard_event(self.source()?, keycode, false) {
                event.post(CGEventTapLocation::HID);
            }
        }
        for (event_type, button) in [
            (CGEventType::LeftMouseUp, CGMouseButton::Left),
            (CGEventType::RightMouseUp, CGMouseButton::Right),
            (CGEventType::OtherMouseUp, CGMouseButton::Center),
        ] {
            let point = CGPoint::new(0.0, 0.0);
            if let Ok(event) = CGEvent::new_mouse_event(self.source()?, event_type, point, button) {
                event.post(CGEventTapLocation::HID);
            }
        }
        Ok(())
    }

    fn source(&self) -> Result<CGEventSource, MacError> {
        CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| MacError::Event)
    }

    fn map_point(&self, point: Point, observation: &Observation) -> CGPoint {
        let width = f64::from(observation.image.width.max(1));
        let height = f64::from(observation.image.height.max(1));
        let bounds = observation.window.bounds;
        CGPoint::new(
            f64::from(bounds.x) + f64::from(point.x) * f64::from(bounds.width) / width,
            f64::from(bounds.y) + f64::from(point.y) * f64::from(bounds.height) / height,
        )
    }

    fn move_pointer(&self, point: CGPoint) -> Result<(), MacError> {
        self.mouse(CGEventType::MouseMoved, point, CGMouseButton::Left)
    }

    fn mouse(
        &self,
        event_type: CGEventType,
        point: CGPoint,
        button: CGMouseButton,
    ) -> Result<(), MacError> {
        let event = CGEvent::new_mouse_event(self.source()?, event_type, point, button)
            .map_err(|_| MacError::Event)?;
        event.post(CGEventTapLocation::HID);
        Ok(())
    }

    fn type_text(&self, text: &str) -> Result<(), MacError> {
        for character in text.chars() {
            if character.is_control() && !matches!(character, '\n' | '\t') {
                return Err(MacError::UnsupportedText(character));
            }
            let mut buffer = [0u16; 2];
            let encoded = character.encode_utf16(&mut buffer);
            for down in [true, false] {
                let event = CGEvent::new_keyboard_event(self.source()?, 0, down)
                    .map_err(|_| MacError::Event)?;
                event.set_string_from_utf16_unchecked(encoded);
                event.post(CGEventTapLocation::HID);
            }
        }
        Ok(())
    }

    fn key_chord(&self, chord: &KeyChord) -> Result<(), MacError> {
        let (keycode, flags) =
            keycode_for(&chord.key).ok_or_else(|| MacError::InvalidKey(chord.key.clone()))?;
        let mut flags = flags;
        for modifier in &chord.modifiers {
            flags |= match modifier {
                Modifier::Ctrl => CGEventFlags::CGEventFlagControl,
                Modifier::Alt => CGEventFlags::CGEventFlagAlternate,
                Modifier::Shift => CGEventFlags::CGEventFlagShift,
                Modifier::Super => CGEventFlags::CGEventFlagCommand,
            };
        }
        for down in [true, false] {
            let event = CGEvent::new_keyboard_event(self.source()?, keycode, down)
                .map_err(|_| MacError::Event)?;
            event.set_flags(flags);
            event.post(CGEventTapLocation::HID);
        }
        Ok(())
    }
}

impl ComputerBackend for MacBackend {
    fn observe(
        &mut self,
        window: Option<WindowId>,
    ) -> Result<Capture, Box<dyn std::error::Error + Send + Sync>> {
        MacBackend::observe(self, window).map_err(|error| Box::new(error) as _)
    }

    fn perform(
        &mut self,
        action: &ComputerAction,
        observation: &Observation,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        MacBackend::perform(self, action, observation).map_err(|error| Box::new(error) as _)
    }

    fn emergency_stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        MacBackend::emergency_stop(self).map_err(|error| Box::new(error) as _)
    }
}

fn number(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<i64> {
    let value = dict.find(CFString::new(key))?;
    value.downcast::<CFNumber>()?.to_i64()
}

fn string(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<String> {
    let value = dict.find(CFString::new(key))?;
    let string = value.downcast::<CFString>()?;
    Some(string.to_string())
}

fn bounds(dict: &CFDictionary<CFString, CFType>) -> Option<CGRect> {
    let value = dict.find(CFString::new("kCGWindowBounds"))?;
    let bounds: CFDictionary<CFString, CFType> =
        unsafe { TCFType::wrap_under_get_rule(value.as_CFTypeRef() as CFDictionaryRef) };
    Some(CGRect::new(
        &CGPoint::new(number(&bounds, "X")? as f64, number(&bounds, "Y")? as f64),
        &core_graphics::geometry::CGSize::new(
            number(&bounds, "Width")? as f64,
            number(&bounds, "Height")? as f64,
        ),
    ))
}

fn encode_png(image: &core_graphics::image::CGImage) -> Result<Vec<u8>, MacError> {
    let width = image.width() as u32;
    let height = image.height() as u32;
    let data = image.data();
    let bytes = data.bytes();
    let bpp = image.bits_per_pixel() / 8;
    if bpp != 4 {
        return Err(MacError::Capture);
    }
    let row = image.bytes_per_row();
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let offset = y * row + x * 4;
            if offset + 4 > bytes.len() {
                return Err(MacError::Capture);
            }
            rgba.extend_from_slice(&[bytes[offset + 2], bytes[offset + 1], bytes[offset], 255]);
        }
    }
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| MacError::Encode(error.to_string()))?;
        writer
            .write_image_data(&rgba)
            .map_err(|error| MacError::Encode(error.to_string()))?;
    }
    Ok(png)
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

fn accessibility_granted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

fn keycode_for(key: &str) -> Option<(CGKeyCode, CGEventFlags)> {
    let lower = key.to_ascii_lowercase();
    let (code, flags) = match lower.as_str() {
        "return" | "enter" => (36, CGEventFlags::CGEventFlagNull),
        "tab" => (48, CGEventFlags::CGEventFlagNull),
        "escape" | "esc" => (53, CGEventFlags::CGEventFlagNull),
        "space" => (49, CGEventFlags::CGEventFlagNull),
        "backspace" => (51, CGEventFlags::CGEventFlagNull),
        "delete" | "del" => (117, CGEventFlags::CGEventFlagNull),
        "up" => (126, CGEventFlags::CGEventFlagNull),
        "down" => (125, CGEventFlags::CGEventFlagNull),
        "left" => (123, CGEventFlags::CGEventFlagNull),
        "right" => (124, CGEventFlags::CGEventFlagNull),
        "home" => (115, CGEventFlags::CGEventFlagNull),
        "end" => (119, CGEventFlags::CGEventFlagNull),
        "pageup" => (116, CGEventFlags::CGEventFlagNull),
        "pagedown" => (121, CGEventFlags::CGEventFlagNull),
        "f1" => (122, CGEventFlags::CGEventFlagNull),
        "f2" => (120, CGEventFlags::CGEventFlagNull),
        "f3" => (99, CGEventFlags::CGEventFlagNull),
        "f4" => (118, CGEventFlags::CGEventFlagNull),
        "f5" => (96, CGEventFlags::CGEventFlagNull),
        "f6" => (97, CGEventFlags::CGEventFlagNull),
        "f7" => (98, CGEventFlags::CGEventFlagNull),
        "f8" => (100, CGEventFlags::CGEventFlagNull),
        "f9" => (101, CGEventFlags::CGEventFlagNull),
        "f10" => (109, CGEventFlags::CGEventFlagNull),
        "f11" => (103, CGEventFlags::CGEventFlagNull),
        "f12" => (111, CGEventFlags::CGEventFlagNull),
        _ => {
            let character = lower.chars().next()?;
            let code = match character {
                'a' => 0,
                's' => 1,
                'd' => 2,
                'f' => 3,
                'h' => 4,
                'g' => 5,
                'z' => 6,
                'x' => 7,
                'c' => 8,
                'v' => 9,
                'b' => 11,
                'q' => 12,
                'w' => 13,
                'e' => 14,
                'r' => 15,
                'y' => 16,
                't' => 17,
                '1' => 18,
                '2' => 19,
                '3' => 20,
                '4' => 21,
                '6' => 22,
                '5' => 23,
                '9' => 25,
                '7' => 26,
                '8' => 28,
                '0' => 29,
                'o' => 31,
                'u' => 32,
                'i' => 34,
                'p' => 35,
                'l' => 37,
                'j' => 38,
                'k' => 40,
                'n' => 45,
                'm' => 46,
                _ => return None,
            };
            let shifted = key.chars().next().is_some_and(char::is_uppercase);
            (
                code,
                if shifted {
                    CGEventFlags::CGEventFlagShift
                } else {
                    CGEventFlags::CGEventFlagNull
                },
            )
        }
    };
    Some((code, flags))
}

fn classify(app: &str, title: &str) -> WindowKind {
    let haystack = format!("{app} {title}").to_ascii_lowercase();
    const SENSITIVE: &[&str] = &[
        "password",
        "passwd",
        "keychain",
        "1password",
        "bitwarden",
        "keepass",
        "authentication",
        "login",
        "privacy",
    ];
    if SENSITIVE.iter().any(|needle| haystack.contains(needle)) {
        return WindowKind::Sensitive;
    }
    const TERMINALS: &[&str] = &[
        "terminal",
        "iterm",
        "warp",
        "kitty",
        "alacritty",
        "wezterm",
        "hyper",
    ];
    if TERMINALS.iter().any(|needle| haystack.contains(needle)) {
        return WindowKind::Terminal;
    }
    WindowKind::Normal
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_terminals_and_sensitive_windows() {
        assert_eq!(classify("iTerm2", "shell"), WindowKind::Terminal);
        assert_eq!(classify("1Password", "Unlock"), WindowKind::Sensitive);
        assert_eq!(classify("Notes", "todo"), WindowKind::Normal);
    }

    #[test]
    fn named_keys_map() {
        assert_eq!(keycode_for("return").unwrap().0, 36);
        assert_eq!(keycode_for("left").unwrap().0, 123);
        assert!(keycode_for("nope").is_none());
    }
}
