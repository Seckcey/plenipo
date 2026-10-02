//! Check Plenipo's relay from outside, with Plenipo's own PC code: the relay must answer at its
//! name, through the proxy and Cloudflare, with WebSockets on, and must check Pro.
//!
//! ```text
//! cargo run -p plenipo-relay --example probe -- https://relay.getplenipo.com
//! ```
//!
//! It connects as a PC whose weekly answer is signed with the contract's **test** key, which the
//! real relay does not trust, so a healthy relay answers **not_pro**: that proves the path works and
//! the lock on Pro is on. Then it connects as a phone to a mailbox nobody opened, and a healthy
//! relay answers **mailbox_closed**. Nothing here is a secret, and nothing is paired.

use std::sync::Arc;

use futures_util::{SinkExt as _, StreamExt as _};
use plenipo_licensing::answer::{self, AnswerPayload};
use plenipo_licensing::{SignedAnswer, SubscriptionState};
use plenipo_remote::devices::{MemoryConfig, MemoryStore};
use plenipo_remote::link::{self, LinkHost};
use plenipo_remote::{Remote, Settings};
use tokio_tungstenite::tungstenite::Message;

struct Probe;

impl plenipo_remote::Host for Probe {
    fn pro(&self) -> bool {
        true
    }
    fn approval(&self, _: &str, _: &str) -> Option<plenipo_guard::remote::ApprovalFacts> {
        None
    }
    fn carry_out(
        &self,
        _: &plenipo_remote::Phone,
        _: &plenipo_remote::protocol::Ask,
    ) -> Result<serde_json::Value, String> {
        Err("a probe carries nothing out".into())
    }
    fn record(&self, _: Option<&str>, _: &str, _: serde_json::Value) {}
    fn changed(&self, _: plenipo_remote::Change) {}
}

impl LinkHost for Probe {
    fn check_address(&self, address: &str) -> Result<(), String> {
        if address.starts_with("https://") || address.starts_with("http://") {
            Ok(())
        } else {
            Err("the probe reaches https:// addresses only".into())
        }
    }

    fn weekly_answer(&self) -> Option<SignedAnswer> {
        let now = plenipo_licensing::clock();
        Some(answer::sign(
            &AnswerPayload {
                v: 1,
                key_id: "lk_01J9XW3T5B8K2M4N6P7Q8R9S0T".into(),
                state: SubscriptionState::Active,
                paid_through: Some(now + 86_400),
                ends_at: None,
                as_of: now,
                signer: plenipo_licensing::trust::TEST_KEY_ID.into(),
            },
            &plenipo_licensing::trust::test_signing_key(),
        ))
    }
}

#[tokio::main]
async fn main() {
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://relay.getplenipo.com".into());
    let base = base.trim_end_matches('/').to_owned();
    let mut good = true;

    // As a PC.
    let remote = Remote::new(
        Settings {
            origin: plenipo_remote::PAGE_ORIGIN.into(),
            rp_id: plenipo_remote::RP_ID.into(),
            pc_name: "Probe".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        Arc::new(MemoryStore::default()),
        Arc::new(MemoryConfig::default()),
        Arc::new(Probe),
        Arc::new(plenipo_remote::SystemClock),
    );
    let _ = remote.set_switched_on(true);
    let pc = format!("{base}/plenipo/v1/pc");
    println!("1. As a PC, at {pc}");
    match link::connect_once(&remote, &pc, &Probe).await {
        Err(p) if p.message.contains("not on Pro") => {
            println!("   good: the relay answered, checked the proof, and refused the test key's answer (not_pro)")
        }
        Err(p) => {
            good = false;
            println!("   PROBLEM: {}", p.message)
        }
        Ok(()) => {
            good = false;
            println!("   PROBLEM: the relay welcomed a PC whose weekly answer is signed with the TEST key: it trusts the test key, or does not check Pro")
        }
    }

    // As a phone.
    let phone = format!("{}/plenipo/v1/phone", link::socket_address(&base));
    println!("2. As a phone, at {phone}");
    match tokio_tungstenite::connect_async(&phone).await {
        Ok((mut ws, _)) => {
            let first = r#"{"t":"mailbox","mailbox":"00000000000000000000000000000000"}"#;
            let _ = ws.send(Message::text(first)).await;
            let answer = tokio::time::timeout(std::time::Duration::from_secs(20), ws.next()).await;
            match answer {
                Ok(Some(Ok(Message::Text(text)))) if text.contains("mailbox_closed") => {
                    println!(
                        "   good: the relay answered mailbox_closed for a mailbox nobody opened"
                    )
                }
                other => {
                    good = false;
                    println!("   PROBLEM: unexpected answer: {other:?}")
                }
            }
        }
        Err(e) => {
            good = false;
            println!("   PROBLEM: could not connect: {e}")
        }
    }

    // Health.
    let health = format!("{base}/healthz");
    println!("3. {health}");
    println!("   (open it in a browser: it should say ok)");

    if good {
        println!("Plenipo's relay answers at {base}. Made by 8 West Ventures, LLC.");
    } else {
        println!("Something is wrong; see above.");
        std::process::exit(1);
    }
}
