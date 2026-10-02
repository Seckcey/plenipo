//! Plenipo's own relay for Plenipo on your phone (Phase 14; ADR-143, the relay and the lock;
//! ADR-149, Plenipo runs its own relay). Made by 8 West Ventures, LLC.
//!
//! **In plain words:** a Pro copy of Plenipo connects out to this relay, and the owner's phones
//! connect too. The relay passes **sealed** messages between a PC and its own phones, and does
//! nothing else with them. It cannot read them. It keeps nothing: no queue, no copies, no message
//! held for later.
//!
//! It speaks exactly the written contract, `contracts/phone-relay/v1`, from the same code the PC
//! uses (`plenipo-relay-contract`):
//!
//! - a PC proves it holds its relay key (an Ed25519 signature over a fresh challenge) and shows
//!   8 West's signed weekly answer, checked with 8 West's public license keys
//!   (`plenipo_licensing::trust`); then the relay knows it by its key's fingerprint;
//! - a phone shows a **pass** its PC signed (90 days, ADR-147), or joins its PC's pairing
//!   **mailbox**;
//! - sealed messages of at most 65,535 bytes go between a PC and its own phones, in order, as they
//!   are.
//!
//! **Hardened for the internet:** limits per address (IPv6 by its /64) and per PC, a cap on all
//! connections, a budget of messages and bytes per minute, idle timeouts and pings, an **off
//! switch**, `/healthz`, graceful shutdown, and logs that hold counts, codes, and addresses only:
//! never a message, a key, a pass, a weekly answer, a mailbox name, or a challenge.
//!
//! It listens on `127.0.0.1` only. A proxy in front of it (Nginx Proxy Manager) ends TLS and
//! brings `relay.getplenipo.com` to it, so this program carries no certificate and no secret.
//!
//! [`Relay::start`] runs one; [`Handle`] watches and stops it. The program in `main.rs` reads its
//! settings, and the tests run Plenipo's own PC side and phone through it.

mod http;
mod hub;
mod limits;
mod line;
mod pc;
mod phone;

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::Notify;

pub use limits::{AddressKey, Limits};

/// A pairing mailbox's name, as the contract writes it: 32 lower-case hex digits.
pub(crate) fn is_mailbox_name(name: &str) -> bool {
    name.len() == 32
        && name
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A sealed message's text fits the contract: base64url of at most 65,535 bytes.
pub(crate) fn fits(data: &str) -> bool {
    use plenipo_relay_contract::{b64, wire};
    data.len() <= wire::MAX_DATA.div_ceil(3) * 4 && b64::decode(data, wire::MAX_DATA).is_some()
}

/// Is `ip` an address only this machine and its containers can reach: loopback, or a private
/// range (`10/8`, `172.16/12`, `192.168/16`, link-local; IPv6 loopback, unique local, link-local)?
pub fn stays_on_this_machine(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local()
        }
    }
}

/// 32 random bytes from the operating system (a challenge).
pub(crate) fn random32() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the operating system gives random bytes");
    bytes
}

/// Where the relay reads a connection's internet address, for its per-address limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClientAddress {
    /// The connection's own address. For a relay reached directly (the tests).
    Peer,
    /// Behind a proxy on this machine: the last address the proxy appended to `X-Forwarded-For`
    /// (or `X-Real-IP`), which the proxy saw itself. The default.
    #[default]
    Proxy,
    /// Behind a proxy and Cloudflare: `CF-Connecting-IP`, which Cloudflare sets to the phone's or
    /// PC's own address; the proxy's headers when it is missing.
    Cloudflare,
}

/// The clock, in Unix seconds (the tests move it).
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

/// How one relay runs.
#[derive(Clone)]
pub struct Config {
    /// Where it listens: `127.0.0.1` and a port. It refuses any other address.
    pub listen: SocketAddr,
    pub client_address: ClientAddress,
    pub limits: Limits,
    pub clock: Clock,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:8790".parse().expect("an address"),
            client_address: ClientAddress::default(),
            limits: Limits::default(),
            clock: Arc::new(plenipo_licensing::clock),
        }
    }
}

/// Counts, for `/healthz`'s log line and the tests. Never contents.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stats {
    pub connections: usize,
    pub pcs: usize,
    pub phones: usize,
    /// Refusals so far, by code.
    pub refused: BTreeMap<String, u64>,
    /// Connections turned away at the door (over a limit, or while off).
    pub turned_away: u64,
    pub off: bool,
}

/// A running relay.
pub struct Relay;

/// What a started relay gives back: where it listens, its counts, its switches.
#[derive(Clone)]
pub struct Handle {
    address: SocketAddr,
    hub: Arc<hub::Hub>,
    stop: Arc<Notify>,
}

impl Relay {
    /// Start a relay on `config.listen`: `127.0.0.1`, or one of this machine's private addresses
    /// (for a proxy that runs in a container and cannot reach the host's loopback). Never an
    /// address the internet can reach: the proxy in front of it is what the internet reaches.
    pub async fn start(config: Config) -> std::io::Result<Handle> {
        if !stays_on_this_machine(config.listen.ip()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "the relay listens on 127.0.0.1 (or a private address of this machine, for a \
                     proxy in a container), never on {}",
                    config.listen.ip()
                ),
            ));
        }
        let listener = TcpListener::bind(config.listen).await?;
        let address = listener.local_addr()?;
        let hub = Arc::new(hub::Hub::new(config));
        let stop = Arc::new(Notify::new());
        let handle = Handle {
            address,
            hub: hub.clone(),
            stop: stop.clone(),
        };
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accepted = listener.accept() => match accepted {
                        Ok((stream, peer)) => {
                            let hub = hub.clone();
                            tokio::spawn(async move { http::serve(hub, stream, peer).await });
                        }
                        Err(e) => {
                            // Out of file handles, most likely: wait a moment, then go on.
                            log::warn!("accept failed: {e}");
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    },
                    _ = stop.notified() => break,
                }
            }
        });
        Ok(handle)
    }
}

impl Handle {
    /// Where the relay listens.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The address a PC connects to (`http://`, as Guard's test rule and the tests use).
    pub fn pc_address(&self) -> String {
        format!("http://{}/plenipo/v1/pc", self.address)
    }

    /// The address a phone connects to.
    pub fn phone_address(&self) -> String {
        format!("ws://{}/plenipo/v1/phone", self.address)
    }

    pub fn stats(&self) -> Stats {
        self.hub.stats()
    }

    /// The off switch: off closes every connection now and turns new ones away (`503`) until it
    /// is on again. `/healthz` says `off`.
    pub fn set_off(&self, off: bool) {
        self.hub.set_off(off);
    }

    /// Stop taking connections, close every connection, and wait (at most `grace`) for them to
    /// go.
    pub async fn shutdown(&self, grace: Duration) {
        self.stop.notify_one();
        self.hub.close_all();
        let until = tokio::time::Instant::now() + grace;
        while self.hub.stats().connections > 0 && tokio::time::Instant::now() < until {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Is any PC connected? (The tests, and the program's test-hooks lines.)
    pub fn pc_connected(&self) -> bool {
        self.hub.stats().pcs > 0
    }

    /// Every refusal so far, in order, by code (the tests only: the relay keeps counts).
    #[cfg(feature = "test-hooks")]
    pub fn refused(&self) -> Vec<String> {
        self.hub.refused_in_order()
    }

    /// The pairing mailbox a PC has open, if any (the tests only).
    #[cfg(feature = "test-hooks")]
    pub fn mailbox(&self) -> Option<String> {
        self.hub.mailbox_open()
    }

    /// Every sealed message passed so far (the tests only: to prove the words never showed).
    #[cfg(feature = "test-hooks")]
    pub fn seen(&self) -> Vec<Vec<u8>> {
        self.hub.seen()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_relay_listens_only_where_the_internet_cannot_reach() {
        for ok in [
            "127.0.0.1",
            "127.0.0.9",
            "::1",
            "172.17.0.1",
            "10.0.0.5",
            "192.168.1.2",
            "fd00::1",
        ] {
            assert!(stays_on_this_machine(ok.parse().unwrap()), "{ok}");
        }
        for no in ["0.0.0.0", "::", "203.0.113.9", "8.8.8.8", "2001:db8::1"] {
            assert!(!stays_on_this_machine(no.parse().unwrap()), "{no}");
        }
    }

    #[tokio::test]
    async fn a_public_address_is_refused_before_anything_listens() {
        let config = Config {
            listen: "0.0.0.0:0".parse().unwrap(),
            ..Config::default()
        };
        let err = Relay::start(config).await.err().expect("refused");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("never on 0.0.0.0"), "{err}");
    }
}
