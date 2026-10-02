//! The lock between the phone and the PC (ADR-143 §5): the Noise protocol, through `snow`.
//!
//! - **Pairing:** [`PAIRING`], with the pairing code made into the shared key (`psk`, at the end
//!   of the third message). Each side learns the other's long-term key, and only with the right
//!   code.
//! - **Every time after:** [`EVERYDAY`]. Each side already knows the other's long-term key, so a
//!   stranger, or the relay, fails the very first message. Each meeting makes fresh keys.
//!
//! In both, the phone starts (the initiator) and the PC answers (the responder). After the
//! meeting, every message carries a counter: one copied, changed, dropped, or out of order fails.
//!
//! A message longer than Noise allows travels in **pieces**: each sealed piece's first byte says
//! whether more follow ([`MORE`]) or it is the last ([`LAST`]).

use snow::params::DHChoice;
use snow::resolvers::{CryptoResolver as _, DefaultResolver};
use snow::{Builder, HandshakeState, TransportState};

use crate::{RemoteError, Result, MAX_ASSEMBLED, MAX_NOISE_MESSAGE};

/// The first meeting, with the pairing code.
pub const PAIRING: &str = "Noise_XXpsk3_25519_AESGCM_SHA256";
/// Every meeting after.
pub const EVERYDAY: &str = "Noise_KK_25519_AESGCM_SHA256";
/// What both sides agree on before the first meeting's first message.
pub const PAIRING_PROLOGUE: &[u8] = b"plenipo-remote.v1/pair";
/// The AES-GCM tag on every sealed message.
pub const TAG: usize = 16;
/// A piece that more follow.
pub const MORE: u8 = 1;
/// The last (or only) piece.
pub const LAST: u8 = 0;
/// The most plain bytes in one piece, after its first byte.
pub const PIECE: usize = MAX_NOISE_MESSAGE - TAG - 1;

/// What both sides agree on before an everyday meeting: the PC and the phone it is for.
pub fn everyday_prologue(pc: &str, phone: &str) -> Vec<u8> {
    format!("plenipo-remote.v1/kk/{pc}/{phone}").into_bytes()
}

/// A new X25519 key pair: (private, public).
pub fn new_keypair() -> ([u8; 32], [u8; 32]) {
    let private: [u8; 32] = crate::random();
    let public = public_of(&private);
    (private, public)
}

/// The public half of an X25519 private key.
pub fn public_of(private: &[u8; 32]) -> [u8; 32] {
    let mut dh = DefaultResolver
        .resolve_dh(&DHChoice::Curve25519)
        .expect("X25519 is built in");
    dh.set(private);
    dh.pubkey().try_into().expect("32 bytes")
}

fn params(name: &str) -> snow::params::NoiseParams {
    name.parse().expect("a pattern Plenipo names")
}

fn broken(e: snow::Error) -> RemoteError {
    RemoteError::Refused(format!("the sealed meeting failed ({e})"))
}

/// The PC's side of a first meeting.
pub fn pairing_pc(pc_private: &[u8; 32], psk: &[u8; 32]) -> Result<HandshakeState> {
    Builder::new(params(PAIRING))
        .local_private_key(pc_private)
        .and_then(|b| b.prologue(PAIRING_PROLOGUE))
        .and_then(|b| b.psk(3, psk))
        .and_then(Builder::build_responder)
        .map_err(broken)
}

/// The phone's side of a first meeting (the stand-in phone, and the tests).
pub fn pairing_phone(phone_private: &[u8; 32], psk: &[u8; 32]) -> Result<HandshakeState> {
    Builder::new(params(PAIRING))
        .local_private_key(phone_private)
        .and_then(|b| b.prologue(PAIRING_PROLOGUE))
        .and_then(|b| b.psk(3, psk))
        .and_then(Builder::build_initiator)
        .map_err(broken)
}

/// The PC's side of an everyday meeting with the phone whose key is `phone_public`.
pub fn everyday_pc(
    pc_private: &[u8; 32],
    phone_public: &[u8; 32],
    prologue: &[u8],
) -> Result<HandshakeState> {
    Builder::new(params(EVERYDAY))
        .local_private_key(pc_private)
        .and_then(|b| b.remote_public_key(phone_public))
        .and_then(|b| b.prologue(prologue))
        .and_then(Builder::build_responder)
        .map_err(broken)
}

/// The phone's side of an everyday meeting (the stand-in phone, and the tests).
pub fn everyday_phone(
    phone_private: &[u8; 32],
    pc_public: &[u8; 32],
    prologue: &[u8],
) -> Result<HandshakeState> {
    Builder::new(params(EVERYDAY))
        .local_private_key(phone_private)
        .and_then(|b| b.remote_public_key(pc_public))
        .and_then(|b| b.prologue(prologue))
        .and_then(Builder::build_initiator)
        .map_err(broken)
}

/// Write one meeting message carrying `payload`.
pub fn write(hs: &mut HandshakeState, payload: &[u8]) -> Result<Vec<u8>> {
    let mut out = vec![0u8; MAX_NOISE_MESSAGE];
    let n = hs.write_message(payload, &mut out).map_err(broken)?;
    out.truncate(n);
    Ok(out)
}

/// Read one meeting message, giving its payload.
pub fn read(hs: &mut HandshakeState, message: &[u8]) -> Result<Vec<u8>> {
    if message.len() > MAX_NOISE_MESSAGE {
        return Err(RemoteError::Refused("a sealed message was too long".into()));
    }
    let mut out = vec![0u8; MAX_NOISE_MESSAGE];
    let n = hs.read_message(message, &mut out).map_err(broken)?;
    out.truncate(n);
    Ok(out)
}

/// Seal a whole message, in as many pieces as it needs.
pub fn seal(transport: &mut TransportState, message: &[u8]) -> Result<Vec<Vec<u8>>> {
    if message.len() > MAX_ASSEMBLED {
        return Err(RemoteError::Invalid("a message was too long to send".into()));
    }
    let mut pieces = Vec::new();
    let mut chunks = message.chunks(PIECE).peekable();
    // An empty message is one empty last piece.
    if chunks.peek().is_none() {
        pieces.push(seal_piece(transport, LAST, &[])?);
    }
    while let Some(chunk) = chunks.next() {
        let flag = if chunks.peek().is_some() { MORE } else { LAST };
        pieces.push(seal_piece(transport, flag, chunk)?);
    }
    Ok(pieces)
}

fn seal_piece(transport: &mut TransportState, flag: u8, chunk: &[u8]) -> Result<Vec<u8>> {
    let mut plain = Vec::with_capacity(chunk.len() + 1);
    plain.push(flag);
    plain.extend_from_slice(chunk);
    let mut out = vec![0u8; plain.len() + TAG];
    let n = transport.write_message(&plain, &mut out).map_err(broken)?;
    out.truncate(n);
    Ok(out)
}

/// Open one sealed piece: (more follow?, its bytes).
pub fn open(transport: &mut TransportState, sealed: &[u8]) -> Result<(bool, Vec<u8>)> {
    if sealed.len() > MAX_NOISE_MESSAGE || sealed.len() < TAG + 1 {
        return Err(RemoteError::Refused("a sealed message was the wrong size".into()));
    }
    let mut plain = vec![0u8; sealed.len()];
    let n = transport.read_message(sealed, &mut plain).map_err(broken)?;
    plain.truncate(n);
    match plain.first() {
        Some(&MORE) => Ok((true, plain.split_off(1))),
        Some(&LAST) => Ok((false, plain.split_off(1))),
        _ => Err(RemoteError::Refused("a sealed message was not one of ours".into())),
    }
}

/// Puts pieces back together.
#[derive(Debug, Default)]
pub struct Assembler {
    buf: Vec<u8>,
}

impl Assembler {
    /// Add one opened piece; the whole message when it was the last.
    pub fn add(&mut self, more: bool, bytes: Vec<u8>) -> Result<Option<Vec<u8>>> {
        if self.buf.len() + bytes.len() > MAX_ASSEMBLED {
            self.buf.clear();
            return Err(RemoteError::Refused("a message was too long".into()));
        }
        if more {
            self.buf.extend_from_slice(&bytes);
            return Ok(None);
        }
        if self.buf.is_empty() {
            return Ok(Some(bytes));
        }
        let mut whole = std::mem::take(&mut self.buf);
        whole.extend_from_slice(&bytes);
        Ok(Some(whole))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn kk_pair(prologue: &[u8]) -> (TransportState, TransportState) {
        let pc = new_keypair();
        let phone = new_keypair();
        let mut i = everyday_phone(&phone.0, &pc.1, prologue).unwrap();
        let mut r = everyday_pc(&pc.0, &phone.1, prologue).unwrap();
        let m1 = write(&mut i, b"hello").unwrap();
        assert_eq!(read(&mut r, &m1).unwrap(), b"hello");
        let m2 = write(&mut r, b"welcome").unwrap();
        assert_eq!(read(&mut i, &m2).unwrap(), b"welcome");
        (
            i.into_transport_mode().unwrap(),
            r.into_transport_mode().unwrap(),
        )
    }

    #[test]
    fn an_everyday_meeting_needs_both_known_keys() {
        let prologue = everyday_prologue("pc", "phone");
        let (mut phone, mut pc) = kk_pair(&prologue);
        let pieces = seal(&mut phone, b"approve").unwrap();
        assert_eq!(pieces.len(), 1);
        assert_eq!(open(&mut pc, &pieces[0]).unwrap(), (false, b"approve".to_vec()));

        // A stranger who knows the PC's public key, but is not the phone the PC knows.
        let pc_keys = new_keypair();
        let phone_keys = new_keypair();
        let stranger = new_keypair();
        let mut i = everyday_phone(&stranger.0, &pc_keys.1, &prologue).unwrap();
        let mut r = everyday_pc(&pc_keys.0, &phone_keys.1, &prologue).unwrap();
        let m1 = write(&mut i, b"").unwrap();
        assert!(read(&mut r, &m1).is_err(), "the PC refused the stranger");

        // The same phone, but a prologue for another phone ID.
        let mut i = everyday_phone(&phone_keys.0, &pc_keys.1, &prologue).unwrap();
        let mut r =
            everyday_pc(&pc_keys.0, &phone_keys.1, &everyday_prologue("pc", "other")).unwrap();
        let m1 = write(&mut i, b"").unwrap();
        assert!(read(&mut r, &m1).is_err());
    }

    #[test]
    fn the_first_meeting_needs_the_right_code() {
        let pc = new_keypair();
        let phone = new_keypair();
        let right = [1u8; 32];
        let wrong = [2u8; 32];
        for (psk, works) in [(right, true), (wrong, false)] {
            let mut i = pairing_phone(&phone.0, &psk).unwrap();
            let mut r = pairing_pc(&pc.0, &right).unwrap();
            let m1 = write(&mut i, b"").unwrap();
            read(&mut r, &m1).unwrap();
            let m2 = write(&mut r, b"").unwrap();
            read(&mut i, &m2).unwrap();
            let m3 = write(&mut i, br#"{"name":"phone"}"#).unwrap();
            let got = read(&mut r, &m3);
            assert_eq!(got.is_ok(), works);
            if works {
                assert_eq!(r.get_remote_static().unwrap(), phone.1);
                assert_eq!(i.get_remote_static().unwrap(), pc.1);
            }
        }
    }

    #[test]
    fn a_copied_changed_or_reordered_message_fails() {
        let (mut phone, mut pc) = kk_pair(b"p");
        let a = seal(&mut phone, b"first").unwrap().remove(0);
        let b = seal(&mut phone, b"second").unwrap().remove(0);
        // Out of order.
        assert!(open(&mut pc, &b).is_err());
        let (mut phone, mut pc) = kk_pair(b"p");
        let a2 = seal(&mut phone, b"first").unwrap().remove(0);
        assert!(open(&mut pc, &a2).is_ok());
        // Copied.
        assert!(open(&mut pc, &a2).is_err());
        // From another meeting.
        assert!(open(&mut pc, &a).is_err());
        // Changed.
        let (mut phone, mut pc) = kk_pair(b"p");
        let mut c = seal(&mut phone, b"first").unwrap().remove(0);
        c[3] ^= 1;
        assert!(open(&mut pc, &c).is_err());
    }

    #[test]
    fn long_messages_travel_in_pieces() {
        let (mut phone, mut pc) = kk_pair(b"p");
        let long: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let pieces = seal(&mut pc, &long).unwrap();
        assert_eq!(pieces.len(), 4);
        let mut assembler = Assembler::default();
        let mut whole = None;
        for p in &pieces {
            assert!(p.len() <= MAX_NOISE_MESSAGE);
            let (more, bytes) = open(&mut phone, p).unwrap();
            whole = assembler.add(more, bytes).unwrap();
        }
        assert_eq!(whole.unwrap(), long);
        // An empty message is one piece.
        let pieces = seal(&mut pc, b"").unwrap();
        assert_eq!(pieces.len(), 1);
        let (more, bytes) = open(&mut phone, &pieces[0]).unwrap();
        assert_eq!(Assembler::default().add(more, bytes).unwrap(), Some(vec![]));
    }
}
