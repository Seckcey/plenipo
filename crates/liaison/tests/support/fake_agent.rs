//! The runtime's fake agent CLIs (`plenipo-fake-agent`), built as a binary of this crate too so
//! Liaison's integration tests can run them. Never shipped.

#[path = "../../../runtime/src/bin/plenipo-fake-agent.rs"]
mod fake;

fn main() {
    fake::main();
}
