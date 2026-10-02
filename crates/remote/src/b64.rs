//! base64url without padding (RFC 4648 §5), the only encoding the sealed line and the relay use.

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

/// Is `text` a random ID: `bytes` random bytes in base64url?
pub fn is_id(text: &str, bytes: usize) -> bool {
    text.len() == (bytes * 4).div_ceil(3) && decode(text, bytes).is_some_and(|b| b.len() == bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_strict() {
        let bytes = [0xfbu8, 0xff, 0x00, 0x10];
        let text = encode(&bytes);
        assert_eq!(text, "-_8AEA");
        assert_eq!(decode(&text, 4).unwrap(), bytes);
        assert_eq!(decode("-_8AEA==", 4), None, "padding");
        assert_eq!(decode("+/8AEA", 4), None, "the other alphabet");
        assert_eq!(decode(&text, 3), None, "too long");
        assert_eq!(decode_exact::<4>(&text), Some(bytes));
        assert_eq!(decode_exact::<5>(&text), None);
    }

    #[test]
    fn ids() {
        let id = encode(&[7u8; 16]);
        assert_eq!(id.len(), 22);
        assert!(is_id(&id, 16));
        assert!(!is_id(&id, 32));
        assert!(!is_id("not an id!", 16));
    }
}
