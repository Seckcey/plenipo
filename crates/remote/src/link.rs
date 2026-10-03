//! The PC's link to 8 West's relay (ADR-143 §1–§3): the PC calls out, so nothing listens on the
//! PC or the router.
//!
//! Each time it connects, the link asks Guard (the **phone access** purpose: Plenipo's relay
//! only), proves the PC holds its relay key, and shows 8 West's newest signed weekly answer, so
//! the relay knows the PC is Pro. Then it passes what the relay says to [`Remote`] and sends what
//! [`Remote`] asks, until the connection ends; it tries again after a wait that grows.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::{SinkExt as _, StreamExt as _};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;

use crate::b64;
use crate::service::{Remote, ToRelay};
use crate::wire::{self, codes, PcToRelay, RelayToPc, SignedAnswer, PC_PROOF_CONTEXT};

/// What the link needs from the app each time it connects.
pub trait LinkHost: Send + Sync + 'static {
    /// Guard's check of the relay's address (**phone access**). `Err`: why not, in plain words
    /// (Guard records it).
    fn check_address(&self, address: &str) -> Result<(), String>;
    /// 8 West's newest signed weekly answer (`None`: none yet).
    fn weekly_answer(&self) -> Option<SignedAnswer>;
}

/// How long to wait for the relay at each step.
const STEP: Duration = Duration::from_secs(20);
/// How often the service looks at sign-ins and codes.
const TICK: Duration = Duration::from_secs(5);
/// How often the link shows it is alive (Cloudflare closes quiet connections).
const KEEP_ALIVE: Duration = Duration::from_secs(30);
/// A connection that lasted this long was a good one: the next wait starts short again.
const GOOD_RUN: Duration = Duration::from_secs(60);

/// Why a connection ended, and how long to wait before the next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// In plain words, for Settings → Devices.
    pub message: String,
    /// Wait at least this long (`None`: the growing wait).
    pub wait: Option<Duration>,
}

impl Problem {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            wait: None,
        }
    }

    fn after(message: impl Into<String>, wait: Duration) -> Self {
        Self {
            message: message.into(),
            wait: Some(wait),
        }
    }
}

/// The waits between tries: 1, 2, 5, 10, 30, then 60 seconds.
#[derive(Debug, Default)]
pub struct Waits(usize);

impl Waits {
    const STEPS: [u64; 6] = [1, 2, 5, 10, 30, 60];

    pub fn wait(&mut self) -> Duration {
        let secs = Self::STEPS[self.0.min(Self::STEPS.len() - 1)];
        self.0 += 1;
        Duration::from_secs(secs)
    }

    pub fn reset(&mut self) {
        self.0 = 0;
    }
}

/// The WebSocket address for `address` (`https:` → `wss:`, the tests' `http:` → `ws:`).
pub fn socket_address(address: &str) -> String {
    if let Some(rest) = address.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = address.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        address.to_owned()
    }
}

/// Keep the PC connected to the relay at `address`, for as long as this runs (the app stops it
/// when phone access is off, or Pro ends).
pub async fn run(remote: Arc<Remote>, address: String, host: Arc<dyn LinkHost>) {
    let mut waits = Waits::default();
    loop {
        let started = Instant::now();
        let result = connect_once(&remote, &address, host.as_ref()).await;
        remote.relay_down(None);
        let wait = match result {
            Ok(()) if started.elapsed() >= GOOD_RUN => {
                waits.reset();
                waits.wait()
            }
            Ok(()) => waits.wait(),
            Err(problem) => {
                log::info!("phone access: {}", problem.message);
                remote.set_relay_problem(Some(problem.message.clone()));
                problem.wait.unwrap_or_else(|| waits.wait())
            }
        };
        tokio::time::sleep(wait).await;
    }
}

/// One connection, from the first message to its end.
pub async fn connect_once(
    remote: &Arc<Remote>,
    address: &str,
    host: &dyn LinkHost,
) -> Result<(), Problem> {
    host.check_address(address)
        .map_err(|why| Problem::after(why, Duration::from_secs(600)))?;
    let answer = host.weekly_answer().ok_or_else(|| {
        Problem::after(
            "Plenipo is waiting for 8 West's weekly check before it connects your phones.",
            Duration::from_secs(300),
        )
    })?;
    let keys = remote
        .relay_identity()
        .map_err(|e| Problem::after(e.to_string(), Duration::from_secs(600)))?;
    let config = WebSocketConfig::default()
        .max_message_size(Some(wire::MAX_RELAY_MESSAGE))
        .max_frame_size(Some(wire::MAX_RELAY_MESSAGE));
    let connect =
        tokio_tungstenite::connect_async_with_config(socket_address(address), Some(config), false);
    let (mut ws, _) = tokio::time::timeout(STEP, connect)
        .await
        .map_err(|_| Problem::new("8 West's relay did not answer in time."))?
        .map_err(|e| Problem::new(format!("Plenipo couldn't reach 8 West's relay ({e}).")))?;

    // The relay's challenge.
    let first = next_text(&mut ws).await?;
    let Some(RelayToPc::Challenge { nonce }) = wire::read(&first) else {
        return Err(Problem::new(
            "8 West's relay said something Plenipo does not know.",
        ));
    };
    if b64::decode_exact::<32>(&nonce).is_none() {
        return Err(Problem::new(
            "8 West's relay said something Plenipo does not know.",
        ));
    }
    let hello = PcToRelay::Hello {
        v: 1,
        key: b64::encode(&keys.relay_public()),
        proof: b64::encode(&keys.relay_sign(PC_PROOF_CONTEXT, &nonce)),
        answer,
    };
    ws.send(Message::text(wire::write(&hello)))
        .await
        .map_err(|e| Problem::new(format!("The connection to 8 West's relay broke ({e}).")))?;
    match wire::read::<RelayToPc>(&next_text(&mut ws).await?) {
        Some(RelayToPc::Welcome { pc }) if pc == keys.fingerprint() => {}
        Some(RelayToPc::Refused { code }) => return Err(refused(&code)),
        _ => {
            return Err(Problem::new(
                "8 West's relay said something Plenipo does not know.",
            ))
        }
    }

    let (out, mut commands) = tokio::sync::mpsc::unbounded_channel::<ToRelay>();
    remote.relay_up(out);
    let mut tick = tokio::time::interval(TICK);
    let mut alive = tokio::time::interval(KEEP_ALIVE);
    loop {
        tokio::select! {
            message = ws.next() => match message {
                Some(Ok(Message::Text(text))) => {
                    if let Some(m) = wire::read::<RelayToPc>(text.as_str()) {
                        remote.from_relay(m);
                    }
                }
                Some(Ok(Message::Close(_))) | None => return Ok(()),
                Some(Ok(_)) => {}
                Some(Err(_)) => return Ok(()),
            },
            command = commands.recv() => {
                let Some(command) = command else { return Ok(()) };
                let text = wire::write(&command.message());
                if ws.send(Message::text(text)).await.is_err() {
                    return Ok(());
                }
            }
            _ = tick.tick() => remote.tick(),
            _ = alive.tick() => {
                if ws.send(Message::Ping(Vec::new().into())).await.is_err() {
                    return Ok(());
                }
            }
        }
    }
}

async fn next_text<S>(ws: &mut S) -> Result<String, Problem>
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let next = tokio::time::timeout(STEP, ws.next())
            .await
            .map_err(|_| Problem::new("8 West's relay did not answer in time."))?;
        match next {
            Some(Ok(Message::Text(text))) => return Ok(text.as_str().to_owned()),
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
            Some(Ok(Message::Close(_))) | None => {
                return Err(Problem::new("8 West's relay closed the connection."))
            }
            Some(Ok(_)) => {
                return Err(Problem::new(
                    "8 West's relay said something Plenipo does not know.",
                ))
            }
            Some(Err(e)) => {
                return Err(Problem::new(format!(
                    "The connection to 8 West's relay broke ({e})."
                )))
            }
        }
    }
}

/// The relay's refusal, in plain words.
fn refused(code: &str) -> Problem {
    match code {
        codes::NOT_PRO => Problem::after(
            "8 West's relay says this PC is not on Pro. Plenipo tries again after its next weekly \
             check.",
            Duration::from_secs(3600),
        ),
        codes::TOO_MANY_TRIES => Problem::after(
            "8 West's relay asked Plenipo to wait before trying again.",
            Duration::from_secs(300),
        ),
        _ => Problem::new(format!(
            "8 West's relay did not accept {} ({code}).",
            plenipo_core::WORDS.this_computer
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_waits_grow_and_start_again() {
        let mut w = Waits::default();
        let secs: Vec<u64> = (0..8).map(|_| w.wait().as_secs()).collect();
        assert_eq!(secs, [1, 2, 5, 10, 30, 60, 60, 60]);
        w.reset();
        assert_eq!(w.wait().as_secs(), 1);
    }

    #[test]
    fn https_becomes_a_secure_socket() {
        assert_eq!(
            socket_address("https://relay.getplenipo.com/plenipo/v1/pc"),
            "wss://relay.getplenipo.com/plenipo/v1/pc"
        );
        assert_eq!(
            socket_address("http://127.0.0.1:8769/plenipo/v1/pc"),
            "ws://127.0.0.1:8769/plenipo/v1/pc"
        );
    }

    #[test]
    fn every_refusal_has_plain_words() {
        for code in codes::ALL {
            let p = refused(code);
            assert!(p.message.contains("relay"), "{code}: {}", p.message);
        }
        assert_eq!(
            refused(codes::NOT_PRO).wait,
            Some(Duration::from_secs(3600))
        );
    }
}
