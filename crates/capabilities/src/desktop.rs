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

/// The key combinations a worker may press: ordinary keys (not F1, which opens a browser with
/// help, or F12, a browser's developer tools), and Ctrl, Shift, or Alt with letters, digits,
/// and the moving keys. Every other combination is refused before any card: the owner approves
/// each step from a picture of the screen (ADR-049), and what a shortcut does is not in the
/// picture. Refused are, among others, those that close or switch programs (Alt+F4, Ctrl+W,
/// Ctrl+F4, Ctrl+Q, Alt+Tab, Ctrl+Alt+Tab, Alt+Esc), open the system's own screens (Ctrl+Esc,
/// Ctrl+Shift+Esc, Ctrl+Alt+Delete, Alt+Space), open a browser's developer tools (F12,
/// Ctrl+Shift+I, J, or C), or delete for good (Shift+Delete). An allow-list, so a shortcut
/// nobody thought of is refused too. It is a second wall only: a worker can still reach the
/// same places in several approved steps.
fn allowed(parts: &[KeyPart]) -> bool {
    use KeyPart::{Alt, Backspace, Ctrl, Delete, Down, End, Enter, Home, Left, PageDown, PageUp};
    use KeyPart::{Char, Right, Shift, Space, Tab, Up, F};
    let Some((key, mods)) = parts.split_last() else {
        return false;
    };
    let moving = matches!(
        key,
        Up | Down | Left | Right | Home | End | PageUp | PageDown
    );
    match (
        mods.contains(&Ctrl),
        mods.contains(&Alt),
        mods.contains(&Shift),
    ) {
        (false, false, false) => !matches!(key, F(1) | F(12)),
        (false, false, true) => {
            moving || matches!(key, Tab | Enter | Space | Backspace | F(10) | Char(_))
        }
        (true, false, false) => {
            moving
                || matches!(key, Tab | Enter | Space | Backspace | Delete)
                || matches!(key, Char(c) if !matches!(c, 'w' | 'q' | '`'))
        }
        (true, false, true) => {
            moving
                || matches!(key, Tab)
                || matches!(key, Char(c) if c.is_ascii_alphabetic()
                    && !matches!(c, 'w' | 'q' | 'i' | 'j' | 'c'))
        }
        (false, true, false) => {
            matches!(key, Up | Down | Left | Right)
                || matches!(key, Char(c) if c.is_ascii_alphanumeric())
        }
        _ => false,
    }
}

/// A key combination as a person writes it: "Ctrl+Shift+W", "Alt+F4".
fn shown(parts: &[KeyPart]) -> String {
    parts
        .iter()
        .map(|p| match p {
            KeyPart::Ctrl => "Ctrl".into(),
            KeyPart::Alt => "Alt".into(),
            KeyPart::Shift => "Shift".into(),
            KeyPart::Enter => "Enter".into(),
            KeyPart::Tab => "Tab".into(),
            KeyPart::Escape => "Esc".into(),
            KeyPart::Backspace => "Backspace".into(),
            KeyPart::Delete => "Delete".into(),
            KeyPart::Up => "Up".into(),
            KeyPart::Down => "Down".into(),
            KeyPart::Left => "Left".into(),
            KeyPart::Right => "Right".into(),
            KeyPart::Home => "Home".into(),
            KeyPart::End => "End".into(),
            KeyPart::PageUp => "PageUp".into(),
            KeyPart::PageDown => "PageDown".into(),
            KeyPart::Space => "Space".into(),
            KeyPart::F(n) => format!("F{n}"),
            KeyPart::Char(c) => c.to_ascii_uppercase().to_string(),
        })
        .collect::<Vec<String>>()
        .join("+")
}

/// Refuse text a worker would type on the screen that holds a character the owner cannot see on
/// the approval card: a control character other than tab and line breaks (it acts like a key
/// press: Ctrl+C stops a program in a terminal), or an invisible one (zero-width characters,
/// the marks that turn text right to left, tag characters, line and paragraph separators),
/// which can make the card show other text than what is typed.
pub fn refuse_hidden_characters(text: &str) -> Result<(), String> {
    let hidden = |c: char| {
        (c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
            || matches!(c,
                '\u{00AD}' | '\u{061C}' | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
                | '\u{E0000}'..='\u{E007F}')
    };
    match text.chars().find(|&c| hidden(c)) {
        Some(c) => Err(format!(
            "the text has a hidden character (U+{:04X}) that acts like a key press or does not \
             show on the owner's card; only visible text, tabs, and line breaks can be typed",
            c as u32
        )),
        None => Ok(()),
    }
}

/// Text to type with its line breaks as one kind (`\n`): a Windows line break (`\r\n`) or a
/// lone `\r` would otherwise press Enter more than once.
pub fn one_kind_of_line_break(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Read a key combination such as `enter`, `ctrl+s`, or `alt+f`: modifiers first, then one key,
/// each modifier once, and a letter or sign from the ASCII keys only (another letter goes by the
/// keyboard's layout, and could be a shortcut there). The Windows (or Command) key is refused,
/// and so is every combination [`allowed`] does not list.
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
                    (Some(c), None) if c.is_ascii_graphic() => {
                        KeyPart::Char(c.to_ascii_lowercase())
                    }
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
    if (1..mods.len()).any(|i| mods[i..].contains(&mods[i - 1])) {
        return Err("give each modifier once, like ctrl+shift+s".into());
    }
    if !allowed(&parts) {
        return Err(format!(
            "{} is not available to workers: they may press ordinary keys, and Ctrl, Shift, or \
             Alt with letters, digits, and the moving keys, but not the shortcuts that close or \
             switch programs, open the system's own screens, or open a browser's developer tools",
            shown(&parts)
        ));
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

/// What a worker is told on a Linux desktop that runs Wayland (ADR-154, Wave 2): Plenipo sees
/// and uses only X11 so far, and never sends a picture of part of the screen.
pub const WAYLAND_NOT_YET: &str = "Computer use doesn't work on this desktop yet: it runs \
    Wayland, which Plenipo supports in a later version. On Ubuntu 22.04 and 24.04, choose \
    \"Ubuntu on Xorg\" with the gear button when you sign in, and it works.";

/// Refuse under Wayland (Linux); fine everywhere else.
fn not_on_wayland() -> Result<(), String> {
    let wayland = cfg!(target_os = "linux")
        && runs_wayland(
            std::env::var_os("XDG_SESSION_TYPE"),
            std::env::var_os("WAYLAND_DISPLAY"),
        );
    if wayland {
        Err(WAYLAND_NOT_YET.into())
    } else {
        Ok(())
    }
}

/// Whether the desktop session is Wayland: as the session says, or, when it does not say, when
/// it has a Wayland display.
fn runs_wayland(
    session: Option<std::ffi::OsString>,
    wayland_display: Option<std::ffi::OsString>,
) -> bool {
    match session.as_ref().and_then(|s| s.to_str()) {
        Some("x11") => false,
        Some("wayland") => true,
        _ => wayland_display.is_some_and(|d| !d.is_empty()),
    }
}

fn enigo() -> Result<enigo::Enigo, String> {
    not_on_wayland()?;
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
        not_on_wayland()?;
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
            parse_keys("shift + f10").unwrap(),
            [KeyPart::Shift, KeyPart::F(10)]
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
            "ctrl+ctrl+s",
            // A letter from another alphabet goes by the keyboard's layout: Ctrl+W on a Russian
            // one.
            "ctrl+ц",
        ] {
            assert!(parse_keys(bad).is_err(), "{bad}");
        }
    }

    /// Workers may press ordinary keys, and Ctrl, Shift, or Alt with letters, digits, and the
    /// moving keys. The shortcuts that close or switch programs, open the system's own screens,
    /// or open a browser's developer tools are refused, however they are written, and so is
    /// every combination the list does not name.
    #[test]
    fn only_ordinary_keys_and_shortcuts_are_available() {
        for fine in [
            "f4",
            "f5",
            "tab",
            "shift+tab",
            "esc",
            "enter",
            "ctrl+s",
            "ctrl+c",
            "ctrl+v",
            "ctrl+z",
            "ctrl+t",
            "ctrl+j",
            "ctrl+shift+s",
            "ctrl+shift+t",
            "ctrl+tab",
            "ctrl+shift+tab",
            "ctrl+delete",
            "ctrl+backspace",
            "ctrl+left",
            "ctrl+home",
            "shift+end",
            "shift+f10",
            "alt+f",
            "alt+2",
            "alt+left",
            "space",
            "delete",
        ] {
            assert!(parse_keys(fine).is_ok(), "{fine}");
        }
        for (combo, shown) in [
            ("alt+f4", "Alt+F4"),
            ("ALT + F4", "Alt+F4"),
            ("shift+alt+f4", "Shift+Alt+F4"),
            ("ctrl+f4", "Ctrl+F4"),
            ("ctrl+w", "Ctrl+W"),
            ("ctrl+shift+w", "Ctrl+Shift+W"),
            ("ctrl+q", "Ctrl+Q"),
            ("alt+tab", "Alt+Tab"),
            ("shift+alt+tab", "Shift+Alt+Tab"),
            ("ctrl+alt+tab", "Ctrl+Alt+Tab"),
            ("alt+esc", "Alt+Esc"),
            ("alt+shift+esc", "Alt+Shift+Esc"),
            ("ctrl+esc", "Ctrl+Esc"),
            ("ctrl+shift+esc", "Ctrl+Shift+Esc"),
            ("shift+control+escape", "Shift+Ctrl+Esc"),
            ("ctrl+alt+del", "Ctrl+Alt+Delete"),
            ("alt+space", "Alt+Space"),
            ("alt+enter", "Alt+Enter"),
            ("f1", "F1"),
            ("f12", "F12"),
            ("ctrl+shift+i", "Ctrl+Shift+I"),
            ("ctrl+shift+j", "Ctrl+Shift+J"),
            ("ctrl+shift+c", "Ctrl+Shift+C"),
            ("ctrl+shift+delete", "Ctrl+Shift+Delete"),
            ("shift+delete", "Shift+Delete"),
        ] {
            let err = parse_keys(combo).unwrap_err();
            assert!(
                err.starts_with(&format!("{shown} is not available to workers")),
                "{combo}: {err}"
            );
        }
    }

    /// Typed text is visible text, tabs, and line breaks; a hidden character is refused, and
    /// line breaks are made one kind.
    #[test]
    fn typed_text_has_no_hidden_characters() {
        for fine in [
            "hello",
            "two\nlines",
            "tab\tstop",
            "windows\r\nline",
            "café ✓",
            "",
        ] {
            assert!(refuse_hidden_characters(fine).is_ok(), "{fine:?}");
        }
        for bad in [
            "esc\u{1b}",
            "back\u{8}space",
            "\u{7f}",
            "nul\0",
            "\u{9b}",
            "zero\u{200B}width",
            "turn\u{202E}around",
            "tag\u{E0041}",
            "line\u{2028}sep",
        ] {
            let err = refuse_hidden_characters(bad).unwrap_err();
            assert!(err.contains("hidden character (U+"), "{bad:?}: {err}");
        }
        assert!(refuse_hidden_characters("\u{1b}")
            .unwrap_err()
            .contains("U+001B"));
        assert_eq!(one_kind_of_line_break("a\r\nb\rc\nd"), "a\nb\nc\nd");
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

    /// ADR-154: under Wayland computer use is refused, plainly; X11 is used.
    #[test]
    fn computer_use_waits_for_x11_under_wayland() {
        let os = |s: &str| Some(std::ffi::OsString::from(s));
        assert!(runs_wayland(os("wayland"), None));
        assert!(runs_wayland(os("wayland"), os("wayland-0")));
        assert!(
            !runs_wayland(os("x11"), os("wayland-0")),
            "the session says X11"
        );
        assert!(runs_wayland(None, os("wayland-0")));
        assert!(!runs_wayland(None, None));
        assert!(!runs_wayland(None, os("")));
        assert!(!runs_wayland(os("tty"), None));
        assert!(WAYLAND_NOT_YET.contains("Ubuntu on Xorg"));
    }
}
