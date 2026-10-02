//! Plenipo on your phone (Phase 14): the PC's side of the sealed line to the owner's phone.
//!
//! The phone opens Plenipo's own page (`remote.getplenipo.com`, ADR-146) and reaches the PC
//! through Plenipo's own relay, run by 8 West (`crates/relay`, ADR-149). The PC calls out to the
//! relay; nothing listens on the PC. Everything the phone and the PC say is sealed end to end with the Noise protocol
//! (ADR-143): the relay passes sealed messages along, and cannot read, answer, make up, or replay
//! one.
//!
//! - **Pairing** happens at the PC (ADR-141): a picture code or a 16-letter typed code
//!   ([`code`]), a Noise `XXpsk3` first meeting with the code as its shared key, the owner's
//!   "Is this your phone?", and a passkey made on the phone ([`webauthn`]).
//! - **Every meeting after** is Noise `KK` ([`noise`]): each side already knows the other's key.
//!   The phone signs in with its passkey (ADR-142), checked here, and stays signed in for 30
//!   minutes after its last request, 12 hours at most.
//! - **Requests** are a fixed list ([`protocol::Ask`]); Guard decides each one
//!   (`plenipo_guard::remote`), and the PC's own services carry it out ([`service::Host`]).
//! - **Wrong tries** slow the PC down, then stop it for a while ([`limits`]).
//!
//! The relay's own messages are in [`wire`], written once in `contracts/phone-relay/v1` and
//! shared with Plenipo's own relay through the `plenipo-relay-contract` crate (ADR-149).

pub use plenipo_relay_contract::{b64, wire};

pub mod code;
pub mod devices;
pub mod keys;
pub mod limits;
pub mod link;
pub mod noise;
pub mod pass;
pub mod protocol;
pub mod qr;
pub mod service;
pub mod webauthn;
pub mod webpush;

#[cfg(any(test, feature = "stand-in"))]
pub mod stand_in;

#[cfg(test)]
mod contract;

pub use service::{Change, Clock, Host, Phone, Remote, Settings, SystemClock};

/// The phone's page (ADR-146): its address, and the passkeys' site.
pub const PAGE_ORIGIN: &str = "https://remote.getplenipo.com";
/// The passkeys' site: the page's host (WebAuthn's "relying party ID").
pub const RP_ID: &str = "remote.getplenipo.com";

/// Signed in for this long after the phone's last request (ADR-142 §3, the owner's answer 4).
pub const SIGNED_IN_IDLE_MS: u64 = 30 * 60 * 1000;
/// Signed in for this long at most.
pub const SIGNED_IN_MOST_MS: u64 = 12 * 60 * 60 * 1000;
/// A pairing code lasts this long (ADR-141 §5).
pub const CODE_LIFE_MS: u64 = 10 * 60 * 1000;
/// After the first meeting, the owner's "Add" and the phone's passkey must come within this.
pub const PAIRING_FINISH_MS: u64 = 5 * 60 * 1000;
/// A sign-in challenge works once, for this long.
pub const CHALLENGE_LIFE_MS: u64 = 2 * 60 * 1000;
/// A relay pass lasts this long, and is renewed at every sign-in (ADR-147, which changes
/// ADR-143 §4's 7 days).
pub const PASS_LIFE_SECS: i64 = 90 * 24 * 60 * 60;
/// The most phones one PC keeps.
pub const MAX_DEVICES: usize = 20;
/// The largest sealed message (Noise's own limit, and the largest the relay passes).
pub const MAX_NOISE_MESSAGE: usize = wire::MAX_DATA;
/// The largest message assembled from sealed pieces.
pub const MAX_ASSEMBLED: usize = 4 * 1024 * 1024;

/// Why something about the phone did not work, in plain words.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RemoteError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Refused(String),
    #[error("{0}")]
    Store(String),
}

pub type Result<T> = std::result::Result<T, RemoteError>;

/// Random bytes from the operating system.
pub fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("the operating system gives random bytes");
    bytes
}
