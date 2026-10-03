//! base64url without padding (RFC 4648 §5), the contract's only encoding for keys, signatures,
//! and sealed bytes (contract §1). Decoding is strict: no padding, no other alphabet, and no
//! leftover bits, so every value has exactly one spelling.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

pub fn encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Decode, refusing padding, other alphabets, and anything longer than `most` bytes.
pub fn decode(text: &str, most: usize) -> Option<Vec<u8>> {
    if text.len() > most.div_ceil(3) * 4 + 4 {
        return None;
    }
    let bytes = URL_SAFE_NO_PAD.decode(text).ok()?;
    (bytes.len() <= most).then_some(bytes)
}

/// Decode exactly `N` bytes.
pub fn decode_exact<const N: usize>(text: &str) -> Option<[u8; N]> {
    decode(text, N)?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_spelling_for_every_value() {
        let bytes = [0xfbu8, 0xff, 0x00, 0x10];
        let text = encode(&bytes);
        assert_eq!(text, "-_8AEA");
        assert_eq!(decode(&text, 4).unwrap(), bytes);
        assert_eq!(decode("-_8AEA==", 4), None, "padding");
        assert_eq!(decode("+/8AEA", 4), None, "the other alphabet");
        assert_eq!(decode("-_8AEB", 4), None, "leftover bits");
        assert_eq!(decode(&text, 3), None, "too long");
        assert_eq!(decode_exact::<4>(&text), Some(bytes));
        assert_eq!(decode_exact::<5>(&text), None);
    }
}
