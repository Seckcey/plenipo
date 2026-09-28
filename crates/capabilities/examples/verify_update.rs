//! The Release workflow's check (Phase 13, ADR-038): the installer it is about to publish
//! carries an updater signature Plenipo will accept, for exactly this version.
//!
//! `cargo run -p plenipo-capabilities --example verify_update -- <installer> <signature file>
//! <version>`, with the updater key's public half in `PLENIPO_UPDATER_PUBLIC_KEY`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [installer, signature, version] = args.as_slice() else {
        eprintln!("usage: verify_update <installer> <signature file> <version>");
        std::process::exit(2);
    };
    let key = std::env::var("PLENIPO_UPDATER_PUBLIC_KEY").unwrap_or_default();
    if key.trim().is_empty() {
        eprintln!("PLENIPO_UPDATER_PUBLIC_KEY is not set");
        std::process::exit(2);
    }
    let bytes = std::fs::read(installer).expect("read the installer");
    let signature = std::fs::read_to_string(signature).expect("read the signature");
    match plenipo_capabilities::updates::verify(&bytes, &signature, key.trim(), version) {
        Ok(()) => println!("{installer}: signed for {version} with the updater key"),
        Err(e) => {
            eprintln!("{installer}: {e}");
            std::process::exit(1);
        }
    }
}
