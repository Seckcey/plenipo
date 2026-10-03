//! The safety code (ADR-164 §2, contract §7): 12 digits made from both people's PCs' keys, the
//! same on both sides, that two people can compare by phone or in person (**Check the safety
//! code**). Because 8 West hands out the keys, this is how two people can tell nobody swapped
//! one. A new, removed, or changed PC of either person changes it.

use sha2::{Digest as _, Sha256};

use crate::wire::DevicePublic;

/// The label the code is made under.
pub const SAFETY_CONTEXT: &str = "plenipo-community-safety.v1.";

/// One person's line: their member ID, `:`, then each PC as `<signing_key>.<sealing_key>`,
/// sorted, joined with `,`.
fn line(member_id: &str, pcs: &[DevicePublic]) -> String {
    let mut keys: Vec<String> = pcs
        .iter()
        .map(|pc| format!("{}.{}", pc.signing_key, pc.sealing_key))
        .collect();
    keys.sort();
    format!("{member_id}:{}", keys.join(","))
}

/// The safety code for two people and their PCs, as `1234 5678 9012`. It does not matter which
/// person comes first.
pub fn safety_code(a: (&str, &[DevicePublic]), b: (&str, &[DevicePublic])) -> String {
    let mut lines = [line(a.0, a.1), line(b.0, b.1)];
    lines.sort();
    let mut hash = Sha256::new();
    hash.update(SAFETY_CONTEXT.as_bytes());
    hash.update(lines.join("\n").as_bytes());
    let digest = hash.finalize();
    let mut first = [0u8; 8];
    first.copy_from_slice(&digest[..8]);
    let digits = format!("{:012}", u64::from_be_bytes(first) % 1_000_000_000_000);
    format!("{} {} {}", &digits[0..4], &digits[4..8], &digits[8..12])
}
