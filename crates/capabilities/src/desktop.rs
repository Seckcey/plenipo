//! This computer's screen, mouse, and keyboard, for computer use (Phase 10, ADR-020): the last
//! resort, after an official connection, a command-line program, and the browser.
//!
//! [`SystemDesktop`] is the real one: screenshots of the main screen (`xcap` on Windows, X11 on
//! Linux) and input through `enigo` (Windows `SendInput`, X11 XTest). [`SyntheticDesktop`] is a
//! stand-in screen for tests: it draws nothing real and records what it was asked to do.

use std::sync::{Arc, Mutex, MutexGuard};

/// A screenshot: its size in screen pixels and its RGBA pixels, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

/// One part of a key combination ("ctrl+s": `Ctrl`, then `Char('s')`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPart {
    Ctrl,
    Alt,
    Shift,
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Space,
    F(u8),
    Char(char),
}

impl KeyPart {
    fn modifier(self) -> bool {
        matches!(self, Self::Ctrl | Self::Alt | Self::Shift)
    }
}

/// Read a key combination such as `enter`, `ctrl+s`, or `alt+tab`: modifiers first, then one
/// key. The Windows (or Command) key is refused: it opens system menus and shortcuts.
pub fn parse_keys(text: &str) -> Result<Vec<KeyPart>, String> {
    let mut parts = Vec::new();
    for raw in text.split('+').map(str::trim) {
        let lower = raw.to_ascii_lowercase();
        let part = match lower.as_str() {
            "ctrl" | "control" => KeyPart::Ctrl,
            "alt" => KeyPart::Alt,
            "shift" => KeyPart::Shift,
            "enter" | "return" => KeyPart::Enter,
            "tab" => KeyPart::Tab,
            "esc" | "escape" => KeyPart::Escape,
            "backspace" => KeyPart::Backspace,
            "delete" | "del" => KeyPart::Delete,
            "up" | "arrowup" => KeyPart::Up,
            "down" | "arrowdown" => KeyPart::Down,
            "left" | "arrowleft" => KeyPart::Left,
            "right" | "arrowright" => KeyPart::Right,
            "home" => KeyPart::Home,
            "end" => KeyPart::End,
            "pageup" => KeyPart::PageUp,
            "pagedown" => KeyPart::PageDown,
            "space" => KeyPart::Space,
            "win" | "windows" | "meta" | "cmd" | "command" | "super" => {
                return Err(
                    "the Windows key is not available to workers (it opens system menus and \
                     shortcuts)"
                        .into(),
                )
            }
            f if f.len() >= 2 && f.starts_with('f') && f[1..].parse::<u8>().is_ok() => {
                let n: u8 = f[1..].parse().unwrap_or(0);
                if !(1..=12).contains(&n) {
                    return Err(format!("{raw} is not a key"));
                }
                KeyPart::F(n)
            }
            _ => {
                let mut chars = raw.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) if !c.is_control() => KeyPart::Char(c.to_ascii_lowercase()),
                    _ => {
                        return Err(format!(
                            "{raw:?} is not a key (use names like enter, tab, ctrl+s)"
                        ))
                    }
                }
            }
        };
        parts.push(part);
    }
    let (last, mods) = parts.split_last().ok_or("no key given")?;
    if last.modifier() || mods.iter().any(|m| !m.modifier()) {
        return Err("give modifiers first and one key last, like ctrl+s".into());
    }
    if parts.len() > 4 {
        return Err("at most three modifiers and one key".into());
    }
    Ok(parts)
}

/// The screen, mouse, and keyboard Plenipo's computer-use tools act on.
pub trait Desktop: Send + Sync + 'static {
    /// "This computer's screen", or the stand-in's name.
    fn name(&self) -> String;
    /// A screenshot of the main screen.
    fn capture(&self) -> Result<Frame, String>;
    /// Where the mouse pointer is, in screen pixels.
    fn cursor(&self) -> Result<(i32, i32), String>;
    fn move_to(&self, x: i32, y: i32) -> Result<(), String>;
    fn click(&self, button: Button, count: u8) -> Result<(), String>;
    /// Type text (short pieces: Plenipo checks for a stop between them).
    fn type_text(&self, text: &str) -> Result<(), String>;
    /// Press a key combination.
    fn keys(&self, keys: &[KeyPart]) -> Result<(), String>;
    /// Scroll by `lines` (down when positive).
    fn scroll(&self, lines: i32) -> Result<(), String>;
    /// Let go of every key and button (after a stop).
    fn release_all(&self);
}

// ---- The real screen ------------------------------------------------------------------------

/// This computer's own screen, mouse, and keyboard.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemDesktop;

fn enigo() -> Result<enigo::Enigo, String> {
    enigo::Enigo::new(&enigo::Settings::default())
        .map_err(|e| format!("the mouse and keyboard cannot be used: {e}"))
}

fn input_error(e: enigo::InputError) -> String {
    format!("the mouse or keyboard did not respond: {e}")
}

fn enigo_key(k: KeyPart) -> enigo::Key {
    use enigo::Key;
    match k {
        KeyPart::Ctrl => Key::Control,
        KeyPart::Alt => Key::Alt,
        KeyPart::Shift => Key::Shift,
        KeyPart::Enter => Key::Return,
        KeyPart::Tab => Key::Tab,
        KeyPart::Escape => Key::Escape,
        KeyPart::Backspace => Key::Backspace,
        KeyPart::Delete => Key::Delete,
        KeyPart::Up => Key::UpArrow,
        KeyPart::Down => Key::DownArrow,
        KeyPart::Left => Key::LeftArrow,
        KeyPart::Right => Key::RightArrow,
        KeyPart::Home => Key::Home,
        KeyPart::End => Key::End,
        KeyPart::PageUp => Key::PageUp,
        KeyPart::PageDown => Key::PageDown,
        KeyPart::Space => Key::Space,
        KeyPart::F(n) => match n {
            1 => Key::F1,
            2 => Key::F2,
            3 => Key::F3,
            4 => Key::F4,
            5 => Key::F5,
            6 => Key::F6,
            7 => Key::F7,
            8 => Key::F8,
            9 => Key::F9,
            10 => Key::F10,
            11 => Key::F11,
            _ => Key::F12,
        },
        KeyPart::Char(c) => Key::Unicode(c),
    }
}

impl Desktop for SystemDesktop {
    fn name(&self) -> String {
        "This computer's screen".into()
    }

    fn capture(&self) -> Result<Frame, String> {
        capture::main_screen()
    }

    fn cursor(&self) -> Result<(i32, i32), String> {
        use enigo::Mouse as _;
        enigo()?.location().map_err(input_error)
    }

    fn move_to(&self, x: i32, y: i32) -> Result<(), String> {
        use enigo::Mouse as _;
        enigo()?
            .move_mouse(x, y, enigo::Coordinate::Abs)
            .map_err(input_error)
    }

    fn click(&self, button: Button, count: u8) -> Result<(), String> {
        use enigo::Mouse as _;
        let b = match button {
            Button::Left => enigo::Button::Left,
            Button::Right => enigo::Button::Right,
            Button::Middle => enigo::Button::Middle,
        };
        let mut e = enigo()?;
        for _ in 0..count.max(1) {
            e.button(b, enigo::Direction::Click).map_err(input_error)?;
        }
        Ok(())
    }

    fn type_text(&self, text: &str) -> Result<(), String> {
        use enigo::Keyboard as _;
        enigo()?.text(text).map_err(input_error)
    }

    fn keys(&self, keys: &[KeyPart]) -> Result<(), String> {
        use enigo::Keyboard as _;
        let mut e = enigo()?;
        let (last, mods) = keys.split_last().ok_or("no key given")?;
        for m in mods {
            e.key(enigo_key(*m), enigo::Direction::Press)
                .map_err(input_error)?;
        }
        let pressed = e.key(enigo_key(*last), enigo::Direction::Click);
        for m in mods.iter().rev() {
            let _ = e.key(enigo_key(*m), enigo::Direction::Release);
        }
        pressed.map_err(input_error)
    }

    fn scroll(&self, lines: i32) -> Result<(), String> {
        use enigo::Mouse as _;
        enigo()?
            .scroll(lines, enigo::Axis::Vertical)
            .map_err(input_error)
    }

    fn release_all(&self) {
        use enigo::{Keyboard as _, Mouse as _};
        if let Ok(mut e) = enigo() {
            for k in [enigo::Key::Control, enigo::Key::Alt, enigo::Key::Shift] {
                let _ = e.key(k, enigo::Direction::Release);
            }
            for b in [
                enigo::Button::Left,
                enigo::Button::Right,
                enigo::Button::Middle,
            ] {
                let _ = e.button(b, enigo::Direction::Release);
            }
        }
    }
}

#[cfg(windows)]
mod capture {
    use super::Frame;

    pub fn main_screen() -> Result<Frame, String> {
        let monitors =
            xcap::Monitor::all().map_err(|e| format!("the screen cannot be captured: {e}"))?;
        let monitor = monitors
            .iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .or_else(|| monitors.first())
            .ok_or("no screen was found")?;
        let image = monitor
            .capture_image()
            .map_err(|e| format!("the screen cannot be captured: {e}"))?;
        let (width, height) = (image.width(), image.height());
        Ok(Frame {
            width,
            height,
            rgba: image.into_raw(),
        })
    }
}

#[cfg(target_os = "linux")]
mod capture {
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat, ImageOrder};

    use super::Frame;

    /// The whole X11 screen (development and CI; Windows is the target).
    pub fn main_screen() -> Result<Frame, String> {
        let fail = |e: &dyn std::fmt::Display| format!("the screen cannot be captured: {e}");
        let (conn, n) = x11rb::connect(None).map_err(|e| fail(&e))?;
        let setup = conn.setup();
        let screen = &setup.roots[n];
        let (w, h) = (screen.width_in_pixels, screen.height_in_pixels);
        let bpp = setup
            .pixmap_formats
            .iter()
            .find(|f| f.depth == screen.root_depth)
            .map_or(0, |f| f.bits_per_pixel);
        if bpp != 32 || setup.image_byte_order != ImageOrder::LSB_FIRST {
            return Err(fail(&"this screen's pixel format is not supported"));
        }
        let image = conn
            .get_image(ImageFormat::Z_PIXMAP, screen.root, 0, 0, w, h, !0)
            .map_err(|e| fail(&e))?
            .reply()
            .map_err(|e| fail(&e))?;
        let mut rgba = image.data;
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        Ok(Frame {
            width: u32::from(w),
            height: u32::from(h),
            rgba,
        })
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod capture {
    use super::Frame;

    pub fn main_screen() -> Result<Frame, String> {
        Err("seeing the screen is not supported on this system".into())
    }
}

// ---- A stand-in screen for tests ------------------------------------------------------------

/// What the stand-in screen was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Did {
    Move(i32, i32),
    Click(Button, u8),
    Type(String),
    Keys(Vec<KeyPart>),
    Scroll(i32),
    ReleaseAll,
}

#[derive(Default)]
struct SyntheticState {
    cursor: (i32, i32),
    did: Vec<Did>,
}

/// A stand-in screen, 800×600, for tests: screenshots are a plain picture, and every mouse and
/// keyboard action is recorded instead of done. The owner "moving the mouse" is simulated with
/// [`SyntheticDesktop::owner_moves`].
#[derive(Clone, Default)]
pub struct SyntheticDesktop {
    state: Arc<Mutex<SyntheticState>>,
}

impl SyntheticDesktop {
    fn state(&self) -> MutexGuard<'_, SyntheticState> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Everything it was asked to do, in order.
    pub fn did(&self) -> Vec<Did> {
        self.state().did.clone()
    }

    /// The owner moves the mouse (not recorded as the worker's).
    pub fn owner_moves(&self, x: i32, y: i32) {
        self.state().cursor = (x, y);
    }
}

impl Desktop for SyntheticDesktop {
    fn name(&self) -> String {
        "A stand-in screen for tests".into()
    }

    fn capture(&self) -> Result<Frame, String> {
        let (w, h) = (800u32, 600u32);
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let title = y < 40;
                rgba.extend_from_slice(if title {
                    &[31, 111, 235, 255]
                } else if (x / 100 + y / 100) % 2 == 0 {
                    &[245, 245, 245, 255]
                } else {
                    &[225, 225, 225, 255]
                });
            }
        }
        Ok(Frame {
            width: w,
            height: h,
            rgba,
        })
    }

    fn cursor(&self) -> Result<(i32, i32), String> {
        Ok(self.state().cursor)
    }

    fn move_to(&self, x: i32, y: i32) -> Result<(), String> {
        let mut s = self.state();
        s.cursor = (x, y);
        s.did.push(Did::Move(x, y));
        Ok(())
    }

    fn click(&self, button: Button, count: u8) -> Result<(), String> {
        self.state().did.push(Did::Click(button, count));
        Ok(())
    }

    fn type_text(&self, text: &str) -> Result<(), String> {
        self.state().did.push(Did::Type(text.to_owned()));
        Ok(())
    }

    fn keys(&self, keys: &[KeyPart]) -> Result<(), String> {
        self.state().did.push(Did::Keys(keys.to_vec()));
        Ok(())
    }

    fn scroll(&self, lines: i32) -> Result<(), String> {
        self.state().did.push(Did::Scroll(lines));
        Ok(())
    }

    fn release_all(&self) {
        self.state().did.push(Did::ReleaseAll);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_combinations_are_read_and_the_windows_key_refused() {
        assert_eq!(parse_keys("enter").unwrap(), [KeyPart::Enter]);
        assert_eq!(
            parse_keys("Ctrl+S").unwrap(),
            [KeyPart::Ctrl, KeyPart::Char('s')]
        );
        assert_eq!(
            parse_keys("ctrl + shift + f5").unwrap(),
            [KeyPart::Ctrl, KeyPart::Shift, KeyPart::F(5)]
        );
        for bad in [
            "win+r",
            "meta",
            "ctrl",
            "s+ctrl",
            "f13",
            "hello",
            "",
            "ctrl+alt+shift+ctrl+x",
        ] {
            assert!(parse_keys(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_stand_in_records_and_the_owner_can_move_its_mouse() {
        let d = SyntheticDesktop::default();
        d.move_to(10, 20).unwrap();
        d.click(Button::Left, 1).unwrap();
        d.type_text("hi").unwrap();
        assert_eq!(d.cursor().unwrap(), (10, 20));
        d.owner_moves(300, 300);
        assert_eq!(d.cursor().unwrap(), (300, 300));
        assert_eq!(
            d.did(),
            [
                Did::Move(10, 20),
                Did::Click(Button::Left, 1),
                Did::Type("hi".into())
            ]
        );
        let f = d.capture().unwrap();
        assert_eq!(f.rgba.len(), (f.width * f.height * 4) as usize);
    }
}
