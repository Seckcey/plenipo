//! Community messages in the app (Phase 24, ADR-164): picking up what waits for this PC, and
//! the commands of the Messages page. Organizations' windows only (`capabilities/default.json`):
//! a pop-out, the sign, and web pages cannot call them.
//!
//! Messages are kept in the first organization's Ledger (the PC's shared record, ADR-164 §10).
//! Nothing listens on this PC: while it is signed in and joined, it asks 8 West for what waits,
//! each request held up to 25 seconds (ADR-164 §3). A message is never shown as a web page; a
//! link in one opens only in your own browser, after you say yes on screen (ADR-164 §4).

use std::sync::Arc;
use std::time::Duration;

use plenipo_capabilities::fence;
use plenipo_community::messages::{
    self, ConversationSummary, ConversationView, MessageView, PICK_UP_WAIT_SECS,
};
use plenipo_community::service::{Refused, Stage};
use plenipo_core::CommandError;
use plenipo_ledger::Ledger;
use plenipo_licensing::Limit;
use plenipo_workforce::Workforce;
use tauri::{AppHandle, Emitter as _, Runtime, State};

use crate::community_host::{self, CommunityState};
use crate::orgs::Org;

/// Told to every window when messages arrive or change (the Messages page reads them again).
pub const MESSAGES_EVENT: &str = "plenipo://community-messages";
/// At most one pick-up every this long (contract §15: 720 an hour).
const LEAST_BETWEEN: Duration = Duration::from_secs(5);
/// While this PC is not signed in, look at whether it is every this long.
const IDLE_LOOK: Duration = Duration::from_secs(15);
/// After a failed pick-up, wait this long at first, then twice as long each time, up to 5 min.
const FIRST_BACKOFF: Duration = Duration::from_secs(5);
const MOST_BACKOFF: Duration = Duration::from_secs(5 * 60);
/// The longest web address a message's link may open.
const MOST_LINK_CHARS: usize = 2048;

fn refused(r: Refused) -> CommandError {
    CommandError::invalid_input(r.0)
}

/// Tell every window that messages changed.
pub fn messages_changed<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit(MESSAGES_EVENT, "changed");
}

/// The first organization's Ledger, where messages are kept.
fn ledger(state: &CommunityState) -> Result<Arc<Ledger>, CommandError> {
    state
        .ledger()
        .ok_or_else(|| CommandError::invalid_input("Plenipo is still starting."))
}

/// Pick up messages while this PC is signed in and joined, for as long as the app runs.
pub fn start_picking_up<R: Runtime>(app: &AppHandle<R>, state: Arc<CommunityState>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut backoff = FIRST_BACKOFF;
        loop {
            let view = state.view();
            let joined = view.stage == Stage::SignedIn && view.member.is_some();
            let Some(ledger) = state.ledger().filter(|_| joined) else {
                tokio::time::sleep(IDLE_LOOK).await;
                continue;
            };
            let started = tokio::time::Instant::now();
            match state.community.pick_up(&ledger, PICK_UP_WAIT_SECS).await {
                Ok(picked) => {
                    backoff = FIRST_BACKOFF;
                    if picked.kept > 0 || picked.notices > 0 {
                        messages_changed(&app);
                    }
                    if picked.me_changed {
                        state.community.refresh().await;
                        community_host::changed(&app);
                    }
                    let waited = started.elapsed();
                    if waited < LEAST_BETWEEN {
                        tokio::time::sleep(LEAST_BETWEEN - waited).await;
                    }
                }
                Err(_) => {
                    // Signed out by 8 West, closed, or not reachable: Settings → Community says
                    // which; try again later.
                    community_host::changed(&app);
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(MOST_BACKOFF);
                }
            }
        }
    });
}

/// Your conversations and requests, the newest first.
#[tauri::command]
pub async fn community_conversations(
    state: State<'_, Arc<CommunityState>>,
) -> Result<Vec<ConversationSummary>, CommandError> {
    let ledger = ledger(&state)?;
    tauri::async_runtime::spawn_blocking(move || messages::conversations(&ledger))
        .await
        .map_err(|e| CommandError::internal(e.to_string()))?
        .map_err(refused)
}

/// One conversation: up to 100 messages before `before` (Unix seconds; none: the newest).
/// Opening it marks its messages seen.
#[tauri::command]
pub async fn community_conversation<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    member_id: String,
    before: Option<i64>,
) -> Result<Option<ConversationView>, CommandError> {
    let ledger = ledger(&state)?;
    let view = tauri::async_runtime::spawn_blocking(move || {
        messages::conversation(&ledger, &member_id, before)
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
    .map_err(refused)?;
    messages_changed(&app);
    Ok(view)
}

/// **Send** a message. A first message to someone you don't talk with is a request, which is
/// part of Pro (ADR-162 §5).
#[tauri::command]
pub async fn send_community_message<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    to: String,
    name: String,
    text: String,
    reply_to: Option<String>,
) -> Result<MessageView, CommandError> {
    let state = state.inner().clone();
    let ledger = ledger(&state)?;
    let talking = ledger
        .community_person(&to)
        .ok()
        .flatten()
        .is_some_and(|p| {
            use plenipo_ledger::community::PersonState;
            matches!(
                p.state,
                PersonState::Accepted | PersonState::RequestedByThem
            )
        });
    if !talking && !state.view().pro {
        return Err(CommandError::part_of_pro(
            plenipo_licensing::words::message(Limit::CommunityStart),
        ));
    }
    let sent = state
        .community
        .send_message(&ledger, &to, &name, &text, reply_to.as_deref())
        .await;
    messages_changed(&app);
    sent.map_err(refused)
}

/// React to a message with one of the reactions, or take yours back (`emoji` none).
#[tauri::command]
pub async fn react_in_community<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    item_id: String,
    emoji: Option<String>,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let ledger = ledger(&state)?;
    let done = state
        .community
        .react(&ledger, &item_id, emoji.as_deref())
        .await;
    messages_changed(&app);
    done.map_err(refused)
}

/// **Accept** someone's first message.
#[tauri::command]
pub async fn accept_community_request<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    member_id: String,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let ledger = ledger(&state)?;
    let done = state.community.accept(&ledger, &member_id).await;
    messages_changed(&app);
    done.map_err(refused)
}

/// **Leave this conversation**: decline a request, take yours back, or stop their messages.
#[tauri::command]
pub async fn leave_community_conversation<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    member_id: String,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let ledger = ledger(&state)?;
    let done = state
        .community
        .leave_conversation(&ledger, &member_id)
        .await;
    messages_changed(&app);
    done.map_err(refused)
}

/// **Delete for me**: the message leaves this PC only (ADR-172). Nothing is sent.
#[tauri::command]
pub async fn delete_community_message<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    item_id: String,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let ledger = ledger(&state)?;
    state
        .community
        .delete_for_me(&ledger, &item_id)
        .map_err(refused)?;
    messages_changed(&app);
    Ok(())
}

/// **Check the safety code**: you compared it, so "computers changed" goes away.
#[tauri::command]
pub async fn community_safety_code_checked<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Arc<CommunityState>>,
    member_id: String,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    let ledger = ledger(&state)?;
    state
        .community
        .safety_code_checked(&ledger, &member_id)
        .map_err(refused)?;
    messages_changed(&app);
    Ok(())
}

/// A web address from a message, as Plenipo will open it: `http` or `https`, plain letters,
/// numbers, and marks only (an address in another alphabet, or with a hidden character, can be
/// copied but not opened from here), no name or password in it, and not too long. `None` for
/// anything else.
pub(crate) fn openable_link(link: &str) -> Option<String> {
    let link = link.trim();
    if link.is_empty()
        || link.len() > MOST_LINK_CHARS
        || !link.chars().all(|c| c.is_ascii_graphic())
    {
        return None;
    }
    let lower = link.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))?;
    let host_part = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host_part.is_empty() || host_part.contains('@') || host_part.contains('\\') {
        return None;
    }
    Some(link.to_owned())
}

/// **Open this link in your web browser?** — the owner said yes: open it in their own browser,
/// never Plenipo's. Only `http` and `https`.
#[tauri::command]
pub async fn open_community_link(
    state: State<'_, Arc<CommunityState>>,
    link: String,
) -> Result<(), CommandError> {
    let link = openable_link(&link).ok_or_else(|| {
        CommandError::invalid_input("Plenipo opens only web addresses that start with https://.")
    })?;
    state.opener.open(link).await.map_err(|why| {
        CommandError::invalid_input(format!("Plenipo couldn't open your web browser: {why}"))
    })
}

/// **Give to a worker** (ADR-164 §9): the message's words go to a worker you choose as an
/// objective, marked as outside words from the person who wrote them, so the worker treats them
/// as information, never as orders. `note` is what you ask the worker to do with it.
#[tauri::command]
pub async fn give_community_message_to_worker(
    state: State<'_, Arc<CommunityState>>,
    workforce: Org<'_, Workforce>,
    item_id: String,
    position_id: String,
    note: String,
) -> Result<plenipo_runtime::agent::AgentSessionDetail, CommandError> {
    crate::commands::validate_id("position", &position_id)?;
    let ledger = ledger(&state)?;
    let item = ledger
        .community_item(&item_id)
        .map_err(crate::commands::ledger_error)?
        .ok_or_else(|| CommandError::invalid_input("That message isn't on this computer."))?;
    let words = item
        .body
        .get("text")
        .and_then(|t| t.as_str())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| CommandError::invalid_input("Only a message's words can go to a worker."))?
        .to_owned();
    let person = ledger
        .community_person(&item.member_id)
        .map_err(crate::commands::ledger_error)?
        .ok_or_else(|| CommandError::invalid_input("That message isn't on this computer."))?;
    let from = if item.outgoing {
        "you".to_owned()
    } else {
        format!("@{}", person.name)
    };
    let note = note.trim();
    let ask = if note.is_empty() {
        format!("Here is a Community message from {from}. Read it and tell me what you think.")
    } else {
        note.to_owned()
    };
    let objective = format!(
        "{ask}\n\n{}",
        fence::fenced(&fence::Source::Community(from), &words)
    );
    crate::commands::validate_objective(&objective)?;
    workforce
        .give_objective(&position_id, &objective, None)
        .await
        .map_err(crate::commands::workforce_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_plain_web_address_opens() {
        for ok in [
            "https://example.com",
            "http://example.com/a?b=c#d",
            "https://sub.example.co.uk/path",
        ] {
            assert_eq!(openable_link(ok).as_deref(), Some(ok));
        }
        for bad in [
            "javascript:alert(1)",
            "file:///C:/Windows/System32/calc.exe",
            "plenipo://command",
            "https://user:pass@example.com",
            "https://example.com@evil.com/",
            "https:\\\\evil.com",
            "https://exa mple.com",
            "https://example.com/\u{202E}",
            "https://",
            "data:text/html,<script>",
            "ftp://example.com",
        ] {
            assert_eq!(openable_link(bad), None, "{bad}");
        }
        assert_eq!(
            openable_link(&format!("https://e.com/{}", "a".repeat(2048))),
            None
        );
    }
}
