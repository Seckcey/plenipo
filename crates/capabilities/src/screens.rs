//! Screenshots (Phase 10, ADR-020): the pictures workers see, and the evidence Plenipo keeps of
//! every significant browser and desktop action. Each is a file in Plenipo's data folder
//! (`screenshots/<task>/…`) recorded in the Ledger as a `screenshot` artifact with its SHA-256,
//! so the Activity trail and approval cards can show it. The owner reads one back only by its
//! artifact ID, never by a path.

use std::path::{Path, PathBuf};

use plenipo_ledger::{Artifact, Ledger};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::desktop::Frame;

/// Widest picture a worker is given (screens are scaled down to it).
pub const MAX_WIDTH: u32 = 1280;
/// Largest screenshot file Plenipo reads back for the owner.
const MAX_READ_BYTES: u64 = 20 * 1024 * 1024;

/// A frame scaled down to at most `max_width` pixels wide (a box filter: each new pixel is the
/// average of the pixels it covers). Smaller frames come back unchanged.
pub fn downscale(frame: &Frame, max_width: u32) -> Frame {
    if frame.width <= max_width || frame.width == 0 || frame.height == 0 {
        return frame.clone();
    }
    let (w, h) = (frame.width as usize, frame.height as usize);
    let nw = max_width as usize;
    let nh = ((h * nw) / w).max(1);
    let mut rgba = vec![0u8; nw * nh * 4];
    for ny in 0..nh {
        let y0 = ny * h / nh;
        let y1 = ((ny + 1) * h / nh).max(y0 + 1);
        for nx in 0..nw {
            let x0 = nx * w / nw;
            let x1 = ((nx + 1) * w / nw).max(x0 + 1);
            let mut sum = [0u32; 4];
            for y in y0..y1 {
                let row = y * w * 4;
                for x in x0..x1 {
                    let i = row + x * 4;
                    for (c, s) in sum.iter_mut().enumerate() {
                        *s += u32::from(frame.rgba[i + c]);
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            let o = (ny * nw + nx) * 4;
            for c in 0..4 {
                rgba[o + c] = (sum[c] / n) as u8;
            }
        }
    }
    Frame {
        width: nw as u32,
        height: nh as u32,
        rgba,
    }
}

/// Mark a point on a frame for an approval card (ADR-049, computer use asks before every click
/// and keystroke): a red ring with a white edge around a red dot at the point, so the owner
/// sees where a click lands. A point off the frame marks nothing.
pub fn mark(frame: &mut Frame, x: i32, y: i32) {
    const DOT: f64 = 2.5;
    const RING: (f64, f64) = (8.0, 12.0);
    const EDGE: f64 = 15.0;
    const RED: [u8; 4] = [220, 38, 38, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];
    let (w, h) = (frame.width as i32, frame.height as i32);
    if x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let reach = EDGE.ceil() as i32;
    for py in (y - reach).max(0)..=(y + reach).min(h - 1) {
        for px in (x - reach).max(0)..=(x + reach).min(w - 1) {
            let d = f64::from(px - x).hypot(f64::from(py - y));
            let color = if d <= DOT || (RING.0..=RING.1).contains(&d) {
                RED
            } else if d > RING.1 && d <= EDGE {
                WHITE
            } else {
                continue;
            };
            let i = ((py * w + px) * 4) as usize;
            frame.rgba[i..i + 4].copy_from_slice(&color);
        }
    }
}

/// A frame as a PNG file.
pub fn png(frame: &Frame) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, frame.width, frame.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("the screenshot could not be saved: {e}"))?;
        writer
            .write_image_data(&frame.rgba)
            .map_err(|e| format!("the screenshot could not be saved: {e}"))?;
    }
    Ok(out)
}

/// A picture's type from its first bytes.
pub fn mime_of(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else {
        "image/jpeg"
    }
}

/// Where screenshots are kept, and their Ledger records.
#[derive(Debug, Clone)]
pub struct Evidence {
    dir: PathBuf,
}

impl Evidence {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Keep a screenshot for `task_id`, recorded in the Ledger with `metadata` (what was done,
    /// where, by whom).
    pub fn save(
        &self,
        ledger: &Ledger,
        task_id: &str,
        bytes: &[u8],
        metadata: &Value,
    ) -> Result<Artifact, String> {
        let folder: String = task_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .take(64)
            .collect();
        let dir = self
            .dir
            .join(if folder.is_empty() { "other" } else { &folder });
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("the screenshot folder could not be made: {e}"))?;
        let ext = if mime_of(bytes) == "image/png" {
            "png"
        } else {
            "jpg"
        };
        let name = format!(
            "{}-{}.{ext}",
            plenipo_ledger::now_ms(),
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        );
        let path = dir.join(name);
        std::fs::write(&path, bytes)
            .map_err(|e| format!("the screenshot could not be saved: {e}"))?;
        let hash = format!("sha256:{:x}", Sha256::digest(bytes));
        ledger
            .record_artifact(
                Some(task_id),
                "screenshot",
                Some(&path.display().to_string()),
                None,
                Some(&hash),
                metadata,
                "plenipo",
            )
            .map_err(|e| format!("the screenshot could not be recorded: {e}"))
    }

    /// A screenshot's picture and type, by its artifact ID: only a `screenshot` artifact kept in
    /// this folder, unchanged since it was recorded.
    pub fn read(&self, ledger: &Ledger, artifact_id: &str) -> Result<(Vec<u8>, String), String> {
        let a = ledger
            .artifact(artifact_id)
            .map_err(|e| e.to_string())?
            .filter(|a| a.artifact_type == "screenshot")
            .ok_or("that screenshot does not exist")?;
        let path = PathBuf::from(a.local_path.unwrap_or_default());
        let root = dunce::canonicalize(&self.dir).map_err(|_| "no screenshots are kept yet")?;
        let file =
            dunce::canonicalize(&path).map_err(|_| "that screenshot's file is gone".to_owned())?;
        if !file.starts_with(&root) {
            return Err("that file is not one of Plenipo's screenshots".into());
        }
        if std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0) > MAX_READ_BYTES {
            return Err("that screenshot is too large to show".into());
        }
        let bytes = std::fs::read(&file).map_err(|e| format!("it could not be read: {e}"))?;
        let hash = format!("sha256:{:x}", Sha256::digest(&bytes));
        if a.hash.as_deref().is_some_and(|h| h != hash) {
            return Err("that screenshot's file was changed after it was recorded".into());
        }
        let mime = mime_of(&bytes).to_owned();
        Ok((bytes, mime))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn frame(w: u32, h: u32, v: u8) -> Frame {
        Frame {
            width: w,
            height: h,
            rgba: vec![v; (w * h * 4) as usize],
        }
    }

    #[test]
    fn big_screens_are_scaled_down_and_encoded() {
        let f = frame(2560, 1440, 200);
        let small = downscale(&f, MAX_WIDTH);
        assert_eq!((small.width, small.height), (1280, 720));
        assert!(
            small.rgba.iter().all(|v| *v == 200),
            "averages keep a flat color"
        );
        assert_eq!(downscale(&frame(800, 600, 1), MAX_WIDTH).width, 800);
        let bytes = png(&small).unwrap();
        assert_eq!(mime_of(&bytes), "image/png");
        assert_eq!(mime_of(&[0xff, 0xd8, 0xff]), "image/jpeg");
    }

    fn pixel(f: &Frame, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * f.width + x) * 4) as usize;
        f.rgba[i..i + 4].try_into().unwrap()
    }

    /// ADR-049: the point a click lands at is marked on the card's picture, a red ring with a
    /// white edge around a red dot; the mark is clipped at the picture's edge, and a point off
    /// the picture marks nothing.
    #[test]
    fn a_point_is_marked_for_the_owner() {
        let mut f = frame(100, 100, 200);
        mark(&mut f, 50, 50);
        assert_eq!(pixel(&f, 50, 50), [220, 38, 38, 255], "the dot");
        assert_eq!(pixel(&f, 60, 50), [220, 38, 38, 255], "the ring");
        assert_eq!(pixel(&f, 50, 64), [255, 255, 255, 255], "its white edge");
        assert_eq!(pixel(&f, 55, 50), [200; 4], "between the dot and the ring");
        assert_eq!(pixel(&f, 80, 50), [200; 4], "beyond the mark");
        mark(&mut f, 0, 0);
        assert_eq!(pixel(&f, 0, 0), [220, 38, 38, 255]);
        let before = f.clone();
        mark(&mut f, 100, 50);
        mark(&mut f, -1, 50);
        assert_eq!(f, before, "off the picture: nothing");
    }

    #[test]
    fn evidence_is_recorded_and_read_back_only_by_its_id() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = Ledger::open_in_memory().unwrap();
        let task = ledger
            .create_task(
                plenipo_ledger::NewTask {
                    objective: "t".into(),
                    requested_by: "owner".into(),
                    ..Default::default()
                },
                "test",
            )
            .unwrap();
        let ev = Evidence::new(dir.path().join("screenshots"));
        let bytes = png(&frame(4, 4, 9)).unwrap();
        let a = ev
            .save(&ledger, &task.id, &bytes, &json!({ "tool": "screen_view" }))
            .unwrap();
        assert_eq!(a.artifact_type, "screenshot");
        let (back, mime) = ev.read(&ledger, &a.id).unwrap();
        assert_eq!((back, mime.as_str()), (bytes, "image/png"));
        // Changed afterwards: refused.
        std::fs::write(a.local_path.as_deref().unwrap(), b"x").unwrap();
        assert!(ev.read(&ledger, &a.id).unwrap_err().contains("changed"));
        // Another kind of artifact, or one outside the folder: refused.
        let outside = dir.path().join("elsewhere.png");
        std::fs::write(&outside, png(&frame(1, 1, 0)).unwrap()).unwrap();
        let other = ledger
            .record_artifact(
                Some(&task.id),
                "screenshot",
                Some(&outside.display().to_string()),
                None,
                None,
                &json!({}),
                "test",
            )
            .unwrap();
        assert!(ev.read(&ledger, &other.id).is_err());
        assert!(ev.read(&ledger, "missing").is_err());
    }
}
