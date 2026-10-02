//! A stand-in for 8 West's relay on 127.0.0.1, for the Phase 14 end-to-end tests: the desktop app
//! built for the tests connects here, and so does the phone's page in the test browser. Never
//! shipped (ADR-143 §14).
//!
//! `plenipo-test-relay [port]` (8769 by default). It trusts the license contract's test key, as
//! copies built for the tests do.

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let port = std::env::args()
        .nth(1)
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8769);
    let relay =
        plenipo_remote::stand_in::Relay::start_on(&format!("127.0.0.1:{port}"), plenipo_licensing::clock)
            .await;
    println!("the stand-in relay listens on {}", relay.address);
    std::future::pending::<()>().await;
}
