//! Diagnostic child process for runtime tests.
//!
//! `plenipo-diag --plenipo-diagnostic=<scenario>` runs a harmless scenario (see
//! `plenipo_runtime::diagnostic`). `plenipo-diag --host <work-dir>` runs a Supervisor that
//! owns one long-running child; tests kill the host to prove its children do not survive.

use std::sync::Arc;
use std::time::Duration;

use plenipo_runtime::diagnostic::{self, Scenario};
use plenipo_runtime::{
    EventSink, ExecutablePolicy, LaunchProfile, MetadataStore, ProfileRegistry, RuntimeEvent,
    Supervisor, SupervisorConfig,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--host") {
        std::process::exit(host(args.get(2).map(String::as_str)));
    }
    let code = diagnostic::maybe_run_from_args(args).unwrap_or_else(|| {
        eprintln!("usage: plenipo-diag --plenipo-diagnostic=<scenario> | --host <work-dir>");
        64
    });
    std::process::exit(code);
}

struct Silent;
impl EventSink for Silent {
    fn emit(&self, _event: RuntimeEvent) {}
}

fn host(work_dir: Option<&str>) -> i32 {
    let Some(work_dir) = work_dir else {
        eprintln!("--host requires a working directory");
        return 64;
    };
    let exe = std::env::current_exe().expect("current exe");
    let policy = ExecutablePolicy::new([&exe]).expect("policy");
    let profile = LaunchProfile {
        id: "host.long".into(),
        label: "Hosted long-running".into(),
        description: String::new(),
        executable: exe,
        args: vec![diagnostic::arg(Scenario::LongRunning)],
        env: vec![],
        working_dir: work_dir.into(),
        max_runtime: Duration::from_secs(600),
    };
    let (profiles, rejected) = ProfileRegistry::new([profile], &policy);
    assert!(rejected.is_empty(), "{rejected:?}");
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let supervisor = Supervisor::new(
            SupervisorConfig::default(),
            policy,
            profiles,
            MetadataStore::in_memory(),
            Arc::new(Silent),
            vec![],
        );
        let record = supervisor.start("host.long").await.expect("start");
        println!("hosted-pid:{}", record.pid.unwrap_or(0));
        // Wait to be killed.
        tokio::time::sleep(Duration::from_secs(600)).await;
    });
    0
}
