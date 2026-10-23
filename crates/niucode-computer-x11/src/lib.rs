//! Linux X11 observe backend for computer use.
//!
//! First slice: connect to an X display, enumerate viewable windows, capture a
//! window as PNG, and return an observation bound to its geometry/hash. Input
//! injection is intentionally not implemented here.

#![cfg(target_os = "linux")]

use std::time::{SystemTime, UNIX_EPOCH};

use niucode_computer::{
    ComputerAction, ImageMeta, KeyChord, Modifier, MouseButton, Observation, ObservationId, Point,
    Rect, ScrollDirection, WindowId, WindowInfo, WindowKind,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use thiserror::Error;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, ImageFormat, InputFocus, MapState, MOTION_NOTIFY_EVENT,
};
use x11rb::protocol::xtest;
use x11rb::rust_connection::RustConnection;

const MAX_WINDOWS: usize = 256;
const OBSERVATION_TTL_MS: u64 = 60_000;

#[derive(Debug, Error)]
pub enum X11Error {
    #[error("connect to X display: {0}")]
    Connect(String),
    #[error("X11 protocol error: {0}")]
    Protocol(String),
    #[error("window {0} is not viewable")]
    NotViewable(u64),
    #[error("window {0} has no usable geometry")]
    EmptyGeometry(u64),
    #[error("capture failed: {0}")]
    Capture(String),
    #[error("png encode failed: {0}")]
    Png(String),
    #[error("computer action rejected: {0}")]
    Action(String),
    #[error("window moved since observation")]
    StaleWindow,
    #[error("unsupported text character {0:?}")]
    UnsupportedText(char),
    #[error("invalid key {0}")]
    InvalidKey(String),
}

pub struct X11Backend {
    connection: RustConnection,
    screen: usize,
    root: u32,
    atoms: Atoms,
    keymap: HashMap<u32, (u8, bool)>,
}

struct Atoms {
    net_wm_name: u32,
    utf8_string: u32,
    wm_class: u32,
}

pub struct Capture {
    pub observation: Observation,
    pub png: Vec<u8>,
}

impl X11Backend {
    pub fn connect(display: Option<&str>) -> Result<Self, X11Error> {
        let (connection, screen) =
            x11rb::connect(display).map_err(|error| X11Error::Connect(error.to_string()))?;
        let root = connection.setup().roots[screen].root;
        let atoms = Atoms {
            net_wm_name: intern(&connection, b"_NET_WM_NAME")?,
            utf8_string: intern(&connection, b"UTF8_STRING")?,
            wm_class: intern(&connection, b"WM_CLASS")?,
        };
        let keymap = build_keymap(&connection)?;
        Ok(Self {
            connection,
            screen,
            root,
            atoms,
            keymap,
        })
    }

    pub fn root(&self) -> WindowId {
        WindowId(u64::from(self.root))
    }

    pub fn list_windows(&self) -> Result<Vec<WindowInfo>, X11Error> {
        let tree = self
            .connection
            .query_tree(self.root)
            .map_err(protocol)?
            .reply()
            .map_err(protocol)?;
        let mut windows = Vec::new();
        for window in tree.children.into_iter().take(MAX_WINDOWS) {
            if let Some(info) = self.window_info(window) {
                windows.push(info);
            }
        }
        windows.sort_by_key(|window| (window.bounds.y, window.bounds.x));
        Ok(windows)
    }

    /// Capture `window` (or the first viewable window) as PNG.
    pub fn observe(&self, window: Option<WindowId>) -> Result<Capture, X11Error> {
        let window = match window {
            Some(window) => window.0 as u32,
            None => self
                .list_windows()?
                .first()
                .map(|window| window.id.0 as u32)
                .ok_or(X11Error::NotViewable(0))?,
        };
        let info = self
            .window_info(window)
            .ok_or(X11Error::NotViewable(u64::from(window)))?;
        let geometry = self
            .connection
            .get_geometry(window)
            .map_err(protocol)?
            .reply()
            .map_err(protocol)?;
        if geometry.width == 0 || geometry.height == 0 {
            return Err(X11Error::EmptyGeometry(u64::from(window)));
        }
        let png = self.capture_png(window, geometry.width, geometry.height)?;
        let hash = hex(&Sha256::digest(&png));
        let captured = now_ms();
        let observation = Observation {
            id: ObservationId::new(format!("x11-{hash}")),
            window: WindowInfo {
                id: WindowId(u64::from(window)),
                ..info
            },
            image: ImageMeta {
                width: u32::from(geometry.width),
                height: u32::from(geometry.height),
                scale_percent: 100,
                sha256: hash,
                bytes: png.len() as u64,
            },
            captured_at_ms: captured,
            expires_at_ms: captured + OBSERVATION_TTL_MS,
        };
        Ok(Capture { observation, png })
    }

    /// Execute one observation-bound action. The observation is validated
    /// before any X request; the live window geometry must still match.
    pub fn perform(
        &self,
        action: &ComputerAction,
        observation: &Observation,
    ) -> Result<(), X11Error> {
        observation
            .validate(action, now_ms())
            .map_err(|error| X11Error::Action(error.to_string()))?;
        if matches!(action, ComputerAction::Observe { .. }) {
            return Ok(());
        }
        let window = observation.window.id.0 as u32;
        let current = self
            .window_info(window)
            .ok_or_else(|| X11Error::NotViewable(u64::from(window)))?;
        if current.bounds != observation.window.bounds {
            return Err(X11Error::StaleWindow);
        }
        if matches!(
            action,
            ComputerAction::Type { .. } | ComputerAction::Key { .. }
        ) {
            self.connection
                .set_input_focus(InputFocus::PARENT, window, 0u32)
                .map_err(protocol)?
                .check()
                .map_err(protocol)?;
        }
        match action {
            ComputerAction::Observe { .. } => {}
            ComputerAction::Click { point, button, .. } => {
                self.move_pointer(*point, observation)?;
                let detail = match button {
                    MouseButton::Left => 1,
                    MouseButton::Middle => 2,
                    MouseButton::Right => 3,
                };
                self.button(detail)?;
            }
            ComputerAction::Type { text, .. } => self.type_text(text)?,
            ComputerAction::Key { key, .. } => self.key_chord(key)?,
            ComputerAction::Scroll {
                point,
                direction,
                amount,
                ..
            } => {
                self.move_pointer(*point, observation)?;
                let detail = match direction {
                    ScrollDirection::Up => 4,
                    ScrollDirection::Down => 5,
                    ScrollDirection::Left => 6,
                    ScrollDirection::Right => 7,
                };
                for _ in 0..*amount {
                    self.button(detail)?;
                }
            }
        }
        self.connection.flush().map_err(protocol)?;
        Ok(())
    }

    fn move_pointer(&self, point: Point, observation: &Observation) -> Result<(), X11Error> {
        let width = u64::from(observation.image.width).max(1);
        let height = u64::from(observation.image.height).max(1);
        let bounds = observation.window.bounds;
        let root_x =
            i64::from(bounds.x) + (i64::from(point.x) * i64::from(bounds.width) / width as i64);
        let root_y =
            i64::from(bounds.y) + (i64::from(point.y) * i64::from(bounds.height) / height as i64);
        let root_x = i16::try_from(root_x.clamp(0, i64::from(i16::MAX)))
            .map_err(|_| X11Error::Capture("pointer x out of range".into()))?;
        let root_y = i16::try_from(root_y.clamp(0, i64::from(i16::MAX)))
            .map_err(|_| X11Error::Capture("pointer y out of range".into()))?;
        self.fake(MOTION_NOTIFY_EVENT, 0, root_x, root_y)
    }

    fn fake(&self, event_type: u8, detail: u8, root_x: i16, root_y: i16) -> Result<(), X11Error> {
        xtest::fake_input(
            &self.connection,
            event_type,
            detail,
            0,
            self.root,
            root_x,
            root_y,
            0,
        )
        .map_err(protocol)?
        .check()
        .map_err(protocol)
    }

    fn button(&self, button: u8) -> Result<(), X11Error> {
        self.fake(4, button, 0, 0)?;
        self.fake(5, button, 0, 0)
    }

    fn type_text(&self, text: &str) -> Result<(), X11Error> {
        for character in text.chars() {
            let keysym = match character {
                '\n' => 0xff0d,
                '\t' => 0xff09,
                ' ' => 0x20,
                character if character.is_ascii() => character as u32,
                _ => return Err(X11Error::UnsupportedText(character)),
            };
            let (keycode, shift) = self
                .keymap
                .get(&keysym)
                .copied()
                .ok_or(X11Error::UnsupportedText(character))?;
            if shift {
                self.fake(2, self.shift_keycode()?, 0, 0)?;
            }
            self.fake(2, keycode, 0, 0)?;
            self.fake(3, keycode, 0, 0)?;
            if shift {
                self.fake(3, self.shift_keycode()?, 0, 0)?;
            }
        }
        Ok(())
    }

    fn key_chord(&self, chord: &KeyChord) -> Result<(), X11Error> {
        let keycode = self.keysym_keycode(
            keysym_for_key(&chord.key).ok_or_else(|| X11Error::InvalidKey(chord.key.clone()))?,
        )?;
        let mut modifiers = Vec::new();
        for modifier in &chord.modifiers {
            if let Some(keysym) = modifier_keysym(*modifier) {
                modifiers.push(self.keysym_keycode(keysym)?);
            }
        }
        for keycode in &modifiers {
            self.fake(2, *keycode, 0, 0)?;
        }
        self.fake(2, keycode, 0, 0)?;
        self.fake(3, keycode, 0, 0)?;
        for keycode in modifiers.iter().rev() {
            self.fake(3, *keycode, 0, 0)?;
        }
        Ok(())
    }

    fn keysym_keycode(&self, keysym: u32) -> Result<u8, X11Error> {
        self.keymap
            .get(&keysym)
            .map(|(keycode, _)| *keycode)
            .ok_or(X11Error::InvalidKey(format!("keysym {keysym:#x}")))
    }

    fn shift_keycode(&self) -> Result<u8, X11Error> {
        self.keysym_keycode(0xffe1)
    }

    fn window_info(&self, window: u32) -> Option<WindowInfo> {
        let attributes = self
            .connection
            .get_window_attributes(window)
            .ok()?
            .reply()
            .ok()?;
        if attributes.map_state != MapState::VIEWABLE {
            return None;
        }
        let geometry = self.connection.get_geometry(window).ok()?.reply().ok()?;
        if geometry.width == 0 || geometry.height == 0 {
            return None;
        }
        let translated = self
            .connection
            .translate_coordinates(window, self.root, 0, 0)
            .ok()?
            .reply()
            .ok()?;
        let title = self
            .text_property(window, self.atoms.net_wm_name, self.atoms.utf8_string)
            .or_else(|| self.text_property(window, u32::from(AtomEnum::WM_NAME), 0))
            .unwrap_or_default();
        let class = self
            .text_property(window, self.atoms.wm_class, 0)
            .map(|value| value.split('\0').next().unwrap_or_default().to_string());
        let kind = classify_window(&class, &title);
        Some(WindowInfo {
            id: WindowId(u64::from(window)),
            app: class.clone().unwrap_or_default(),
            title,
            class,
            kind,
            bounds: Rect {
                x: i32::from(translated.dst_x),
                y: i32::from(translated.dst_y),
                width: u32::from(geometry.width),
                height: u32::from(geometry.height),
            },
        })
    }

    fn text_property(&self, window: u32, property: u32, kind: u32) -> Option<String> {
        let reply = self
            .connection
            .get_property(false, window, property, kind, 0, 1024)
            .ok()?
            .reply()
            .ok()?;
        let bytes = reply.value;
        if bytes.is_empty() {
            return None;
        }
        let text = String::from_utf8_lossy(&bytes)
            .trim_matches('\0')
            .trim()
            .to_string();
        (!text.is_empty()).then_some(text)
    }

    fn capture_png(&self, window: u32, width: u16, height: u16) -> Result<Vec<u8>, X11Error> {
        let reply = self
            .connection
            .get_image(ImageFormat::Z_PIXMAP, window, 0, 0, width, height, !0)
            .map_err(protocol)?
            .reply()
            .map_err(protocol)?;
        let setup = self.connection.setup();
        let depth = u32::from(reply.depth);
        let format = setup
            .pixmap_formats
            .iter()
            .find(|format| u32::from(format.depth) == depth)
            .ok_or_else(|| X11Error::Capture(format!("unsupported depth {depth}")))?;
        let bpp = u32::from(format.bits_per_pixel);
        let stride = (u32::from(width) * bpp).div_ceil(u32::from(format.scanline_pad))
            * u32::from(format.scanline_pad)
            / 8;
        let visual = setup.roots[self.screen]
            .allowed_depths
            .iter()
            .flat_map(|depth| &depth.visuals)
            .find(|visual| visual.visual_id == reply.visual)
            .ok_or_else(|| X11Error::Capture("visual not found".into()))?;
        let mut rgba = Vec::with_capacity(usize::from(width) * usize::from(height) * 4);
        for row in 0..usize::from(height) {
            let start = row * stride as usize;
            for column in 0..usize::from(width) {
                let offset = start + column * (bpp as usize / 8);
                if offset + (bpp as usize / 8) > reply.data.len() {
                    return Err(X11Error::Capture("short image data".into()));
                }
                let pixel = match bpp {
                    32 => u32::from_le_bytes([
                        reply.data[offset],
                        reply.data[offset + 1],
                        reply.data[offset + 2],
                        reply.data[offset + 3],
                    ]),
                    24 => u32::from_le_bytes([
                        reply.data[offset],
                        reply.data[offset + 1],
                        reply.data[offset + 2],
                        0,
                    ]),
                    _ => return Err(X11Error::Capture(format!("unsupported bpp {bpp}"))),
                };
                rgba.push(channel(pixel, visual.red_mask));
                rgba.push(channel(pixel, visual.green_mask));
                rgba.push(channel(pixel, visual.blue_mask));
                rgba.push(255);
            }
        }
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, u32::from(width), u32::from(height));
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|error| X11Error::Png(error.to_string()))?;
            writer
                .write_image_data(&rgba)
                .map_err(|error| X11Error::Png(error.to_string()))?;
        }
        Ok(png)
    }
}

fn channel(pixel: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shift = mask.trailing_zeros();
    let max = mask >> shift;
    (((pixel & mask) >> shift) * 255 / max) as u8
}

fn build_keymap(connection: &RustConnection) -> Result<HashMap<u32, (u8, bool)>, X11Error> {
    let setup = connection.setup();
    let first = setup.min_keycode;
    let count = setup.max_keycode.saturating_sub(first).saturating_add(1);
    let reply = connection
        .get_keyboard_mapping(first, count)
        .map_err(protocol)?
        .reply()
        .map_err(protocol)?;
    let per = usize::from(reply.keysyms_per_keycode);
    let mut keymap = HashMap::new();
    for (offset, keycode) in (first..=setup.max_keycode).enumerate() {
        for column in 0..per {
            let keysym = reply.keysyms[offset * per + column];
            if keysym == 0 {
                continue;
            }
            let shifted = column == 1;
            keymap.entry(keysym).or_insert((keycode, shifted));
        }
    }
    Ok(keymap)
}

fn modifier_keysym(modifier: Modifier) -> Option<u32> {
    match modifier {
        Modifier::Ctrl => Some(0xffe3),
        Modifier::Alt => Some(0xffe9),
        Modifier::Shift => Some(0xffe1),
        Modifier::Super => Some(0xffeb),
    }
}

fn keysym_for_key(key: &str) -> Option<u32> {
    match key.trim().to_ascii_lowercase().as_str() {
        "return" | "enter" => Some(0xff0d),
        "tab" => Some(0xff09),
        "escape" | "esc" => Some(0xff1b),
        "space" => Some(0x20),
        "backspace" => Some(0xff08),
        "delete" | "del" => Some(0xffff),
        "up" => Some(0xff52),
        "down" => Some(0xff54),
        "left" => Some(0xff51),
        "right" => Some(0xff53),
        "home" => Some(0xff50),
        "end" => Some(0xff57),
        "pageup" => Some(0xff55),
        "pagedown" => Some(0xff56),
        key if key.len() == 1 => Some(key.chars().next()? as u32),
        key if key.starts_with('f') => key[1..]
            .parse::<u32>()
            .ok()
            .map(|n| {
                if (1..=12).contains(&n) {
                    0xffbe + n - 1
                } else {
                    0
                }
            })
            .filter(|keysym| *keysym != 0),
        _ => None,
    }
}

fn classify_window(class: &Option<String>, title: &str) -> WindowKind {
    let haystack =
        format!("{} {}", class.as_deref().unwrap_or_default(), title).to_ascii_lowercase();
    const SENSITIVE: &[&str] = &[
        "password",
        "passwd",
        "keyring",
        "polkit",
        "authentication",
        "1password",
        "bitwarden",
        "keepass",
        "login",
    ];
    if SENSITIVE.iter().any(|needle| haystack.contains(needle)) {
        return WindowKind::Sensitive;
    }
    const TERMINALS: &[&str] = &[
        "terminal",
        "xterm",
        "kitty",
        "alacritty",
        "wezterm",
        "konsole",
        "gnome-terminal",
    ];
    if TERMINALS.iter().any(|needle| haystack.contains(needle)) {
        return WindowKind::Terminal;
    }
    WindowKind::Normal
}

fn intern(connection: &RustConnection, name: &[u8]) -> Result<u32, X11Error> {
    connection
        .intern_atom(false, name)
        .map_err(protocol)?
        .reply()
        .map(|reply| reply.atom)
        .map_err(protocol)
}

fn protocol(error: impl std::fmt::Display) -> X11Error {
    X11Error::Protocol(error.to_string())
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
    use std::process::{Child, Command, Stdio};
    use std::thread::sleep;
    use std::time::Duration;

    use super::*;
    use x11rb::protocol::xproto::{
        ChangeWindowAttributesAux, CreateWindowAux, EventMask, WindowClass,
    };

    #[test]
    fn classifies_terminal_and_sensitive_windows() {
        assert_eq!(
            classify_window(&Some("Gnome-terminal".into()), "bash"),
            WindowKind::Terminal
        );
        assert_eq!(
            classify_window(&Some("1Password".into()), "Unlock"),
            WindowKind::Sensitive
        );
        assert_eq!(
            classify_window(&Some("gedit".into()), "notes.txt"),
            WindowKind::Normal
        );
    }

    #[test]
    fn channel_respects_masks() {
        assert_eq!(channel(0x00ff0000, 0x00ff0000), 255);
        assert_eq!(channel(0x000000ff, 0x000000ff), 255);
        assert_eq!(channel(0, 0x00ff0000), 0);
    }

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
        sleep(Duration::from_millis(200));
        Some((Xvfb(child), display))
    }

    #[test]
    fn observes_a_mapped_window_when_xvfb_is_available() {
        let Some((_guard, display)) = start_xvfb() else {
            eprintln!("skipping: Xvfb is not installed");
            return;
        };
        let (connection, screen) = x11rb::connect(Some(&display)).expect("test X connection");
        let root = connection.setup().roots[screen].root;
        let window_id = connection.generate_id().expect("window id");
        let width = 120_u16;
        let height = 80_u16;
        connection
            .create_window(
                0,
                window_id,
                root,
                10,
                10,
                width,
                height,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new().background_pixel(0x00ff00),
            )
            .expect("create");
        connection.map_window(window_id).expect("map");
        connection
            .change_window_attributes(
                window_id,
                &ChangeWindowAttributesAux::new()
                    .event_mask(EventMask::BUTTON_PRESS | EventMask::KEY_PRESS),
            )
            .expect("select input");
        connection.flush().expect("flush");
        sleep(Duration::from_millis(250));

        let backend = X11Backend::connect(Some(&display)).expect("backend");
        let capture = backend
            .observe(Some(WindowId(u64::from(window_id))))
            .expect("observe");
        assert_eq!(capture.observation.image.width, u32::from(width));
        assert_eq!(capture.observation.image.height, u32::from(height));
        assert_eq!(
            capture.observation.window.id,
            WindowId(u64::from(window_id))
        );
        assert!(capture.png.starts_with(b"\x89PNG\r\n\x1a\n"));
        let listed = backend.list_windows().expect("windows");
        assert!(listed
            .iter()
            .any(|window| window.id == WindowId(u64::from(window_id))));

        let click = ComputerAction::Click {
            observation_id: capture.observation.id.clone(),
            window_id: capture.observation.window.id,
            point: Point::new(60, 40),
            button: MouseButton::Left,
        };
        backend
            .perform(&click, &capture.observation)
            .expect("click");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut clicked = false;
        while std::time::Instant::now() < deadline {
            if let Some(x11rb::protocol::Event::ButtonPress(_)) =
                connection.poll_for_event().expect("poll")
            {
                clicked = true;
                break;
            }
            sleep(Duration::from_millis(20));
        }
        assert!(clicked, "XTEST click did not reach the window");

        let typed = ComputerAction::Type {
            observation_id: capture.observation.id.clone(),
            window_id: capture.observation.window.id,
            text: "a".into(),
        };
        backend.perform(&typed, &capture.observation).expect("type");
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut keyed = false;
        while std::time::Instant::now() < deadline {
            if let Some(x11rb::protocol::Event::KeyPress(_)) =
                connection.poll_for_event().expect("poll")
            {
                keyed = true;
                break;
            }
            sleep(Duration::from_millis(20));
        }
        assert!(keyed, "XTEST key did not reach the window");
    }
}
