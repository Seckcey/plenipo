//! Test helper: the stand-ins for the connections' services as a program, for the Phase 20
//! end-to-end tests. Never shipped: Microsoft's sign-in and Microsoft Graph
//! (`support/microsoft.rs`, part 20A), and Slack's and Google's sign-ins and APIs
//! (`support/slack.rs`, `support/google.rs`, part 20B), each at
//! `http://127.0.0.1:<port>/<real host><path>`.
//!
//! `plenipo-test-services [--port N]` prints `{"port":…}` on one line, then serves on 127.0.0.1
//! until it is stopped. Tests read and change it over HTTP: `GET /_control/world`,
//! `POST /_control/knobs`.

#[path = "google.rs"]
mod google;
#[path = "microsoft.rs"]
mod microsoft;
#[path = "slack.rs"]
mod slack;

#[tokio::main]
async fn main() {
    let mut port = 0;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--port" => port = args.next().and_then(|v| v.parse().ok()).expect("--port N"),
            other => panic!("unknown option {other}"),
        }
    }
    let server = microsoft::StandIn::start_on(port).await;
    println!("{}", serde_json::json!({ "port": server.port }));
    std::future::pending::<()>().await;
}
