//! Whether the links named in a worker's answer exist (Phase 25, item 4.8; ADR-257, catch made-up
//! answers, step 2). Liaison asks while it checks an answer; the capability broker looks, through
//! Guard (only the owner's allowed websites, and GitHub's own `gh` for a pull request), and only
//! where the worker that wrote the answer could have looked itself.
//!
//! Liaison checks answers on threads that must not wait on async work themselves, and one of
//! them ends the worker's turn, so an answer's looks run at once, each on a thread of its own,
//! and Liaison waits for them [`LOOK_LIMIT`] at most (the security review of #156): a look not
//! finished by then is "not checked". A link looked at for the same task in the last ten minutes
//! is not looked at again.

use std::collections::HashMap;
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

use plenipo_capabilities::broker::links::LinkVerdict;
use plenipo_capabilities::Broker;
use plenipo_liaison::LinkChecker;

/// The longest an answer's check waits for its links, all of them together.
const LOOK_LIMIT: Duration = Duration::from_secs(6);
/// How long a look at a link is remembered.
const REMEMBER: Duration = Duration::from_secs(10 * 60);
/// Most links remembered.
const MAX_REMEMBERED: usize = 512;

/// (task, link) → what was found, and when: a look depends on what the task's worker could reach,
/// so it is remembered for that task only.
type Seen = HashMap<(String, String), (Option<bool>, Instant)>;

pub struct Links {
    broker: Broker,
    seen: Mutex<Seen>,
}

impl Links {
    pub fn new(broker: Broker) -> Self {
        Self {
            broker,
            seen: Mutex::new(HashMap::new()),
        }
    }

    fn remembered(&self, task_id: &str, url: &str) -> Option<Option<bool>> {
        let seen = self.seen.lock().unwrap_or_else(|p| p.into_inner());
        seen.get(&(task_id.to_owned(), url.to_owned()))
            .filter(|(_, at)| at.elapsed() < REMEMBER)
            .map(|(found, _)| *found)
    }

    fn remember(&self, task_id: &str, url: &str, found: Option<bool>) {
        let mut seen = self.seen.lock().unwrap_or_else(|p| p.into_inner());
        if seen.len() >= MAX_REMEMBERED {
            seen.retain(|_, (_, at)| at.elapsed() < REMEMBER);
            if seen.len() >= MAX_REMEMBERED {
                seen.clear();
            }
        }
        seen.insert(
            (task_id.to_owned(), url.to_owned()),
            (found, Instant::now()),
        );
    }
}

impl LinkChecker for Links {
    fn check(&self, task_id: &str, links: &[String]) -> Vec<Option<bool>> {
        let mut found: Vec<Option<Option<bool>>> = links
            .iter()
            .map(|url| self.remembered(task_id, url))
            .collect();
        let (sent, looked) = mpsc::channel();
        for (i, url) in links
            .iter()
            .enumerate()
            .filter(|(i, _)| found[*i].is_none())
        {
            let (broker, task, link, sent) = (
                self.broker.clone(),
                task_id.to_owned(),
                url.clone(),
                sent.clone(),
            );
            std::thread::spawn(move || {
                let verdict =
                    tauri::async_runtime::block_on(
                        async move { broker.check_link(&task, &link).await },
                    );
                // Nobody listens once the answer's check stopped waiting.
                let _ = sent.send((i, verdict));
            });
        }
        drop(sent);
        let deadline = Instant::now() + LOOK_LIMIT;
        while let Ok((i, verdict)) =
            looked.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        {
            let result = match verdict {
                LinkVerdict::Exists => Some(true),
                LinkVerdict::Missing => Some(false),
                LinkVerdict::NotChecked(_) => None,
            };
            self.remember(task_id, &links[i], result);
            found[i] = Some(result);
        }
        found.into_iter().map(Option::flatten).collect()
    }
}
