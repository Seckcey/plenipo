//! Test helper: the synthetic SSH server (`support/sshd.rs`) as a program, for the end-to-end
//! test. Never shipped.
//!
//! `plenipo-test-sshd [--port N] [--seed N] [--user NAME] [--password TEXT] [--file PATH=TEXT]
//! [--authorized PUBLIC_KEY_FILE] [--shell on]` (`--shell on`: give the owner's terminal a small
//! shell; workers never ask for one)
//! prints `{"port":…,"fingerprint":"SHA256:…","algorithm":"ssh-ed25519"}` on one line, then serves
//! on 127.0.0.1 until it is stopped.

#[path = "sshd.rs"]
mod sshd;

#[tokio::main]
async fn main() {
    let mut options = sshd::Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().unwrap_or_default();
        match flag.as_str() {
            "--port" => options.port = value.parse().expect("--port N"),
            "--seed" => options.seed = value.parse().expect("--seed N"),
            "--user" => options.user = value,
            "--password" => options.password = value,
            "--authorized" => {
                let text = std::fs::read_to_string(&value).expect("--authorized PUBLIC_KEY_FILE");
                options.authorized.push(
                    russh::keys::PublicKey::from_openssh(text.trim())
                        .expect("an OpenSSH public key"),
                );
            }
            "--shell" => options.shell = value == "on",
            "--file" => {
                let (path, text) = value.split_once('=').expect("--file PATH=TEXT");
                options.files.insert(path.into(), format!("{text}\n"));
            }
            other => panic!("unknown option {other}"),
        }
    }
    let server = sshd::Sshd::start(options).await;
    println!(
        "{}",
        serde_json::json!({
            "port": server.port,
            "fingerprint": server.fingerprint,
            "algorithm": server.algorithm,
        })
    );
    std::future::pending::<()>().await;
}
