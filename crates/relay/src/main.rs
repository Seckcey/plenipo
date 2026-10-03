//! `plenipo-relay`: Plenipo's own relay for Plenipo on your phone (Phase 14, ADR-149), as a
//! program. Made by 8 West Ventures, LLC.
//!
//! It reads its settings from the environment (a systemd unit's `EnvironmentFile`), listens on
//! `127.0.0.1` only, logs to standard error (the journal), honours an **off switch** (a file the
//! operator makes), and stops cleanly on SIGTERM. `plenipo-relay --help` lists the settings.
//!
//! A copy built with the `test-hooks` feature (never a release) also trusts the license
//! contract's test key and prints what Plenipo's real-app tests need to know, one line each: `a PC
//! connected`, `no PC connected`, and `refused <code>`. Given `look <words>` on its input, it
//! answers `saw <words>` or `never saw <words>`: whether any sealed message it passed held them.

use std::path::PathBuf;
use std::time::Duration;

use plenipo_relay::{ClientAddress, Config, Limits, Relay};

const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "\
plenipo-relay: Plenipo's own relay for Plenipo on your phone. Made by 8 West Ventures, LLC.

Usage: plenipo-relay [--version] [--help] [PORT]

It listens on 127.0.0.1 (or a private address of this machine, for a proxy that runs in a
container); a proxy in front of it ends TLS. Settings come from the environment:

  PLENIPO_RELAY_LISTEN                   where to listen (default 127.0.0.1:8790; PORT replaces the port);
                                         never an address the internet can reach
  PLENIPO_RELAY_CLIENT_ADDRESS           where a connection's internet address is read, for the limits:
                                         proxy (default: the proxy's X-Forwarded-For), cloudflare
                                         (CF-Connecting-IP), or peer (the connection itself)
  PLENIPO_RELAY_OFF_FILE                 the off switch: while this file exists, every connection is
                                         closed and new ones get 503 (default /etc/plenipo-relay/off)
  PLENIPO_RELAY_LOG                      error, warn, info (default), or debug
  PLENIPO_RELAY_MAX_CONNECTIONS          open connections in all (default 512)
  PLENIPO_RELAY_MAX_PER_ADDRESS          open connections from one address (default 32)
  PLENIPO_RELAY_MAX_NEW_PER_MINUTE       new connections from one address in a minute (default 120)
  PLENIPO_RELAY_MAX_TRIES_PER_MINUTE     refusals for one address in a minute (default 30)
  PLENIPO_RELAY_MAX_PHONES_PER_PC        phone connections one PC may have at once (default 40)
  PLENIPO_RELAY_MAX_MESSAGES_PER_MINUTE  messages one connection may send in a minute (default 1200)
  PLENIPO_RELAY_MAX_BYTES_PER_MINUTE     bytes one connection may send in a minute (default 16777216)
  PLENIPO_RELAY_IDLE_SECONDS             a connection quiet this long is closed (default 90)

GET /healthz answers `ok` (or `off`). Logs hold counts, codes, and addresses only: never a message,
a key, a pass, a weekly answer, a mailbox name, or a challenge.
";

struct Settings {
    config: Config,
    off_file: PathBuf,
    log: log::LevelFilter,
}

fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

fn env_number<T: std::str::FromStr>(name: &str, default: T) -> Result<T, String> {
    match env(name) {
        None => Ok(default),
        Some(v) => v
            .parse::<T>()
            .map_err(|_| format!("{name} must be a whole number, not \"{v}\"")),
    }
}

impl Settings {
    fn from_env(port: Option<u16>) -> Result<Self, String> {
        let mut config = Config::default();
        if let Some(listen) = env("PLENIPO_RELAY_LISTEN") {
            config.listen = listen.parse().map_err(|_| {
                format!("PLENIPO_RELAY_LISTEN must be an address and port, not \"{listen}\"")
            })?;
        }
        if let Some(port) = port {
            config.listen.set_port(port);
        }
        config.client_address = match env("PLENIPO_RELAY_CLIENT_ADDRESS").as_deref() {
            None => {
                if cfg!(feature = "test-hooks") {
                    ClientAddress::Peer
                } else {
                    ClientAddress::Proxy
                }
            }
            Some("peer") => ClientAddress::Peer,
            Some("proxy") => ClientAddress::Proxy,
            Some("cloudflare") => ClientAddress::Cloudflare,
            Some(other) => {
                return Err(format!(
                "PLENIPO_RELAY_CLIENT_ADDRESS must be proxy, cloudflare, or peer, not \"{other}\""
            ))
            }
        };
        let defaults = Limits::default();
        config.limits = Limits {
            connections: env_number("PLENIPO_RELAY_MAX_CONNECTIONS", defaults.connections)?,
            connections_per_address: env_number(
                "PLENIPO_RELAY_MAX_PER_ADDRESS",
                defaults.connections_per_address,
            )?,
            new_per_address_per_minute: env_number(
                "PLENIPO_RELAY_MAX_NEW_PER_MINUTE",
                defaults.new_per_address_per_minute,
            )?,
            tries_per_address_per_minute: env_number(
                "PLENIPO_RELAY_MAX_TRIES_PER_MINUTE",
                defaults.tries_per_address_per_minute,
            )?,
            phones_per_pc: env_number("PLENIPO_RELAY_MAX_PHONES_PER_PC", defaults.phones_per_pc)?,
            messages_per_minute: env_number(
                "PLENIPO_RELAY_MAX_MESSAGES_PER_MINUTE",
                defaults.messages_per_minute,
            )?,
            bytes_per_minute: env_number(
                "PLENIPO_RELAY_MAX_BYTES_PER_MINUTE",
                defaults.bytes_per_minute,
            )?,
            idle: Duration::from_secs(env_number(
                "PLENIPO_RELAY_IDLE_SECONDS",
                defaults.idle.as_secs(),
            )?),
            ..defaults
        };
        let off_file = env("PLENIPO_RELAY_OFF_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/etc/plenipo-relay/off"));
        let log = match env("PLENIPO_RELAY_LOG").as_deref() {
            None | Some("info") => log::LevelFilter::Info,
            Some("error") => log::LevelFilter::Error,
            Some("warn") => log::LevelFilter::Warn,
            Some("debug") => log::LevelFilter::Debug,
            Some(other) => {
                return Err(format!(
                    "PLENIPO_RELAY_LOG must be error, warn, info, or debug, not \"{other}\""
                ))
            }
        };
        Ok(Self {
            config,
            off_file,
            log,
        })
    }
}

/// Lines on standard error, which the journal keeps with its own time stamps.
struct Logger(log::LevelFilter);

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.0
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            eprintln!("{} {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let mut port = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("plenipo-relay {VERSION}");
                return;
            }
            "--help" | "-h" => {
                print!("{HELP}");
                return;
            }
            other => match other.parse::<u16>() {
                Ok(p) => port = Some(p),
                Err(_) => {
                    eprintln!("plenipo-relay: unknown option \"{other}\" (see --help)");
                    std::process::exit(2);
                }
            },
        }
    }
    let settings = match Settings::from_env(port) {
        Ok(s) => s,
        Err(why) => {
            eprintln!("plenipo-relay: {why}");
            std::process::exit(2);
        }
    };
    let _ = log::set_boxed_logger(Box::new(Logger(settings.log)));
    log::set_max_level(settings.log);
    std::process::exit(run(settings).await);
}

async fn run(settings: Settings) -> i32 {
    let listen = settings.config.listen;
    let handle = match Relay::start(settings.config).await {
        Ok(h) => h,
        Err(e) => {
            log::error!("could not listen on {listen}: {e}");
            return 1;
        }
    };
    log::info!(
        "Plenipo's relay {VERSION} listens on {} (made by 8 West Ventures, LLC)",
        handle.address()
    );
    #[cfg(feature = "test-hooks")]
    test_hooks::start(handle.clone());

    // The off switch: a file the operator makes or removes. Looked at once a second.
    let switch = handle.clone();
    let off_file = settings.off_file;
    tokio::spawn(async move {
        let mut off = false;
        loop {
            let now_off = off_file.exists();
            if now_off != off {
                off = now_off;
                switch.set_off(off);
                if off {
                    log::warn!(
                        "off: {} exists; every connection is closed and new ones are turned away",
                        off_file.display()
                    );
                } else {
                    log::warn!("on again: {} is gone", off_file.display());
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    // Counts, now and then.
    let counting = handle.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(600));
        tick.tick().await;
        loop {
            tick.tick().await;
            let s = counting.stats();
            let refused: u64 = s.refused.values().sum();
            log::info!(
                "{} connections: {} PCs, {} phones, {} addresses; {} refused and {} turned \
                 away so far",
                s.connections,
                s.pcs,
                s.phones,
                s.addresses,
                refused,
                s.turned_away
            );
        }
    });

    wait_for_stop().await;
    log::info!("stopping: closing every connection");
    handle.shutdown(Duration::from_secs(5)).await;
    log::info!("stopped");
    0
}

#[cfg(unix)]
async fn wait_for_stop() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = signal(SignalKind::terminate()).expect("SIGTERM");
    let mut int = signal(SignalKind::interrupt()).expect("SIGINT");
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_stop() {
    let _ = tokio::signal::ctrl_c().await;
}

/// What Plenipo's real-app tests read (`tests/e2e/specs/remote.e2e.mjs`), the same lines the
/// stand-in relay prints. Only in a copy built for the tests.
#[cfg(feature = "test-hooks")]
mod test_hooks {
    use std::io::{BufRead as _, Write as _};
    use std::time::Duration;

    use plenipo_relay::Handle;

    /// One line, at once (the test reads them as they come).
    fn say(line: &str) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }

    pub fn start(handle: Handle) {
        say(&format!("the relay listens on {}", handle.address()));
        let looking = handle.clone();
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
        std::thread::spawn(move || {
            let mut connected = false;
            let mut refused = 0;
            loop {
                std::thread::sleep(Duration::from_millis(100));
                let now = handle.pc_connected();
                if now != connected {
                    say(if now {
                        "a PC connected"
                    } else {
                        "no PC connected"
                    });
                    connected = now;
                }
                let codes = handle.refused();
                for code in codes.iter().skip(refused) {
                    say(&format!("refused {code}"));
                }
                refused = codes.len();
            }
        });
    }
}
