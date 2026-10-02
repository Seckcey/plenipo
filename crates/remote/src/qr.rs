//! The picture code (QR code) Settings → Devices shows for **Add a phone** (ADR-141 §2).

use qrcode::{Color, EcLevel, QrCode};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A picture code: `size` × `size` squares, row by row, `1` dark and `0` light. The screen
/// draws it (with a quiet border of its own).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Qr {
    pub size: u32,
    pub cells: String,
}

impl Qr {
    /// The picture code for `text` (medium error correction, so a phone reads it off a screen).
    pub fn of(text: &str) -> Self {
        let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::M)
            .expect("a short address fits a picture code");
        let size = u32::try_from(code.width()).expect("a small picture code");
        let cells = code
            .to_colors()
            .into_iter()
            .map(|c| if c == Color::Dark { '1' } else { '0' })
            .collect();
        Self { size, cells }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pairing_address_makes_a_square_picture_code() {
        let qr = Qr::of("https://remote.getplenipo.com/#pair=7K3QM9TX2HFDR8WB");
        assert!(qr.size >= 21);
        assert_eq!(qr.cells.len(), (qr.size * qr.size) as usize);
        assert!(qr.cells.chars().all(|c| c == '0' || c == '1'));
        // The three corner squares start dark.
        assert!(qr.cells.starts_with("1111111"));
    }
}
