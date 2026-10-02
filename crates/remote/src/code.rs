//! Pairing codes (ADR-141 §2): 16 letters and digits, 80 bits of chance, with no letters that
//! look alike (Crockford's base32: no I, L, O, or U).
//!
//! The code is the shared key of the phone's first meeting with the PC (ADR-143 §5, Noise
//! `XXpsk3`). The relay never sees it: the picture code carries it after a `#`, which a browser
//! never sends to any server, and the relay finds the waiting PC by a mailbox name made from the
//! code with HKDF ([`Code::mailbox`]), never the code itself.

use hkdf::Hkdf;
use sha2::Sha256;

/// Crockford's base32 alphabet.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// The code's length in letters and digits.
pub const LENGTH: usize = 16;
/// HKDF's salt for everything made from a code.
const SALT: &[u8] = b"plenipo-remote-pairing.v1";

/// A pairing code, in its plain form (16 characters, no dashes).
#[derive(Clone, PartialEq, Eq)]
pub struct Code(String);

impl std::fmt::Debug for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never in a log.
        f.write_str("Code(…)")
    }
}

impl Code {
    /// A new code: 80 random bits.
    pub fn new() -> Self {
        let bytes: [u8; 10] = crate::random();
        Self::from_bits(bytes)
    }

    fn from_bits(bytes: [u8; 10]) -> Self {
        let mut bits: u128 = 0;
        for b in bytes {
            bits = (bits << 8) | u128::from(b);
        }
        let mut text = String::with_capacity(LENGTH);
        for i in (0..LENGTH).rev() {
            let index = usize::try_from((bits >> (i * 5)) & 0x1f).expect("five bits");
            text.push(char::from(ALPHABET[index]));
        }
        Self(text)
    }

    /// Read a code as the owner typed it: any case, with or without dashes and spaces, with O
    /// read as 0 and I or L read as 1 (as Crockford's base32 says).
    pub fn parse(typed: &str) -> Option<Self> {
        let mut text = String::with_capacity(LENGTH);
        for c in typed.chars() {
            if c == '-' || c.is_whitespace() {
                continue;
            }
            let c = match c.to_ascii_uppercase() {
                'O' => '0',
                'I' | 'L' => '1',
                c => c,
            };
            if !c.is_ascii() || !ALPHABET.contains(&(c as u8)) {
                return None;
            }
            text.push(c);
            if text.len() > LENGTH {
                return None;
            }
        }
        (text.len() == LENGTH).then_some(Self(text))
    }

    /// The plain form (as the picture code carries it).
    pub fn plain(&self) -> &str {
        &self.0
    }

    /// As the PC shows it: four groups of four.
    pub fn shown(&self) -> String {
        self.0
            .as_bytes()
            .chunks(4)
            .map(|c| std::str::from_utf8(c).expect("ASCII"))
            .collect::<Vec<_>>()
            .join("-")
    }

    fn derive<const N: usize>(&self, info: &[u8]) -> [u8; N] {
        let hk = Hkdf::<Sha256>::new(Some(SALT), self.0.as_bytes());
        let mut out = [0u8; N];
        hk.expand(info, &mut out).expect("a short output");
        out
    }

    /// The relay's mailbox name for this code: 16 bytes from HKDF, in hex. It tells the relay
    /// which PC is waiting, and nothing about the code.
    pub fn mailbox(&self) -> String {
        hex(&self.derive::<16>(b"mailbox"))
    }

    /// The first meeting's shared key (Noise's `psk`).
    pub fn psk(&self) -> [u8; 32] {
        self.derive::<32>(b"psk")
    }

    /// The pairing address: the page's address, with the code after `#`.
    pub fn link(&self, origin: &str) -> String {
        format!("{}/#pair={}", origin.trim_end_matches('/'), self.0)
    }
}

impl Default for Code {
    fn default() -> Self {
        Self::new()
    }
}

/// Lower-case hex.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Is `text` a mailbox name (32 lower-case hex digits)?
pub fn is_mailbox(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fixed code, and what the phone's page must make from it too (the page's tests check the
    /// same values: `apps/remote/src/crypto/code.test.ts`).
    pub(crate) const FIXED: &str = "7K3QM9TX2HFDR8WB";

    #[test]
    fn codes_are_16_characters_of_80_bits() {
        let all_ones = Code::from_bits([0xff; 10]);
        assert_eq!(all_ones.plain(), "ZZZZZZZZZZZZZZZZ");
        let zero = Code::from_bits([0; 10]);
        assert_eq!(zero.plain(), "0000000000000000");
        for _ in 0..50 {
            let c = Code::new();
            assert_eq!(c.plain().len(), LENGTH);
            assert!(c.plain().bytes().all(|b| ALPHABET.contains(&b)));
            assert!(!c.plain().contains(['I', 'L', 'O', 'U']));
        }
        assert_ne!(Code::new(), Code::new());
    }

    #[test]
    fn typed_codes_are_read_kindly() {
        let c = Code::parse(FIXED).unwrap();
        assert_eq!(c.shown(), "7K3Q-M9TX-2HFD-R8WB");
        for typed in ["7k3q-m9tx-2hfd-r8wb", " 7K3Q M9TX 2HFD R8WB ", "7K3QM9TX2HFDR8WB"] {
            assert_eq!(Code::parse(typed).as_ref(), Some(&c), "{typed}");
        }
        // O is 0, and I and L are 1.
        assert_eq!(
            Code::parse("O0IL-0000-0000-0000").unwrap().plain(),
            "0011000000000000"
        );
        for bad in [
            "",
            "7K3Q-M9TX-2HFD-R8W",
            "7K3Q-M9TX-2HFD-R8WBB",
            "7K3Q-M9TX-2HFD-R8WU",
            "7K3Q-M9TX-2HFD-R8W!",
            "7K3Q-M9TX-2HFD-R8WÉ",
        ] {
            assert_eq!(Code::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn what_is_made_from_a_code_is_fixed() {
        // Written down once: the phone's page makes the same (its own test).
        let c = Code::parse(FIXED).unwrap();
        assert_eq!(c.mailbox(), "589704e9466ff61d5f38c2fec2a2f8b8");
        assert_eq!(
            hex(&c.psk()),
            "4fc5acc845bbf02456ddc2dc915c9e23883e7c669830acccb959939a4b90041c"
        );
        assert!(is_mailbox(&c.mailbox()));
        assert_eq!(
            c.link("https://remote.getplenipo.com"),
            "https://remote.getplenipo.com/#pair=7K3QM9TX2HFDR8WB"
        );
        assert_eq!(format!("{c:?}"), "Code(…)");
    }
}
