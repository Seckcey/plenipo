//! The contract's IDs (contract §1): a prefix, `_`, and 26 characters of Crockford base 32 in
//! capitals, made at random. The service makes most of them; a PC makes only item IDs (`ci_`)
//! and the organization IDs it shows one link or collaboration (`co_`).

/// Crockford base 32: `0-9` and `A-Z` without `I`, `L`, `O`, and `U`.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// The characters after the prefix.
const LENGTH: usize = 26;

/// Which kind of thing an ID names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdKind {
    /// A member of Community (`cm_`).
    Member,
    /// A PC signed in to Community (`cd_`).
    Device,
    /// A sealed item (`ci_`), made by the sending PC.
    Item,
    /// A link between two organizations (`cl_`).
    Link,
    /// A collaboration (`cc_`).
    Collab,
    /// A report (`cr_`).
    Report,
    /// An organization as one link or collaboration knows it (`co_`), made by the PC that owns
    /// it: never the organization's real ID.
    Org,
}

impl IdKind {
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Member => "cm_",
            Self::Device => "cd_",
            Self::Item => "ci_",
            Self::Link => "cl_",
            Self::Collab => "cc_",
            Self::Report => "cr_",
            Self::Org => "co_",
        }
    }
}

/// Whether `text` is an ID of this kind.
pub fn is_id(kind: IdKind, text: &str) -> bool {
    text.strip_prefix(kind.prefix())
        .is_some_and(|rest| rest.len() == LENGTH && rest.bytes().all(|b| ALPHABET.contains(&b)))
}

/// A new random ID of this kind: 130 random bits.
pub fn new_id(kind: IdKind) -> String {
    let bytes: [u8; LENGTH] = crate::random();
    let mut id = String::with_capacity(3 + LENGTH);
    id.push_str(kind.prefix());
    // 256 is a multiple of 32, so every character is equally likely.
    id.extend(bytes.iter().map(|b| ALPHABET[usize::from(b % 32)] as char));
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_the_contracts() {
        assert!(is_id(IdKind::Member, "cm_01JA2B3C4D5E6F7G8H9J0K1M2N"));
        assert!(is_id(IdKind::Item, "ci_01JC0D1E2F3G4H5J6K7M8N9P0Q"));
        assert!(
            !is_id(IdKind::Device, "cm_01JA2B3C4D5E6F7G8H9J0K1M2N"),
            "another kind"
        );
        assert!(
            !is_id(IdKind::Member, "cm_01JA2B3C4D5E6F7G8H9J0K1M2"),
            "too short"
        );
        assert!(
            !is_id(IdKind::Member, "cm_01JA2B3C4D5E6F7G8H9J0K1M2NN"),
            "too long"
        );
        assert!(
            !is_id(IdKind::Member, "cm_01JA2B3C4D5E6F7G8H9J0K1M2I"),
            "I is not in it"
        );
        assert!(
            !is_id(IdKind::Member, "cm_01ja2b3c4d5e6f7g8h9j0k1m2n"),
            "capitals only"
        );
        for kind in [IdKind::Item, IdKind::Org] {
            let id = new_id(kind);
            assert!(is_id(kind, &id), "{id}");
            assert_ne!(new_id(kind), id);
        }
    }
}
