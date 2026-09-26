//! The tool relay as a test binary: what the desktop app runs as
//! `plenipo-desktop --plenipo-tools=<ticket>`. Never shipped.

fn main() {
    let code =
        plenipo_capabilities::relay::maybe_run_from_args(std::env::args()).unwrap_or_else(|| {
            eprintln!("usage: plenipo-tool-relay --plenipo-tools=<ticket file>");
            2
        });
    std::process::exit(code);
}
