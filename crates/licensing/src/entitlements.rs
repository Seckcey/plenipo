//! The one place that decides every Free limit: [`Entitlements::check`] (Phase 11A).
//!
//! Workforce, Liaison, the capability broker, and the app's commands ask here; nothing else decides
//! for itself whether an owner is Pro. A limit reached returns [`Decision::Blocked`] with plain
//! words naming what Pro adds — never a silent failure, never a raw error.
//!
//! Free limits count what is live now, across the whole PC (ADR-110): organizations,
//! departments, and projects that are not archived, and workers on the job at this moment.
//! Nothing here deletes, hides, or stops anything: when Pro ends, only creating something new
//! past a Free limit is refused (ADR-022 §6).
//!
//! Safety is never here: Guard, permissions, approvals, the Vault, the Ledger, the Activity trail,
//! spending caps, and every AI tool are outside this system entirely.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::words;

/// Free or Pro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Edition {
    Free,
    Pro,
}

/// Something Free limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Limit {
    /// One organization on Free (ADR-091 §2).
    Organizations,
    /// One department on Free.
    Departments,
    /// One project on Free.
    Projects,
    /// Three workers on the job at once on Free (ADR-113).
    WorkersAtOnce,
    /// A department set up from a business template (ADR-114).
    BusinessDepartment,
    /// Connecting a service, and offering a connection's tools to a worker (ADR-068).
    Connections,
    /// Adding an add-on tool, and offering one to a worker (ADR-068).
    AddOnTools,
    /// Workers that learn from their work (ADR-112).
    Lessons,
}

impl Limit {
    pub const ALL: [Self; 8] = [
        Self::Organizations,
        Self::Departments,
        Self::Projects,
        Self::WorkersAtOnce,
        Self::BusinessDepartment,
        Self::Connections,
        Self::AddOnTools,
        Self::Lessons,
    ];
}

pub const FREE_ORGANIZATIONS: u32 = 1;
pub const FREE_DEPARTMENTS: u32 = 1;
pub const FREE_PROJECTS: u32 = 1;
pub const FREE_WORKERS_AT_ONCE: u32 = 3;

/// A worker let in on Free counts as on the job this long, or until its task is.
const ADMITTED_FOR: Duration = Duration::from_secs(60);
/// How many tasks' editions are remembered (the oldest are forgotten first).
const TASKS_KEPT: usize = 20_000;

/// A limit reached, in plain words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Blocked {
    pub limit: Limit,
    /// What Free has, what Pro adds, and where to enter a key.
    pub message: String,
}

impl Blocked {
    pub fn new(limit: Limit) -> Self {
        Self {
            limit,
            message: words::message(limit),
        }
    }
}

impl std::fmt::Display for Blocked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Blocked {}

/// Allowed, or Blocked with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allowed,
    Blocked(Blocked),
}

impl Decision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    pub fn into_result(self) -> Result<(), Blocked> {
        match self {
            Self::Allowed => Ok(()),
            Self::Blocked(b) => Err(b),
        }
    }
}

/// What is live on the PC now, counted across every organization (the app provides it).
pub trait Usage: Send + Sync {
    /// Organizations that are not archived.
    fn organizations(&self) -> u32;
    /// Departments that are not archived, in every open organization.
    fn departments(&self) -> u32;
    /// Projects that are not archived, in every open organization.
    fn projects(&self) -> u32;
    /// The tasks of the workers on the job now (running, or waiting for an approval).
    fn workers_on_the_job(&self) -> Vec<String>;
}

/// A worker let in to start on Free: it counts as on the job until its task is, for a minute at
/// most. On Pro it counts for nothing.
#[derive(Debug)]
#[must_use = "release an admission whose worker did not start"]
pub struct Admission {
    token: Option<String>,
}

struct Admitted {
    token: String,
    task_id: Option<String>,
    at: Instant,
}

#[derive(Default)]
struct TaskEditions {
    by_task: HashMap<String, Edition>,
    order: VecDeque<String>,
}

/// The PC's Free or Pro, and every Free limit.
pub struct Entitlements {
    /// Always this edition (the tests, and any service not given the PC's).
    fixed: bool,
    edition: RwLock<Edition>,
    usage: RwLock<Option<Arc<dyn Usage>>>,
    tasks: Mutex<TaskEditions>,
    admitted: Mutex<Vec<Admitted>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Entitlements {
    /// The PC's entitlements, starting at `edition`; the license host keeps it current.
    pub fn new(edition: Edition) -> Arc<Self> {
        Arc::new(Self::build(edition, false))
    }

    /// Always Pro: for the tests, and for any service built without the PC's entitlements.
    pub fn unlocked() -> Arc<Self> {
        Arc::new(Self::build(Edition::Pro, true))
    }

    /// Always `edition` (tests).
    pub fn fixed(edition: Edition) -> Arc<Self> {
        Arc::new(Self::build(edition, true))
    }

    fn build(edition: Edition, fixed: bool) -> Self {
        Self {
            fixed,
            edition: RwLock::new(edition),
            usage: RwLock::new(None),
            tasks: Mutex::default(),
            admitted: Mutex::default(),
        }
    }

    pub fn edition(&self) -> Edition {
        *self.edition.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// Change the edition; true when it changed. A fixed one never changes.
    pub fn set_edition(&self, edition: Edition) -> bool {
        if self.fixed {
            return false;
        }
        let mut current = self.edition.write().unwrap_or_else(PoisonError::into_inner);
        let changed = *current != edition;
        *current = edition;
        changed
    }

    /// How to count what is live (the app provides it once its organizations are open).
    pub fn set_usage(&self, usage: Arc<dyn Usage>) {
        *self.usage.write().unwrap_or_else(PoisonError::into_inner) = Some(usage);
    }

    fn usage(&self) -> Option<Arc<dyn Usage>> {
        self.usage
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Allowed, or Blocked with plain words, for `limit` now.
    pub fn check(&self, limit: Limit) -> Decision {
        if self.edition() == Edition::Pro {
            return Decision::Allowed;
        }
        let usage = self.usage();
        let count = |f: fn(&dyn Usage) -> u32| usage.as_deref().map_or(0, f);
        let reached = match limit {
            Limit::Organizations => count(|u| u.organizations()) >= FREE_ORGANIZATIONS,
            Limit::Departments => count(|u| u.departments()) >= FREE_DEPARTMENTS,
            Limit::Projects => count(|u| u.projects()) >= FREE_PROJECTS,
            Limit::WorkersAtOnce => self.on_the_job(usage.as_deref()) >= FREE_WORKERS_AT_ONCE,
            Limit::BusinessDepartment | Limit::Connections | Limit::AddOnTools | Limit::Lessons => {
                true
            }
        };
        if reached {
            Decision::Blocked(Blocked::new(limit))
        } else {
            Decision::Allowed
        }
    }

    /// The edition task `task_id` runs under: the edition when it was first asked about. A task
    /// that started on Pro keeps its tools and lessons until it finishes, even if Pro ends
    /// meanwhile (ADR-068 §4, ADR-112 §5).
    pub fn task_edition(&self, task_id: &str) -> Edition {
        let mut tasks = lock(&self.tasks);
        if let Some(e) = tasks.by_task.get(task_id) {
            return *e;
        }
        let edition = self.edition();
        tasks.by_task.insert(task_id.to_owned(), edition);
        tasks.order.push_back(task_id.to_owned());
        while tasks.order.len() > TASKS_KEPT {
            if let Some(old) = tasks.order.pop_front() {
                tasks.by_task.remove(&old);
            }
        }
        edition
    }

    /// Like [`Self::check`] for a Pro feature a task uses (connections, add-on tools, lessons),
    /// under the edition the task started with.
    pub fn check_for_task(&self, task_id: Option<&str>, limit: Limit) -> Decision {
        let edition = task_id.map_or_else(|| self.edition(), |t| self.task_edition(t));
        if edition == Edition::Pro {
            Decision::Allowed
        } else {
            Decision::Blocked(Blocked::new(limit))
        }
    }

    /// Workers on the job now, with those just let in, forgetting admissions that are over.
    fn on_the_job(&self, usage: Option<&dyn Usage>) -> u32 {
        let running = usage.map(Usage::workers_on_the_job).unwrap_or_default();
        let mut admitted = lock(&self.admitted);
        admitted.retain(|a| {
            a.at.elapsed() < ADMITTED_FOR && a.task_id.as_ref().is_none_or(|t| !running.contains(t))
        });
        u32::try_from(running.len() + admitted.len()).unwrap_or(u32::MAX)
    }

    /// Let a worker start (for task `task_id`, when known), or say it waits its turn. On Free, a
    /// worker let in counts at once, so workers starting together never pass three.
    pub fn admit_worker(&self, task_id: Option<&str>) -> Result<Admission, Blocked> {
        if self.edition() == Edition::Pro {
            return Ok(Admission { token: None });
        }
        let usage = self.usage();
        let running = usage
            .as_deref()
            .map(Usage::workers_on_the_job)
            .unwrap_or_default();
        let mut admitted = lock(&self.admitted);
        admitted.retain(|a| {
            a.at.elapsed() < ADMITTED_FOR && a.task_id.as_ref().is_none_or(|t| !running.contains(t))
        });
        // Already counted: nothing new is on the job.
        if let Some(t) = task_id {
            if running.iter().any(|r| r == t)
                || admitted.iter().any(|a| a.task_id.as_deref() == Some(t))
            {
                return Ok(Admission { token: None });
            }
        }
        let on_the_job = running.len() + admitted.len();
        if on_the_job >= FREE_WORKERS_AT_ONCE as usize {
            return Err(Blocked::new(Limit::WorkersAtOnce));
        }
        let token = uuid::Uuid::new_v4().to_string();
        admitted.push(Admitted {
            token: token.clone(),
            task_id: task_id.map(str::to_owned),
            at: Instant::now(),
        });
        Ok(Admission { token: Some(token) })
    }

    /// The admitted worker's task is `task_id` (learned once it started).
    pub fn bind(&self, admission: &Admission, task_id: &str) {
        let Some(token) = &admission.token else {
            return;
        };
        if let Some(a) = lock(&self.admitted).iter_mut().find(|a| &a.token == token) {
            a.task_id = Some(task_id.to_owned());
        }
    }

    /// The admitted worker did not start: its place is free again.
    pub fn release(&self, admission: Admission) {
        if let Some(token) = admission.token {
            lock(&self.admitted).retain(|a| a.token != token);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[derive(Default)]
    pub(crate) struct Counts {
        pub organizations: u32,
        pub departments: u32,
        pub projects: u32,
        pub working: Mutex<Vec<String>>,
    }

    impl Usage for Counts {
        fn organizations(&self) -> u32 {
            self.organizations
        }
        fn departments(&self) -> u32 {
            self.departments
        }
        fn projects(&self) -> u32 {
            self.projects
        }
        fn workers_on_the_job(&self) -> Vec<String> {
            lock(&self.working).clone()
        }
    }

    fn free_with(counts: Counts) -> (Arc<Entitlements>, Arc<Counts>) {
        let e = Entitlements::new(Edition::Free);
        let counts = Arc::new(counts);
        e.set_usage(counts.clone());
        (e, counts)
    }

    fn blocked(d: Decision) -> Limit {
        match d {
            Decision::Blocked(b) => b.limit,
            Decision::Allowed => panic!("expected Blocked"),
        }
    }

    #[test]
    fn free_allows_the_first_organization_department_and_project_and_blocks_the_second() {
        let (e, _) = free_with(Counts::default());
        for limit in [Limit::Organizations, Limit::Departments, Limit::Projects] {
            assert!(e.check(limit).is_allowed(), "{limit:?}");
        }
        let (e, _) = free_with(Counts {
            organizations: 1,
            departments: 1,
            projects: 1,
            ..Counts::default()
        });
        for limit in [Limit::Organizations, Limit::Departments, Limit::Projects] {
            assert_eq!(blocked(e.check(limit)), limit);
        }
    }

    #[test]
    fn pro_has_no_limits() {
        let e = Entitlements::new(Edition::Pro);
        e.set_usage(Arc::new(Counts {
            organizations: 9,
            departments: 99,
            projects: 999,
            working: Mutex::new((0..50).map(|i| i.to_string()).collect()),
        }));
        for limit in Limit::ALL {
            assert!(e.check(limit).is_allowed(), "{limit:?}");
        }
        assert!(e.admit_worker(None).is_ok());
    }

    #[test]
    fn free_blocks_the_pro_features() {
        let (e, _) = free_with(Counts::default());
        for limit in [
            Limit::BusinessDepartment,
            Limit::Connections,
            Limit::AddOnTools,
            Limit::Lessons,
        ] {
            let Decision::Blocked(b) = e.check(limit) else {
                panic!("{limit:?}");
            };
            assert_eq!(b.message, words::message(limit));
        }
    }

    #[test]
    fn free_lets_three_workers_on_the_job_and_the_fourth_waits() {
        let (e, counts) = free_with(Counts::default());
        *lock(&counts.working) = vec!["t1".into(), "t2".into()];
        let third = e.admit_worker(Some("t3")).unwrap();
        // The third counts at once, before its task is running.
        assert_eq!(
            e.admit_worker(Some("t4")).unwrap_err().limit,
            Limit::WorkersAtOnce
        );
        assert_eq!(blocked(e.check(Limit::WorkersAtOnce)), Limit::WorkersAtOnce);
        // Its task runs: still three, not four.
        lock(&counts.working).push("t3".into());
        assert!(e.admit_worker(Some("t4")).is_err());
        // One finishes: the fourth starts.
        lock(&counts.working).retain(|t| t != "t1");
        let fourth = e.admit_worker(Some("t4")).unwrap();
        e.release(third);
        e.release(fourth);
    }

    #[test]
    fn workers_starting_together_never_pass_three() {
        let (e, _) = free_with(Counts::default());
        let got: Vec<_> = (0..6).map(|_| e.admit_worker(None)).collect();
        assert_eq!(got.iter().filter(|r| r.is_ok()).count(), 3);
        for a in got.into_iter().flatten() {
            e.release(a);
        }
        // Released: three places again.
        let again: Vec<_> = (0..3).map(|_| e.admit_worker(None).unwrap()).collect();
        for a in again {
            e.release(a);
        }
    }

    #[test]
    fn a_worker_already_on_the_job_is_not_counted_twice() {
        let (e, counts) = free_with(Counts::default());
        *lock(&counts.working) = vec!["t1".into(), "t2".into(), "t3".into()];
        // Continuing t1 (it is already on the job).
        assert!(e.admit_worker(Some("t1")).is_ok());
        assert!(e.admit_worker(Some("t9")).is_err());
    }

    #[test]
    fn an_admission_bound_to_its_task_counts_once() {
        let (e, counts) = free_with(Counts::default());
        let a = e.admit_worker(None).unwrap();
        e.bind(&a, "t1");
        lock(&counts.working).push("t1".into());
        // One on the job, not two.
        let b = e.admit_worker(None).unwrap();
        let c = e.admit_worker(None).unwrap();
        assert!(e.admit_worker(None).is_err());
        for x in [a, b, c] {
            e.release(x);
        }
    }

    #[test]
    fn a_task_keeps_the_edition_it_started_with() {
        let e = Entitlements::new(Edition::Pro);
        assert_eq!(e.task_edition("running"), Edition::Pro);
        assert!(e.set_edition(Edition::Free));
        // The running task keeps Pro's tools and lessons; the next task gets none.
        assert!(e
            .check_for_task(Some("running"), Limit::Connections)
            .is_allowed());
        assert_eq!(
            blocked(e.check_for_task(Some("next"), Limit::Connections)),
            Limit::Connections
        );
        // Pro back: new tasks get them again; "next" keeps Free until it finishes.
        e.set_edition(Edition::Pro);
        assert!(e.check_for_task(Some("after"), Limit::Lessons).is_allowed());
        assert!(!e.check_for_task(Some("next"), Limit::Lessons).is_allowed());
    }

    #[test]
    fn a_fixed_edition_never_changes() {
        let e = Entitlements::unlocked();
        assert!(!e.set_edition(Edition::Free));
        assert_eq!(e.edition(), Edition::Pro);
        let f = Entitlements::fixed(Edition::Free);
        assert!(!f.set_edition(Edition::Pro));
        assert_eq!(f.edition(), Edition::Free);
    }

    #[test]
    fn without_counts_free_limits_are_not_hit() {
        // Fail-open: until the app says how to count, nothing is refused for a count.
        let e = Entitlements::new(Edition::Free);
        assert!(e.check(Limit::Departments).is_allowed());
        assert!(!e.check(Limit::Connections).is_allowed());
    }

    #[test]
    fn remembered_tasks_are_bounded() {
        let e = Entitlements::new(Edition::Pro);
        for i in 0..(TASKS_KEPT + 10) {
            e.task_edition(&i.to_string());
        }
        assert_eq!(lock(&e.tasks).by_task.len(), TASKS_KEPT);
        assert!(!lock(&e.tasks).by_task.contains_key("0"));
    }
}
