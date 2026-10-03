//! Whether a link named in a worker's answer exists (Phase 25, item 4.8; ADR-257, catch made-up
//! answers, step 2). Liaison asks while it checks an answer; the capability broker looks, through
//! Guard (only the owner's allowed websites, and GitHub's own `gh` for a pull request).
//!
//! Liaison checks answers on threads that must not wait on async work themselves, so each look
//! runs on a thread of its own. A link looked at in the last ten minutes is not looked at again.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use plenipo_capabilities::broker::links::LinkVerdict;
use plenipo_capabilities::Broker;
use plenipo_liaison::LinkChecker;

/// How long a look at a link is remembered.
const REMEMBER: Duration = Duration::from_secs(10 * 60);
/// Most links remembered.
const MAX_REMEMBERED: usize = 512;

pub struct Links {
    broker: Broker,
    seen: Mutex<HashMap<String, (Option<bool>, Instant)>>,
}

impl Links {
    pub fn new(broker: Broker) -> Self {
        Self {
            broker,
            seen: Mutex::new(HashMap::new()),
        }
    }
}

impl LinkChecker for Links {
    fn exists(&self, url: &str) -> Option<bool> {
        if let Some((found, at)) = self
            .seen
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(url)
            .copied()
        {
            if at.elapsed() < REMEMBER {
                return found;
            }
        }
        let (broker, link) = (self.broker.clone(), url.to_owned());
        let verdict = std::thread::spawn(move || {
            tauri::async_runtime::block_on(async move { broker.check_link(&link).await })
        })
        .join()
        .unwrap_or_else(|_| LinkVerdict::NotChecked("the look stopped unexpectedly".into()));
        let found = match verdict {
            LinkVerdict::Exists => Some(true),
            LinkVerdict::Missing => Some(false),
            LinkVerdict::NotChecked(_) => None,
        };
        let mut seen = self.seen.lock().unwrap_or_else(|p| p.into_inner());
        if seen.len() >= MAX_REMEMBERED {
            seen.retain(|_, (_, at)| at.elapsed() < REMEMBER);
            if seen.len() >= MAX_REMEMBERED {
                seen.clear();
            }
        }
        seen.insert(url.to_owned(), (found, Instant::now()));
        found
    }
}
