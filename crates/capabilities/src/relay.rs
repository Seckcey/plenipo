//! The tool relay: what an AI tool starts as Plenipo's MCP server (`plenipo-desktop
//! --plenipo-tools=<ticket file>`). It reads its ticket, connects to the running Plenipo on the
//! loopback address, introduces itself with the ticket, and then only passes lines through —
//! stdin to Plenipo, Plenipo to stdout. It never interprets a message; everything is decided
//! in Plenipo. Standard library only, so it starts fast and needs no async runtime.

use std::io::{BufRead as _, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The argument that selects relay mode.
pub const ARG: &str = "--plenipo-tools=";

/// What a ticket file holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ticket {
    pub port: u16,
    pub ticket: String,
}

/// The first line the relay sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub ticket: String,
}

/// Run as the relay when the first argument asks for it; returns the exit code.
pub fn maybe_run_from_args(mut args: impl Iterator<Item = String>) -> Option<i32> {
    let _program = args.next();
    let first = args.next()?;
    let path = first.strip_prefix(ARG)?;
    Some(run(Path::new(path)))
}

fn fail(message: &str) -> i32 {
    eprintln!("plenipo tools: {message}");
    1
}

/// Relay stdin/stdout for the ticket at `ticket_path`.
pub fn run(ticket_path: &Path) -> i32 {
    let ticket: Ticket = match std::fs::read_to_string(ticket_path)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
    {
        Ok(t) => t,
        Err(e) => {
            return fail(&format!(
                "the ticket {} could not be read ({e}); the task step may have ended",
                ticket_path.display()
            ))
        }
    };
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, ticket.port));
    let stream = match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
        Ok(s) => s,
        Err(e) => return fail(&format!("could not reach Plenipo ({e})")),
    };
    let _ = stream.set_nodelay(true);
    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(e) => return fail(&e.to_string()),
    };
    let hello = serde_json::to_string(&Hello {
        ticket: ticket.ticket,
    })
    .unwrap_or_default();
    if writeln!(writer, "{hello}")
        .and_then(|()| writer.flush())
        .is_err()
    {
        return fail("could not introduce itself to Plenipo");
    }
    // Plenipo → stdout.
    let reader = std::thread::spawn(move || {
        let mut from = BufReader::new(stream);
        let mut out = std::io::stdout().lock();
        let mut line = Vec::new();
        loop {
            line.clear();
            match from.read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if out.write_all(&line).and_then(|()| out.flush()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    // stdin → Plenipo.
    let mut input = BufReader::new(std::io::stdin().lock());
    let mut buf = [0u8; 16 * 1024];
    loop {
        match input.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if writer
                    .write_all(&buf[..n])
                    .and_then(|()| writer.flush())
                    .is_err()
                {
                    break;
                }
            }
        }
    }
    let _ = writer.shutdown(std::net::Shutdown::Write);
    let _ = reader.join();
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_relay_argument_selects_relay_mode() {
        let args = |v: &[&str]| {
            v.iter()
                .map(|s| (*s).to_owned())
                .collect::<Vec<_>>()
                .into_iter()
        };
        assert_eq!(maybe_run_from_args(args(&["plenipo"])), None);
        assert_eq!(maybe_run_from_args(args(&["plenipo", "--other"])), None);
        // A missing ticket fails (exit 1) instead of starting anything else.
        let missing = std::env::temp_dir().join("plenipo-no-such-ticket.json");
        let arg = format!("{ARG}{}", missing.display());
        assert_eq!(maybe_run_from_args(args(&["plenipo", &arg])), Some(1));
    }
}
