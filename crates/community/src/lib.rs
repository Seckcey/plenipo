//! Community (Phase 24): Plenipo's side of the contract with 8 West's account service.
//!
//! The contract is written once, in `contracts/community/v1`, and the account service keeps a
//! copy of it (ADR-160 §6). This crate is that contract in code, for the PC, with nothing that
//! touches the network, the Vault, or the screen:
//!
//! - [`wire`]: every request and answer, as the contract's schema names them.
//! - [`ids`]: the contract's IDs (`cm_…`, `ci_…`, and the rest).
//! - [`keys`]: each PC's own Community keys (ADR-162 §3): Ed25519 for signing, X25519 for
//!   sealing. The account service learns only the public halves.
//! - [`seal`]: HPKE (RFC 9180), sealing for one PC (ADR-164 §1, contract §7).
//! - [`item`]: sealing an item for every PC it is for, and opening one, with the checks the
//!   contract lists, in its order. Other people's words never reach anything else unchecked.
//! - [`stamp`]: 8 West's stamp on each item, which makes a report provable (ADR-164 §6).
//! - [`safety`]: the 12-digit safety code two people can compare (ADR-164 §2).
//! - `stand_in` (tests only, behind the `stand-in` feature): 8 West's account service as the
//!   contract describes it, in memory, so Plenipo's own tests have a service to talk to.
//!
//! Plenipo is made by 8 West Ventures, LLC.

pub mod b64;
pub mod client;
pub mod ids;
pub mod item;
pub mod keys;
pub mod safety;
pub mod seal;
pub mod service;
pub mod session;
pub mod stamp;
pub mod wire;

/// 8 West's account service as the contract describes it, in memory, for the tests only.
#[cfg(any(test, feature = "stand-in"))]
pub mod stand_in;

#[cfg(test)]
mod vectors;

/// Why something about Community did not work, in plain words. Never holds a key, a pass, or a
/// message's words.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommunityError {
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, CommunityError>;

/// Random bytes from the operating system.
pub fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).expect("the operating system gives random bytes");
    bytes
}
