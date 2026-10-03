//! When a plan runs out (Phase 25, item 4.2; ADR-253): once a minute, each organization gives
//! the work a usage limit stopped back to its workers when the limit is over, unless the owner
//! left it stopped. Nothing is picked up while Stop all work holds the work.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::orgs::OrgStack;

/// How often each organization looks.
const LOOK_EVERY: Duration = Duration::from_secs(60);

/// Start looking for `stack`. It keeps only a weak link, so an organization that is closed,
/// archived, or deleted is never kept open by it.
pub fn start(stack: &Arc<OrgStack>) {
    let weak: Weak<OrgStack> = Arc::downgrade(stack);
    let _ = std::thread::Builder::new()
        .name("plenipo-limit-pick-up".into())
        .spawn(move || loop {
            std::thread::sleep(LOOK_EVERY);
            if !look(&weak) {
                break;
            }
        });
}

/// One look. `false`: stop looking (the organization stopped, or is gone).
fn look(weak: &Weak<OrgStack>) -> bool {
    let Some(stack) = weak.upgrade() else {
        return false;
    };
    if stack.stopped.load(Ordering::SeqCst) {
        return false;
    }
    match tauri::async_runtime::block_on(stack.workforce.pick_up_after_limits()) {
        Ok(0) => {}
        Ok(n) => log::info!("picked up {n} objective(s) a usage limit had stopped"),
        Err(e) => log::warn!("could not pick up work a usage limit stopped: {e}"),
    }
    true
}
