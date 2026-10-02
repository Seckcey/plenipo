//! The relay's own messages (`contracts/phone-relay/v1`, ADR-143, the change request for the
//! relay's repository).
//!
//! Each WebSocket text message is one JSON object with a `t` field. The PC uses
//! `/plenipo/v1/pc`, phones use `/plenipo/v1/phone`. Sealed messages travel as `data`, in
//! base64url; the relay passes them along as they are and cannot read them.

use serde::{Deserialize, Serialize};

pub use plenipo_licensing::SignedAnswer;

/// What the PC's proof of its relay key covers, before the relay's random challenge.
pub const PC_PROOF_CONTEXT: &str = "plenipo-relay-pc.v1.";
/// The largest relay message (a 64 KB sealed message in base64url, and its envelope).
pub const MAX_RELAY_MESSAGE: usize = 96 * 1024;
/// The largest sealed message the relay passes.
pub const MAX_DATA: usize = crate::MAX_NOISE_MESSAGE;

/// The relay to the PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelayToPc {
    /// First: prove you hold your relay key by signing this.
    Challenge { nonce: String },
    /// The relay knows the PC now, by its key's fingerprint.
    Welcome { pc: String },
    /// The relay will not serve this PC (and closes).
    Refused { code: String },
    /// A phone connected: with a pass (`phone`), or to the pairing mailbox (`mailbox`).
    Joined {
        conn: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        phone: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        mailbox: bool,
    },
    /// A sealed message from a phone.
    Data { conn: String, data: String },
    /// A phone's connection closed.
    Left { conn: String },
    /// Something the PC sent was not passed on (for example `too_big`).
    Error { code: String },
}

/// The PC to the relay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case", deny_unknown_fields)]
pub enum PcToRelay {
    /// The PC's relay key, its proof, and 8 West's newest signed weekly answer (the relay checks
    /// that the PC is Pro, ADR-143 §3).
    Hello {
        v: u32,
        key: String,
        proof: String,
        answer: SignedAnswer,
    },
    /// Open the pairing mailbox (one at a time; a new one replaces the old).
    Mailbox { mailbox: String },
    /// Close the pairing mailbox.
    CloseMailbox,
    /// Stop passing this phone's messages, now and until `until` (Unix seconds).
    Drop { phone: String, until: i64 },
    /// A sealed message to one phone's connection.
    Send { conn: String, data: String },
    /// Close one phone's connection.
    Close { conn: String },
}

/// A phone to the relay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case", deny_unknown_fields)]
pub enum PhoneToRelay {
    /// Connect to the PC that signed this pass.
    Pass { pass: String },
    /// Connect to the PC waiting at this pairing mailbox.
    Mailbox { mailbox: String },
    /// A sealed message to the PC.
    Data { data: String },
}

/// The relay to a phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case", deny_unknown_fields)]
pub enum RelayToPhone {
    /// Connected to the PC.
    Ready,
    /// Not connected (and closes): `pc_offline`, `bad_pass`, `mailbox_closed`,
    /// `too_many_tries`.
    Refused { code: String },
    /// A sealed message from the PC.
    Data { data: String },
    /// The PC went away (and the relay closes).
    PcOffline,
}

/// The relay's short codes (written in the contract).
pub mod codes {
    pub const NOT_PRO: &str = "not_pro";
    pub const BAD_PROOF: &str = "bad_proof";
    pub const BAD_HELLO: &str = "bad_hello";
    pub const PC_OFFLINE: &str = "pc_offline";
    pub const BAD_PASS: &str = "bad_pass";
    pub const MAILBOX_CLOSED: &str = "mailbox_closed";
    pub const TOO_MANY_TRIES: &str = "too_many_tries";
    pub const TOO_BIG: &str = "too_big";
    pub const UNKNOWN_CONN: &str = "unknown_conn";
    pub const ALL: [&str; 9] = [
        NOT_PRO,
        BAD_PROOF,
        BAD_HELLO,
        PC_OFFLINE,
        BAD_PASS,
        MAILBOX_CLOSED,
        TOO_MANY_TRIES,
        TOO_BIG,
        UNKNOWN_CONN,
    ];
}

/// Read one relay message, refusing anything too long or not one of these.
pub fn read<T: for<'de> Deserialize<'de>>(text: &str) -> Option<T> {
    if text.len() > MAX_RELAY_MESSAGE {
        return None;
    }
    serde_json::from_str(text).ok()
}

/// Write one relay message.
pub fn write<T: Serialize>(message: &T) -> String {
    serde_json::to_string(message).expect("relay messages are JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_messages_read_and_write_as_the_contract_says() {
        let m: RelayToPc = read(r#"{"t":"joined","conn":"c1","phone":"p"}"#).unwrap();
        assert_eq!(
            m,
            RelayToPc::Joined {
                conn: "c1".into(),
                phone: Some("p".into()),
                mailbox: false
            }
        );
        assert_eq!(write(&m), r#"{"t":"joined","conn":"c1","phone":"p"}"#);
        let m: RelayToPc = read(r#"{"t":"joined","conn":"c2","mailbox":true}"#).unwrap();
        assert_eq!(write(&m), r#"{"t":"joined","conn":"c2","mailbox":true}"#);
        assert_eq!(write(&PcToRelay::CloseMailbox), r#"{"t":"close_mailbox"}"#);
        assert_eq!(write(&RelayToPhone::PcOffline), r#"{"t":"pc_offline"}"#);
        // Nothing else.
        assert!(read::<RelayToPc>(r#"{"t":"joined","conn":"c","extra":1}"#).is_none());
        assert!(read::<RelayToPc>(r#"{"t":"run","command":"rm"}"#).is_none());
        assert!(read::<PcToRelay>(&format!(
            r#"{{"t":"send","conn":"c","data":"{}"}}"#,
            "A".repeat(MAX_RELAY_MESSAGE)
        ))
        .is_none());
    }
}
