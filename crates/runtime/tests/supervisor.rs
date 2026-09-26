//! End-to-end supervisor tests against the real `plenipo-diag` child process.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use plenipo_runtime::diagnostic::{self, Scenario};
use plenipo_runtime::{
    EventSink, ExecutablePolicy, ExecutionRecord, ExecutionState, LaunchProfile, MetadataStore,
    OutputStream, ProfileRegistry, RuntimeError, RuntimeEvent, Supervisor, SupervisorConfig,
};

const WAIT: Duration = Duration::from_secs(20);

fn diag_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_plenipo-diag"))
}

#[derive(Default)]
struct Collector {
    events: Mutex<Vec<RuntimeEvent>>,
}

impl EventSink for Collector {
    fn emit(&self, event: RuntimeEvent) {
        self.events.lock().unwrap().push(event);
    }
}

impl Collector {
    fn events(&self) -> Vec<RuntimeEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Output lines for `id` in arrival order, with the batch index each came in.
    fn lines(&self, id: &str) -> Vec<(usize, OutputStream, String, u64)> {
        let mut out = vec![];
        for (batch, event) in self
            .events()
            .into_iter()
            .filter_map(|e| match e {
                RuntimeEvent::Output(b) if b.execution_id == id => Some(b),
                _ => None,
            })
            .enumerate()
        {
            for l in event.lines {
                out.push((batch, l.stream, l.text, l.seq));
            }
        }
        out
    }

    fn lifecycle_states(&self, id: &str) -> Vec<ExecutionState> {
        self.events()
            .into_iter()
            .filter_map(|e| match e {
                RuntimeEvent::Lifecycle(l) if l.record.id == id => Some(l.record.state),
                _ => None,
            })
            .collect()
    }
}

fn profile(id: &str, scenario: Scenario, dir: &Path, secs: u64) -> LaunchProfile {
    LaunchProfile {
        id: id.into(),
        label: id.into(),
        description: String::new(),
        executable: diag_exe(),
        args: vec![diagnostic::arg(scenario)],
        env: vec![],
        working_dir: dir.to_path_buf(),
        max_runtime: Duration::from_secs(secs),
    }
}

struct Harness {
    sup: Supervisor,
    sink: Arc<Collector>,
    dir: tempfile::TempDir,
}

fn harness_with(
    extra: impl FnOnce(&Path) -> Vec<LaunchProfile>,
    store: Option<PathBuf>,
) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    let mut echo = profile("echo", Scenario::Echo, d, 60);
    echo.env = vec![(diagnostic::GREETING_VAR.into(), "hi there".into())];
    let mut env = profile("env", Scenario::Env, d, 60);
    env.env = vec![("PLENIPO_TEST_DECLARED".into(), "1".into())];
    let mut profiles = vec![
        echo,
        env,
        profile("failure", Scenario::Failure, d, 60),
        profile("long", Scenario::LongRunning, d, 600),
        profile("short-timeout", Scenario::LongRunning, d, 1),
        profile("tree", Scenario::Tree, d, 600),
        profile("burst", Scenario::Burst, d, 60),
    ];
    profiles.extend(extra(d));
    let policy = ExecutablePolicy::new([diag_exe()]).unwrap();
    let (registry, rejected) = ProfileRegistry::new(profiles, &policy);
    assert!(rejected.is_empty(), "{rejected:?}");
    let sink = Arc::new(Collector::default());
    let store: Arc<dyn plenipo_runtime::ExecutionStore> =
        Arc::new(store.map_or_else(MetadataStore::in_memory, MetadataStore::file));
    let sup = Supervisor::new(
        SupervisorConfig::default(),
        policy,
        registry,
        store,
        sink.clone(),
        vec![],
    );
    Harness { sup, sink, dir }
}

fn harness() -> Harness {
    harness_with(|_| vec![], None)
}

async fn wait_terminal(sup: &Supervisor, id: &str) -> ExecutionRecord {
    let deadline = Instant::now() + WAIT;
    loop {
        let record = sup.record(id).expect("known execution");
        if record.state.is_terminal() {
            return record;
        }
        assert!(
            Instant::now() < deadline,
            "execution {id} did not finish: {record:?}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_for_line(sink: &Collector, id: &str, needle: &str) -> String {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some((_, _, text, _)) = sink.lines(id).into_iter().find(|l| l.2.contains(needle)) {
            return text;
        }
        assert!(Instant::now() < deadline, "no output containing {needle:?}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

fn pid_alive(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessStatus, ProcessesToUpdate, System};
    let mut sys = System::new();
    let pid = Pid::from_u32(pid);
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid)
        .is_some_and(|p| !matches!(p.status(), ProcessStatus::Zombie | ProcessStatus::Dead))
}

async fn wait_pid_gone(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while pid_alive(pid) {
        assert!(
            Instant::now() < deadline,
            "process {pid} is still alive (orphaned)"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn launches_process_and_streams_stdout_and_stderr_incrementally() {
    let h = harness();
    let started = h.sup.start("echo").await.unwrap();
    assert_eq!(started.state, ExecutionState::Running);
    assert!(started.pid.is_some());

    let done = wait_terminal(&h.sup, &started.id).await;
    assert_eq!(done.state, ExecutionState::Succeeded);
    assert_eq!(done.exit_code, Some(0));
    assert!(done.ended_at.is_some());

    let lines = h.sink.lines(&started.id);
    let stdout: Vec<_> = lines
        .iter()
        .filter(|l| l.1 == OutputStream::Stdout)
        .collect();
    let stderr: Vec<_> = lines
        .iter()
        .filter(|l| l.1 == OutputStream::Stderr)
        .collect();
    assert!(
        stdout.iter().any(|l| l.2 == "greeting: hi there"),
        "declared env injected"
    );
    assert!(stdout.iter().any(|l| l.2 == "echo complete"));
    assert_eq!(stderr.len(), 3, "{stderr:?}");
    assert!(stderr[0].2.starts_with("stderr notice"));

    // Incremental: the ~2s run arrives in several batches, not one blob at exit.
    let batches = lines.iter().map(|l| l.0).max().unwrap() + 1;
    assert!(batches >= 3, "expected incremental batches, got {batches}");

    // seq is strictly increasing across both streams.
    let seqs: Vec<u64> = lines.iter().map(|l| l.3).collect();
    assert!(seqs.windows(2).all(|w| w[0] < w[1]), "{seqs:?}");

    // Lifecycle: starting -> running -> succeeded, and the final event follows all output.
    assert_eq!(
        h.sink.lifecycle_states(&started.id),
        [
            ExecutionState::Starting,
            ExecutionState::Running,
            ExecutionState::Succeeded
        ]
    );
    let last = h.sink.events().into_iter().rev().find(|e| match e {
        RuntimeEvent::Output(b) => b.execution_id == started.id,
        RuntimeEvent::Lifecycle(l) => l.record.id == started.id,
    });
    assert!(matches!(last, Some(RuntimeEvent::Lifecycle(_))));

    // Output remains available after completion.
    let output = h.sup.output(&started.id).unwrap();
    assert!(output.available);
    assert_eq!(output.lines.len(), lines.len());
}

#[tokio::test]
async fn detects_abnormal_exit() {
    let h = harness();
    let id = h.sup.start("failure").await.unwrap().id;
    let done = wait_terminal(&h.sup, &id).await;
    assert_eq!(done.state, ExecutionState::Failed);
    assert_eq!(done.exit_code, Some(3));
    assert_eq!(done.detail.as_deref(), Some("Exited with code 3"));
    wait_for_line(&h.sink, &id, "error: simulated failure").await;
}

#[tokio::test]
async fn cancels_long_running_process() {
    let h = harness();
    let rec = h.sup.start("long").await.unwrap();
    wait_for_line(&h.sink, &rec.id, "heartbeat 2").await;

    let t = Instant::now();
    let done = h.sup.cancel(&rec.id).await.unwrap();
    assert!(
        t.elapsed() < Duration::from_secs(8),
        "cancel took {:?}",
        t.elapsed()
    );
    assert_eq!(done.state, ExecutionState::Cancelled);
    assert_eq!(done.detail.as_deref(), Some("Cancelled by user"));
    assert_eq!(done.exit_code, None);
    wait_pid_gone(rec.pid.unwrap()).await;

    // Duplicate cancel is idempotent.
    let again = h.sup.cancel(&rec.id).await.unwrap();
    assert_eq!(again, done);
    assert_eq!(h.sup.active_count(), 0);
}

#[tokio::test]
async fn cancel_terminates_the_whole_process_tree() {
    let h = harness();
    let rec = h.sup.start("tree").await.unwrap();
    let line = wait_for_line(&h.sink, &rec.id, "child-pid:").await;
    let grandchild: u32 = line.trim_start_matches("child-pid:").parse().unwrap();
    assert!(pid_alive(grandchild));

    let done = h.sup.cancel(&rec.id).await.unwrap();
    assert_eq!(done.state, ExecutionState::Cancelled);
    wait_pid_gone(rec.pid.unwrap()).await;
    wait_pid_gone(grandchild).await;
}

#[tokio::test]
async fn enforces_maximum_runtime() {
    let h = harness();
    let rec = h.sup.start("short-timeout").await.unwrap();
    let done = wait_terminal(&h.sup, &rec.id).await;
    assert_eq!(done.state, ExecutionState::TimedOut);
    assert!(done.detail.unwrap().contains("maximum runtime of 1s"));
    wait_pid_gone(rec.pid.unwrap()).await;
}

#[tokio::test]
async fn child_environment_is_isolated() {
    let h = harness();
    let id = h.sup.start("env").await.unwrap().id;
    assert_eq!(
        wait_terminal(&h.sup, &id).await.state,
        ExecutionState::Succeeded
    );
    let names: Vec<String> = h
        .sink
        .lines(&id)
        .into_iter()
        .filter_map(|l| l.2.strip_prefix("env:").map(str::to_owned))
        .collect();
    assert!(names.iter().any(|n| n == "PLENIPO_TEST_DECLARED"));
    // Cargo sets CARGO_* in the test process; none may reach the child.
    assert!(std::env::var_os("CARGO_PKG_NAME").is_some());
    assert!(!names.iter().any(|n| n.starts_with("CARGO")), "{names:?}");
    for name in &names {
        let allowed = name == "PLENIPO_TEST_DECLARED"
            || plenipo_runtime::policy::BASELINE_ENV
                .iter()
                .any(|b| b.eq_ignore_ascii_case(name))
            // Windows adds a few per-process variables (e.g. "=C:") to every child.
            || (cfg!(windows) && name.starts_with('='));
        assert!(allowed, "unexpected variable leaked to child: {name}");
    }
}

#[tokio::test]
async fn output_is_bounded_and_long_lines_truncated() {
    let h = harness();
    let id = h.sup.start("burst").await.unwrap().id;
    assert_eq!(
        wait_terminal(&h.sup, &id).await.state,
        ExecutionState::Succeeded
    );
    let lines = h.sink.lines(&id);
    assert_eq!(lines.len(), 5002, "every line is streamed");
    let batches = lines.iter().map(|l| l.0).max().unwrap() + 1;
    assert!(batches >= 5002 / 200, "batched by size: {batches}");
    let long = &lines[5000];
    assert_eq!(long.2.len(), SupervisorConfig::default().max_line_bytes);

    let output = h.sup.output(&id).unwrap();
    assert_eq!(
        output.lines.len(),
        SupervisorConfig::default().output_buffer_lines
    );
    assert_eq!(output.dropped, 5002 - 1000);
    assert!(output.lines.iter().any(|l| l.truncated));
    assert_eq!(output.lines.last().unwrap().text, "burst done");
}

#[tokio::test]
async fn execution_ids_are_unique() {
    let h = harness();
    let mut ids = std::collections::HashSet::new();
    for _ in 0..5 {
        let rec = h.sup.start("failure").await.unwrap();
        assert!(ids.insert(rec.id.clone()));
        wait_terminal(&h.sup, &rec.id).await;
    }
    assert_eq!(h.sup.overview().executions.len(), 5);
}

#[tokio::test]
async fn rejects_unknown_profiles_and_executions() {
    let h = harness();
    assert!(matches!(
        h.sup.start("../../bin/sh").await,
        Err(RuntimeError::UnknownProfile(_))
    ));
    assert!(matches!(
        h.sup.cancel("nope").await,
        Err(RuntimeError::UnknownExecution(_))
    ));
    assert!(matches!(
        h.sup.output("nope"),
        Err(RuntimeError::UnknownExecution(_))
    ));
}

#[tokio::test]
async fn profiles_outside_executable_rules_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let rogue = dir.path().join("rogue");
    std::fs::copy(diag_exe(), &rogue).unwrap();
    let policy = ExecutablePolicy::new([diag_exe()]).unwrap();
    let mut bad = profile("rogue", Scenario::Echo, dir.path(), 60);
    bad.executable = rogue.clone();
    let mut relative = profile("relative", Scenario::Echo, dir.path(), 60);
    relative.executable = PathBuf::from("plenipo-diag");
    let (registry, rejected) = ProfileRegistry::new([bad, relative], &policy);
    assert_eq!(registry.infos().len(), 0);
    assert_eq!(rejected.len(), 2, "{rejected:?}");
    assert!(policy.check(&rogue).is_err());
}

#[tokio::test]
async fn executable_is_rechecked_at_spawn_time() {
    // Allowlist a copy, register the profile, then replace the file with a directory:
    // the spawn-time check must refuse it.
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("diag-copy");
    std::fs::copy(diag_exe(), &copy).unwrap();
    let policy = ExecutablePolicy::new([&copy]).unwrap();
    let mut p = profile("copy", Scenario::Echo, dir.path(), 60);
    p.executable = copy.clone();
    let (registry, rejected) = ProfileRegistry::new([p], &policy);
    assert!(rejected.is_empty());
    let sup = Supervisor::new(
        SupervisorConfig::default(),
        policy,
        registry,
        Arc::new(MetadataStore::in_memory()),
        Arc::new(Collector::default()),
        vec![],
    );
    std::fs::remove_file(&copy).unwrap();
    assert!(matches!(
        sup.start("copy").await,
        Err(RuntimeError::Policy(_))
    ));
    assert!(
        sup.overview().executions.is_empty(),
        "nothing recorded for a refused launch"
    );
}

#[tokio::test]
async fn invalid_working_directory_is_rejected() {
    let h = harness_with(
        |d| {
            vec![profile(
                "gone",
                Scenario::Echo,
                &d.join("does-not-exist"),
                60,
            )]
        },
        None,
    );
    assert!(matches!(
        h.sup.start("gone").await,
        Err(RuntimeError::Policy(_))
    ));
}

#[tokio::test]
async fn spawn_failure_becomes_failed_execution() {
    // An allowlisted file that is not a valid program.
    let dir = tempfile::tempdir().unwrap();
    let not_a_program = dir.path().join("not-a-program.exe");
    std::fs::write(&not_a_program, b"just text").unwrap();
    let policy = ExecutablePolicy::new([&not_a_program]).unwrap();
    let mut p = profile("broken", Scenario::Echo, dir.path(), 60);
    p.executable = not_a_program;
    let (registry, _) = ProfileRegistry::new([p], &policy);
    let sink = Arc::new(Collector::default());
    let sup = Supervisor::new(
        SupervisorConfig::default(),
        policy,
        registry,
        Arc::new(MetadataStore::in_memory()),
        sink.clone(),
        vec![],
    );
    let rec = sup.start("broken").await.unwrap();
    assert_eq!(rec.state, ExecutionState::Failed);
    assert!(rec.detail.unwrap().starts_with("Failed to start"));
    assert_eq!(
        sink.lifecycle_states(&rec.id),
        [ExecutionState::Starting, ExecutionState::Failed]
    );
    assert_eq!(sup.active_count(), 0);
}

#[tokio::test]
async fn shutdown_terminates_everything_and_refuses_new_work() {
    let h = harness();
    let a = h.sup.start("long").await.unwrap();
    let b = h.sup.start("tree").await.unwrap();
    let line = wait_for_line(&h.sink, &b.id, "child-pid:").await;
    let grandchild: u32 = line.trim_start_matches("child-pid:").parse().unwrap();

    assert_eq!(h.sup.shutdown(Duration::from_secs(15)).await, 2);
    for rec in [&a, &b] {
        let done = h.sup.record(&rec.id).unwrap();
        assert_eq!(done.state, ExecutionState::Cancelled);
        assert_eq!(
            done.detail.as_deref(),
            Some("Cancelled because Plenipo was shutting down")
        );
        wait_pid_gone(rec.pid.unwrap()).await;
    }
    wait_pid_gone(grandchild).await;
    assert!(matches!(
        h.sup.start("echo").await,
        Err(RuntimeError::ShuttingDown)
    ));
}

#[tokio::test]
async fn metadata_persists_and_restart_recovers_interrupted_runs() {
    let store_dir = tempfile::tempdir().unwrap();
    let path = store_dir.path().join("executions.json");

    let first = harness_with(|_| vec![], Some(path.clone()));
    let ok = first.sup.start("failure").await.unwrap();
    wait_terminal(&first.sup, &ok.id).await;
    let running = first.sup.start("long").await.unwrap();
    wait_for_line(&first.sink, &running.id, "heartbeat 1").await;

    // Simulate Plenipo dying without a graceful shutdown: the file still says "running".
    let on_disk = MetadataStore::file(&path).load().records;
    assert_eq!(
        on_disk.iter().find(|r| r.id == running.id).unwrap().state,
        ExecutionState::Running
    );

    let second = harness_with(|_| vec![], Some(path.clone()));
    let overview = second.sup.overview();
    let recovered = overview
        .executions
        .iter()
        .find(|r| r.id == running.id)
        .unwrap();
    assert_eq!(recovered.state, ExecutionState::Interrupted);
    let finished = overview.executions.iter().find(|r| r.id == ok.id).unwrap();
    assert_eq!(finished.state, ExecutionState::Failed);
    assert_eq!(finished.exit_code, Some(3));
    assert!(overview
        .notices
        .iter()
        .any(|n| n.contains("marked interrupted")));
    assert_eq!(overview.active_count, 0);
    // Output from a previous session is not retained, and says so.
    assert!(!second.sup.output(&ok.id).unwrap().available);

    // Clean up the first supervisor's process.
    first.sup.shutdown(Duration::from_secs(10)).await;
    drop(first.dir);
}

/// If the process that owns the supervisor dies abruptly, Windows must still terminate its
/// children (Job Object with kill-on-close). Unix relies on graceful shutdown; see ADR-005.
#[cfg(windows)]
#[tokio::test]
async fn children_do_not_outlive_a_crashed_owner() {
    use std::io::{BufRead, BufReader};
    let dir = tempfile::tempdir().unwrap();
    let mut host = std::process::Command::new(diag_exe())
        .arg("--host")
        .arg(dir.path())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(host.stdout.take().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let hosted: u32 = line
        .trim()
        .trim_start_matches("hosted-pid:")
        .parse()
        .unwrap();
    assert!(pid_alive(hosted));

    host.kill().unwrap(); // TerminateProcess: no graceful shutdown runs.
    host.wait().unwrap();
    wait_pid_gone(hosted).await;
}

// ---- Launch specs (Phase 3: adapter-built launches) --------------------------------------

fn spec(id: &str, scenario: Scenario, dir: &Path) -> plenipo_runtime::LaunchSpec {
    let mut spec = profile(id, scenario, dir, 60).to_spec();
    spec.label = format!("{id} spec");
    spec
}

async fn drain(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<plenipo_runtime::OutputLine>,
) -> Vec<plenipo_runtime::OutputLine> {
    let mut lines = vec![];
    while let Some(line) = rx.recv().await {
        lines.push(line);
    }
    lines
}

#[tokio::test]
async fn launch_writes_stdin_and_the_observer_sees_every_line_in_order() {
    let h = harness();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut s = spec("stdin", Scenario::Stdin, h.dir.path());
    let input = "first line\nsecond \"quoted\" line; & | > ok\n";
    s.stdin = Some(input.as_bytes().to_vec());
    s.observer = Some(tx);
    let started = h.sup.launch(s).await.unwrap();
    let observed = tokio::time::timeout(WAIT, drain(rx)).await.unwrap();
    let done = h.sup.wait(&started.id).await.unwrap();
    assert_eq!(done.state, ExecutionState::Succeeded, "{done:?}");

    let texts: Vec<_> = observed.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "stdin: first line",
            "stdin: second \"quoted\" line; & | > ok",
            &format!("stdin bytes: {}", input.len())
        ]
    );
    // Observer lines carry the same sequence numbers as the UI stream.
    let seqs: Vec<_> = observed.iter().map(|l| l.seq).collect();
    assert_eq!(seqs, [1, 2, 3]);
    assert_eq!(
        h.sink
            .lines(&started.id)
            .iter()
            .map(|l| l.3)
            .collect::<Vec<_>>(),
        seqs
    );
    // Stdin content is not recorded in the execution metadata.
    assert!(!format!("{done:?}").contains("first line"));
}

#[tokio::test]
async fn without_stdin_the_child_reads_end_of_file() {
    let h = harness();
    let started = h
        .sup
        .launch(spec("stdin", Scenario::Stdin, h.dir.path()))
        .await
        .unwrap();
    let done = wait_terminal(&h.sup, &started.id).await;
    assert_eq!(done.state, ExecutionState::Succeeded);
    wait_for_line(&h.sink, &started.id, "stdin bytes: 0").await;
}

#[tokio::test]
async fn observer_gets_full_long_lines_while_the_ui_stream_stays_capped() {
    let h = harness();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut s = spec("burst", Scenario::Burst, h.dir.path());
    s.max_line_bytes = Some(256 * 1024);
    s.observer = Some(tx);
    let started = h.sup.launch(s).await.unwrap();
    let observed = tokio::time::timeout(WAIT, drain(rx)).await.unwrap();
    assert_eq!(observed.len(), 5002, "every line reaches the observer");
    let long = observed.iter().find(|l| l.text.starts_with("xxx")).unwrap();
    assert_eq!(long.text.len(), 100_000);
    assert!(!long.truncated);

    wait_terminal(&h.sup, &started.id).await;
    let shown = h
        .sink
        .lines(&started.id)
        .into_iter()
        .find(|l| l.2.starts_with("xxx"))
        .unwrap();
    assert_eq!(shown.2.len(), SupervisorConfig::default().max_line_bytes);
}

#[tokio::test]
async fn launch_requires_the_executable_to_be_allowed_by_core() {
    let h = harness();
    // A hard link: a second path to the same file, without the ETXTBSY race a fresh copy has
    // on Linux while other tests fork.
    let links = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap();
    let copy = links
        .path()
        .join(if cfg!(windows) { "agent.exe" } else { "agent" });
    if std::fs::hard_link(diag_exe(), &copy).is_err() {
        std::fs::copy(diag_exe(), &copy).unwrap();
    }
    let mut s = spec("stdin", Scenario::Stdin, h.dir.path());
    s.executable = copy.clone();
    assert!(matches!(
        h.sup.launch(s.clone()).await,
        Err(RuntimeError::Policy(
            plenipo_runtime::PolicyError::NotAllowed(_)
        ))
    ));
    assert!(h.sup.overview().executions.is_empty(), "nothing recorded");

    // Core located it (e.g. agent CLI detection) and allows it.
    let allowed = h.sup.allow_executable(&copy).unwrap();
    assert_eq!(allowed, dunce_canonical(&copy));
    let started = h.sup.launch(s).await.unwrap();
    assert_eq!(
        wait_terminal(&h.sup, &started.id).await.state,
        ExecutionState::Succeeded
    );
    assert!(h.sup.allow_executable(Path::new("relative/agent")).is_err());
}

fn dunce_canonical(p: &Path) -> PathBuf {
    let c = std::fs::canonicalize(p).unwrap();
    // Match `dunce`: no `\\?\` prefix on Windows for ordinary paths.
    PathBuf::from(c.to_string_lossy().trim_start_matches(r"\\?\"))
}

#[tokio::test]
async fn launch_validates_the_spec() {
    let h = harness();
    let base = spec("stdin", Scenario::Stdin, h.dir.path());
    let cases: Vec<(&str, plenipo_runtime::LaunchSpec)> = vec![
        (
            "id",
            plenipo_runtime::LaunchSpec {
                profile_id: "Bad ID".into(),
                ..base.clone()
            },
        ),
        (
            "label",
            plenipo_runtime::LaunchSpec {
                label: " ".into(),
                ..base.clone()
            },
        ),
        (
            "dir",
            plenipo_runtime::LaunchSpec {
                working_dir: "relative".into(),
                ..base.clone()
            },
        ),
        (
            "env",
            plenipo_runtime::LaunchSpec {
                env: vec![("A=B".into(), "x".into())],
                ..base.clone()
            },
        ),
        (
            "runtime",
            plenipo_runtime::LaunchSpec {
                max_runtime: Duration::ZERO,
                ..base.clone()
            },
        ),
        (
            "lines",
            plenipo_runtime::LaunchSpec {
                max_line_bytes: Some(0),
                ..base.clone()
            },
        ),
    ];
    for (what, s) in cases {
        let err = h.sup.launch(s).await.expect_err(what);
        assert!(err.is_caller_error(), "{what}: {err}");
    }
    assert!(h.sup.overview().executions.is_empty());
}

#[tokio::test]
async fn annotate_updates_agent_attribution_and_persists_it() {
    let dir = tempfile::tempdir().unwrap();
    let store_path = dir.path().join("executions.json");
    let h = harness_with(|_| vec![], Some(store_path.clone()));
    let mut s = spec("stdin", Scenario::Stdin, h.dir.path());
    s.agent = Some(Box::new(plenipo_runtime::AgentAttribution {
        runtime_id: "test-runtime".into(),
        provider: "test".into(),
        session_id: "s1".into(),
        task_id: "t1".into(),
        model: None,
        provider_session_id: None,
        usage: None,
    }));
    let started = h.sup.launch(s).await.unwrap();
    let updated = h
        .sup
        .annotate(&started.id, |a| {
            a.provider_session_id = Some("provider-123".into());
            a.model = Some("model-x".into());
        })
        .unwrap();
    assert_eq!(
        updated
            .agent
            .as_ref()
            .unwrap()
            .provider_session_id
            .as_deref(),
        Some("provider-123")
    );
    let done = h.sup.wait(&started.id).await.unwrap();
    assert_eq!(done.state, ExecutionState::Succeeded);
    assert_eq!(
        done.agent.as_ref().unwrap().model.as_deref(),
        Some("model-x")
    );

    let saved = MetadataStore::file(&store_path).load().records;
    assert_eq!(saved[0].agent, done.agent, "attribution persisted");
    // Plain profile launches carry no attribution, and annotate leaves them alone.
    let plain = h.sup.start("echo").await.unwrap();
    assert!(plain.agent.is_none());
    assert!(h
        .sup
        .annotate(&plain.id, |_| unreachable!())
        .unwrap()
        .agent
        .is_none());
    assert!(h.sup.annotate("nope", |_| {}).is_err());
    assert!(h.sup.wait("nope").await.is_err());
}

#[tokio::test]
async fn a_slow_observer_neither_stalls_reading_nor_loses_output() {
    // Consuming takes longer than the drain timeout (2 s) after the process exits. Reading the
    // pipes must not wait for the observer, or the tail of the output would be cut off.
    let h = harness();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut s = spec("burst", Scenario::Burst, h.dir.path());
    s.max_line_bytes = Some(256 * 1024);
    s.observer = Some(tx);
    let started = h.sup.launch(s).await.unwrap();
    let slow = tokio::spawn(async move {
        let mut n = 0;
        while let Some(line) = rx.recv().await {
            n += 1;
            if n % 2 == 0 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            if line.text == "burst done" {
                return n;
            }
        }
        n
    });
    let done = h.sup.wait(&started.id).await.unwrap();
    assert_eq!(done.state, ExecutionState::Succeeded);
    assert_eq!(done.detail, None, "no descendants were terminated");
    assert_eq!(
        tokio::time::timeout(WAIT, slow).await.unwrap().unwrap(),
        5002
    );
}
