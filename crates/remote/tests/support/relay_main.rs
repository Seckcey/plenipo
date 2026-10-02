//! A stand-in for 8 West's relay on 127.0.0.1, for the Phase 14 end-to-end tests: the desktop app
//! built for the tests connects here, and so does the phone's page in the test browser. Never
//! shipped (ADR-143 §14).
//!
//! `plenipo-test-relay [port]` (8769 by default). It trusts the license contract's test key, as
//! copies built for the tests do. It prints what a test needs to know, one line each: `a PC
//! connected`, `no PC connected`, and `refused <code>`. Given `look <words>` on its input, it
//! answers `saw <words>` or `never saw <words>`: whether any sealed message it passed held them.

use std::io::{BufRead, Write};
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let port = std::env::args()
        .nth(1)
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8769);
    let relay = plenipo_remote::stand_in::Relay::start_on(
        &format!("127.0.0.1:{port}"),
        plenipo_licensing::clock,
    )
    .await;
    say(&format!("the stand-in relay listens on {}", relay.address));

    let looking = relay.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if let Some(words) = line.strip_prefix("look ") {
                let saw = looking
                    .seen()
                    .iter()
                    .any(|m| m.windows(words.len()).any(|w| w == words.as_bytes()));
                say(&format!(
                    "{} {words}",
                    if saw { "saw" } else { "never saw" }
                ));
            }
        }
    });

    let watching = relay.clone();
    std::thread::spawn(move || {
        let mut connected = false;
        let mut refused = 0;
        loop {
            std::thread::sleep(Duration::from_millis(100));
            let now = watching.pc_connected();
            if now != connected {
                say(if now {
                    "a PC connected"
                } else {
                    "no PC connected"
                });
                connected = now;
            }
            let codes = watching.refused();
            for code in codes.iter().skip(refused) {
                say(&format!("refused {code}"));
            }
            refused = codes.len();
        }
    });

    std::future::pending::<()>().await;
}

/// One line, at once (the test reads them as they come).
fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}
