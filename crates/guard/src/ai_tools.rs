//! The owner's actions on an AI tool (Phase 19, ADR-058, ADR-059): opening its sign-in or
//! sign-out tab, updating it, or putting back an earlier version. Each runs one of the tool's own
//! commands from its adapter's fixed list; Guard checks the request before anything starts, and
//! records every refusal (`guard.ai_tool_refused`).
//!
//! The rules:
//! - only an AI tool Plenipo knows, and only an action the tool has its own command for;
//! - never while a task is using the tool (signing out or updating it would break the task):
//!   Plenipo waits until the tool is free;
//! - one at a time: never an update while the tool's sign-in tab runs, and never a sign-in tab
//!   while the tool updates (both run the tool's own program);
//! - Guard never sees, and never checks, what the owner types in a sign-in tab (ADR-031 §3).

use serde_json::json;

use crate::service::{Guard, PLENIPO};

/// What the owner asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiToolAction {
    SignIn,
    SignOut,
    Update,
    PutBack,
}

impl AiToolAction {
    /// The word in the record: `signIn`, `signOut`, `update`, `putBack`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SignIn => "signIn",
            Self::SignOut => "signOut",
            Self::Update => "update",
            Self::PutBack => "putBack",
        }
    }
}

/// Something of Plenipo's own already running the tool's program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiToolBusy {
    /// Its sign-in or sign-out tab is open.
    SignInOpen,
    /// It is being updated (or its old version put back).
    Updating,
}

/// What Guard needs to know to decide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiToolRequest<'a> {
    pub runtime_id: &'a str,
    /// "Codex"; `None` when Plenipo has no AI tool by that ID.
    pub label: Option<&'a str>,
    pub action: AiToolAction,
    /// The tool has its own command for this action.
    pub has_command: bool,
    /// How many tasks are using the tool now.
    pub tasks_using: usize,
    /// Plenipo's own sign-in tab or update already running the tool, if any.
    pub busy: Option<AiToolBusy>,
}

impl Guard {
    /// Check the owner's action on an AI tool against the rules above. A refusal says why in
    /// plain words and is recorded.
    pub fn check_ai_tool_action(&self, request: &AiToolRequest<'_>) -> Result<(), String> {
        decide(request).inspect_err(|why| {
            let event = plenipo_ledger::NewEvent {
                source: PLENIPO.into(),
                event_type: "guard.ai_tool_refused".into(),
                payload: json!({
                    "runtime": request.runtime_id,
                    "action": request.action.as_str(),
                    "reason": why,
                }),
                ..plenipo_ledger::NewEvent::default()
            };
            let _ = self.ledger().append_event(event);
        })
    }
}

fn decide(request: &AiToolRequest<'_>) -> Result<(), String> {
    let Some(label) = request.label else {
        return Err(format!(
            "Plenipo has no AI tool called {:?}.",
            request.runtime_id
        ));
    };
    if !request.has_command {
        return Err(match request.action {
            AiToolAction::SignIn => format!("{label} has no sign-in command of its own."),
            AiToolAction::SignOut => format!("{label} has no sign-out command of its own."),
            AiToolAction::Update => format!("{label} has no update command of its own."),
            AiToolAction::PutBack => {
                format!("{label} has no command of its own that puts back an earlier version.")
            }
        });
    }
    match request.busy {
        Some(AiToolBusy::Updating) => {
            return Err(format!(
                "{label} is being updated. Plenipo waits until it's done."
            ))
        }
        Some(AiToolBusy::SignInOpen) => {
            return Err(format!(
                "{label}'s sign-in tab is open. Close it first; Plenipo waits until then."
            ))
        }
        None => {}
    }
    match request.tasks_using {
        0 => Ok(()),
        1 => Err(format!(
            "1 task is using {label}. Plenipo waits until it finishes."
        )),
        n => Err(format!(
            "{n} tasks are using {label}. Plenipo waits until they finish."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(
        action: AiToolAction,
        has_command: bool,
        tasks_using: usize,
    ) -> AiToolRequest<'static> {
        AiToolRequest {
            runtime_id: "codex",
            label: Some("Codex"),
            action,
            has_command,
            tasks_using,
            busy: None,
        }
    }

    #[test]
    fn only_a_known_tools_own_command_and_never_while_a_task_uses_it() {
        let ledger = std::sync::Arc::new(plenipo_ledger::Ledger::open_in_memory().unwrap());
        let guard = Guard::new(ledger.clone());
        for action in [
            AiToolAction::SignIn,
            AiToolAction::SignOut,
            AiToolAction::Update,
            AiToolAction::PutBack,
        ] {
            assert_eq!(
                guard.check_ai_tool_action(&request(action, true, 0)),
                Ok(())
            );
        }
        assert_eq!(
            guard.check_ai_tool_action(&request(AiToolAction::SignOut, true, 1)),
            Err("1 task is using Codex. Plenipo waits until it finishes.".into())
        );
        assert_eq!(
            guard.check_ai_tool_action(&request(AiToolAction::Update, true, 2)),
            Err("2 tasks are using Codex. Plenipo waits until they finish.".into())
        );
        assert_eq!(
            guard.check_ai_tool_action(&request(AiToolAction::SignOut, false, 0)),
            Err("Codex has no sign-out command of its own.".into())
        );
        // One at a time: an update never starts while the sign-in tab runs, and the reverse.
        assert_eq!(
            guard.check_ai_tool_action(&AiToolRequest {
                busy: Some(AiToolBusy::SignInOpen),
                ..request(AiToolAction::Update, true, 0)
            }),
            Err("Codex's sign-in tab is open. Close it first; Plenipo waits until then.".into())
        );
        assert_eq!(
            guard.check_ai_tool_action(&AiToolRequest {
                busy: Some(AiToolBusy::Updating),
                ..request(AiToolAction::SignOut, true, 0)
            }),
            Err("Codex is being updated. Plenipo waits until it's done.".into())
        );
        let unknown = AiToolRequest {
            runtime_id: "calc",
            label: None,
            ..request(AiToolAction::SignIn, true, 0)
        };
        assert!(guard.check_ai_tool_action(&unknown).is_err());
        let refused = ledger
            .events_of_types(&["guard.ai_tool_refused"], 10)
            .unwrap();
        // Every refusal is recorded: 3 above, the 2 while busy, and the unknown tool.
        assert_eq!(refused.len(), 6);
        assert_eq!(refused[0].payload["runtime"], "calc");
    }
}
