//! Requests from the owner's phone or another device (Phase 14, ADR-145): the fixed list, and
//! Guard's decision on each one.
//!
//! A phone reaches Plenipo through 8 West's relay, sealed end to end (ADR-143), and may ask for
//! the kinds in [`RequestKind`] and nothing else: there is no "any command" kind, so anything not
//! on the list cannot even be written down. Guard decides each request in a fixed order
//! ([`decide`]): phone access is switched on; Pro; the phone is paired and not paused; the phone
//! is signed in (or the request is one a notice may send by itself); and an approval is not one
//! the owner keeps on the PC. Then the PC calls the same core function the main window uses, which
//! applies all of its own rules too.
//!
//! What stays on the PC is simply not here: the terminal and any shell, files, the screen and
//! Plenipo's browser, secrets, and anything that widens what workers may do or who may connect.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::dto::SensitiveKind;
use crate::servers::Environment;

/// Every kind of request a phone may make (ADR-145 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RequestKind {
    // ---- Reading (part 14A) ----
    /// The organizations on the PC, and which one the phone looks at.
    ReadOrganizations,
    /// Home: what needs you, who is working, what is stuck, what just finished.
    ReadHome,
    /// The approvals: waiting, and answered lately.
    ReadApprovals,
    /// The organization: departments, positions, and who holds them.
    ReadOrganization,
    /// The projects, and one project's page.
    ReadProjects,
    /// The workers, and one worker's page.
    ReadWorkers,
    /// The tasks, and one task with its conversation.
    ReadTasks,
    /// The Activity trail.
    ReadActivity,
    /// The AI tools: signed in or not, versions, and usage.
    ReadAiTools,
    /// Diagnostics: the Ledger's health, the last start and close.
    ReadDiagnostics,
    /// Whether everything is stopped, and what an unexpected stop left.
    ReadControl,
    /// Lessons waiting for the owner.
    ReadLessons,
    // ---- Answering and stopping (part 14A) ----
    /// Approve a waiting approval.
    Approve,
    /// Refuse a waiting approval.
    Refuse,
    /// Stop all: every worker on the PC stops.
    StopAll,
    /// End this phone's sign-in.
    SignOut,
    /// Take this phone off the PC's list.
    RemoveThisPhone,
    // ---- Everything else that is safe from the page (part 14B) ----
    /// Allow again, after Stop all.
    AllowAgain,
    /// Stop one worker's task.
    StopTask,
    /// Run a task again after an unexpected stop.
    RunAgain,
    /// Leave a task stopped after an unexpected stop.
    LeaveStopped,
    /// Keep a lesson, as written.
    KeepLesson,
    /// Discard a lesson.
    DiscardLesson,
    /// Give a position an objective (text only).
    SendObjective,
    // ---- Notices (part 14C) ----
    /// Send this phone notices when the page is closed.
    NoticesOn,
    /// Stop sending this phone notices.
    NoticesOff,
}

impl RequestKind {
    pub const ALL: [Self; 26] = [
        Self::ReadOrganizations,
        Self::ReadHome,
        Self::ReadApprovals,
        Self::ReadOrganization,
        Self::ReadProjects,
        Self::ReadWorkers,
        Self::ReadTasks,
        Self::ReadActivity,
        Self::ReadAiTools,
        Self::ReadDiagnostics,
        Self::ReadControl,
        Self::ReadLessons,
        Self::Approve,
        Self::Refuse,
        Self::StopAll,
        Self::SignOut,
        Self::RemoveThisPhone,
        Self::AllowAgain,
        Self::StopTask,
        Self::RunAgain,
        Self::LeaveStopped,
        Self::KeepLesson,
        Self::DiscardLesson,
        Self::SendObjective,
        Self::NoticesOn,
        Self::NoticesOff,
    ];

    /// What it is, in plain words, as Activity says it ("approve", "read Home").
    pub fn label(self) -> &'static str {
        match self {
            Self::ReadOrganizations => "see the list of organizations",
            Self::ReadHome => "read Home",
            Self::ReadApprovals => "read Approvals",
            Self::ReadOrganization => "read the organization",
            Self::ReadProjects => "read the projects",
            Self::ReadWorkers => "read the workers",
            Self::ReadTasks => "read the tasks",
            Self::ReadActivity => "read Activity",
            Self::ReadAiTools => "read the AI tools",
            Self::ReadDiagnostics => "read Diagnostics",
            Self::ReadControl => "see whether everything is stopped",
            Self::ReadLessons => "read the lessons waiting",
            Self::Approve => "approve",
            Self::Refuse => "refuse",
            Self::StopAll => "stop all",
            Self::SignOut => "sign out",
            Self::RemoveThisPhone => "remove this phone",
            Self::AllowAgain => "allow again",
            Self::StopTask => "stop a task",
            Self::RunAgain => "run a task again",
            Self::LeaveStopped => "leave a task stopped",
            Self::KeepLesson => "keep a lesson",
            Self::DiscardLesson => "discard a lesson",
            Self::SendObjective => "send an objective",
            Self::NoticesOn => "turn notices on",
            Self::NoticesOff => "turn notices off",
        }
    }

    /// Reading only: it changes nothing.
    pub fn reads(self) -> bool {
        matches!(
            self,
            Self::ReadOrganizations
                | Self::ReadHome
                | Self::ReadApprovals
                | Self::ReadOrganization
                | Self::ReadProjects
                | Self::ReadWorkers
                | Self::ReadTasks
                | Self::ReadActivity
                | Self::ReadAiTools
                | Self::ReadDiagnostics
                | Self::ReadControl
                | Self::ReadLessons
        )
    }

    /// May a notice's button send it with only the phone's own key, before any sign-in
    /// (ADR-142 §5)? Only the ones that say no: they can never let anything happen.
    pub fn from_a_notice(self) -> bool {
        matches!(self, Self::Refuse | Self::DiscardLesson)
    }

    /// It answers an approval, so the approvals kept on the PC apply.
    pub fn answers_an_approval(self) -> bool {
        matches!(self, Self::Approve | Self::Refuse)
    }
}

/// The phone, as the PC knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhoneState {
    /// Paired, and not paused.
    Paired,
    /// Paired, but paused after too many failed checks (ADR-142 §6).
    Paused,
    /// The PC does not know this phone (never paired, or removed).
    Unknown,
}

/// What an approval is, for the approvals kept on the PC (ADR-145 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ApprovalFacts {
    /// The kind of sensitive action it is, if it is one.
    pub sensitive: Option<SensitiveKind>,
    /// The kind of server it is for, if it is a server's command.
    pub environment: Option<Environment>,
}

/// The approvals the owner keeps on the PC only (Settings → Devices → **Keep these approvals on
/// my PC only**): none ticked to begin with (the plan, ADR-145 §5).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[ts(export)]
pub struct KeptOnPc {
    /// Every approval.
    pub every: bool,
    /// Commands on a server marked Production.
    pub production_servers: bool,
    /// These kinds of sensitive action.
    pub kinds: Vec<SensitiveKind>,
}

impl KeptOnPc {
    /// Is an approval like this kept on the PC?
    pub fn keeps(&self, facts: ApprovalFacts) -> bool {
        self.every
            || (self.production_servers && facts.environment == Some(Environment::Production))
            || facts.sensitive.is_some_and(|k| self.kinds.contains(&k))
    }

    /// The same list, each kind once, in the registry's order.
    pub fn tidy(mut self) -> Self {
        self.kinds = SensitiveKind::ALL
            .into_iter()
            .filter(|k| self.kinds.contains(k))
            .collect();
        self
    }
}

/// Everything Guard needs to decide one request.
#[derive(Debug, Clone, Copy)]
pub struct RemoteCheck<'a> {
    /// Settings → Switches → **Use Plenipo from another device**.
    pub switched_on: bool,
    /// Phone access is Pro (`Entitlements::check(Limit::PhoneAccess)`, ADR-145 §1).
    pub pro: bool,
    pub phone: PhoneState,
    /// The phone is signed in now (ADR-142 §3).
    pub signed_in: bool,
    /// Sent by a notice's button, with only the phone's own key (ADR-142 §5).
    pub from_notice: bool,
    pub kind: RequestKind,
    /// For an answer to an approval: what the approval is.
    pub approval: Option<ApprovalFacts>,
    pub kept: &'a KeptOnPc,
}

/// Why Guard refused, in one plain sentence (shown on the phone, and recorded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub why: Why,
    pub message: String,
}

/// Which of Guard's checks refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Why {
    SwitchedOff,
    NotPro,
    UnknownPhone,
    Paused,
    NotSignedIn,
    NotFromANotice,
    KeptOnPc,
    NotAnApproval,
}

impl Refusal {
    /// Guard's refusal for `why`, in its plain sentence.
    pub fn new(why: Why) -> Self {
        let message = match why {
            Why::SwitchedOff => {
                "Using Plenipo from another device is switched off on your PC (Settings → \
                 Switches)."
            }
            Why::NotPro => "Using Plenipo from your phone is part of Pro.",
            Why::UnknownPhone => {
                "Your PC doesn't know this phone. Add it on your PC: Settings → Devices → Add a \
                 phone."
            }
            Why::Paused => {
                "This phone is paused after too many failed checks. Un-pause it on your PC: \
                 Settings → Devices."
            }
            Why::NotSignedIn => "Sign in on this phone first.",
            Why::NotFromANotice => "Open Plenipo on your phone to do that.",
            Why::KeptOnPc => "Approve this on your PC. You keep this kind of approval on your PC.",
            Why::NotAnApproval => "That is not an approval waiting for you.",
        };
        Self {
            why,
            message: message.to_owned(),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// Guard's decision on one request from a phone, in the order ADR-145 §4 sets.
pub fn decide(check: &RemoteCheck<'_>) -> Result<(), Refusal> {
    if !check.switched_on {
        return Err(Refusal::new(Why::SwitchedOff));
    }
    if !check.pro {
        return Err(Refusal::new(Why::NotPro));
    }
    match check.phone {
        PhoneState::Paired => {}
        PhoneState::Paused => return Err(Refusal::new(Why::Paused)),
        PhoneState::Unknown => return Err(Refusal::new(Why::UnknownPhone)),
    }
    if check.from_notice {
        if !check.kind.from_a_notice() {
            return Err(Refusal::new(Why::NotFromANotice));
        }
    } else if !check.signed_in {
        return Err(Refusal::new(Why::NotSignedIn));
    }
    if check.kind.answers_an_approval() {
        let Some(facts) = check.approval else {
            return Err(Refusal::new(Why::NotAnApproval));
        };
        if check.kept.keeps(facts) {
            return Err(Refusal::new(Why::KeptOnPc));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(kind: RequestKind) -> RemoteCheck<'static> {
        static NONE: KeptOnPc = KeptOnPc {
            every: false,
            production_servers: false,
            kinds: Vec::new(),
        };
        RemoteCheck {
            switched_on: true,
            pro: true,
            phone: PhoneState::Paired,
            signed_in: true,
            from_notice: false,
            kind,
            approval: kind
                .answers_an_approval()
                .then_some(ApprovalFacts::default()),
            kept: &NONE,
        }
    }

    fn why(check: &RemoteCheck<'_>) -> Option<Why> {
        decide(check).err().map(|r| r.why)
    }

    #[test]
    fn a_signed_in_phone_may_ask_for_everything_on_the_list() {
        for kind in RequestKind::ALL {
            assert_eq!(why(&allowed(kind)), None, "{kind:?}");
        }
    }

    #[test]
    fn guard_checks_in_the_order_the_record_sets() {
        // Everything wrong at once: the first check in the order is the one that refuses.
        let kept = KeptOnPc {
            every: true,
            ..KeptOnPc::default()
        };
        let mut c = RemoteCheck {
            switched_on: false,
            pro: false,
            phone: PhoneState::Unknown,
            signed_in: false,
            from_notice: false,
            kind: RequestKind::Approve,
            approval: Some(ApprovalFacts::default()),
            kept: &kept,
        };
        assert_eq!(why(&c), Some(Why::SwitchedOff));
        c.switched_on = true;
        assert_eq!(why(&c), Some(Why::NotPro));
        c.pro = true;
        assert_eq!(why(&c), Some(Why::UnknownPhone));
        c.phone = PhoneState::Paused;
        assert_eq!(why(&c), Some(Why::Paused));
        c.phone = PhoneState::Paired;
        assert_eq!(why(&c), Some(Why::NotSignedIn));
        c.signed_in = true;
        assert_eq!(why(&c), Some(Why::KeptOnPc));
        let none = KeptOnPc::default();
        c.kept = &none;
        assert_eq!(why(&c), None);
    }

    #[test]
    fn a_notice_may_only_say_no() {
        for kind in RequestKind::ALL {
            let mut c = allowed(kind);
            c.signed_in = false;
            c.from_notice = true;
            let expected = if kind.from_a_notice() {
                None
            } else {
                Some(Why::NotFromANotice)
            };
            assert_eq!(why(&c), expected, "{kind:?}");
        }
        assert!(RequestKind::Refuse.from_a_notice());
        assert!(RequestKind::DiscardLesson.from_a_notice());
        assert!(!RequestKind::Approve.from_a_notice());
        assert!(!RequestKind::AllowAgain.from_a_notice());
        assert!(!RequestKind::KeepLesson.from_a_notice());
    }

    #[test]
    fn nothing_but_signing_in_is_allowed_before_signing_in() {
        for kind in RequestKind::ALL {
            let mut c = allowed(kind);
            c.signed_in = false;
            assert_eq!(why(&c), Some(Why::NotSignedIn), "{kind:?}");
        }
    }

    #[test]
    fn approvals_kept_on_the_pc_cannot_be_answered_from_a_phone() {
        let production = ApprovalFacts {
            sensitive: None,
            environment: Some(Environment::Production),
        };
        let payment = ApprovalFacts {
            sensitive: Some(SensitiveKind::Payment),
            environment: None,
        };
        let plain = ApprovalFacts::default();
        let kept = KeptOnPc {
            every: false,
            production_servers: true,
            kinds: vec![SensitiveKind::Payment],
        };
        for kind in [RequestKind::Approve, RequestKind::Refuse] {
            for (facts, keeps) in [(production, true), (payment, true), (plain, false)] {
                let mut c = allowed(kind);
                c.kept = &kept;
                c.approval = Some(facts);
                let expected = keeps.then_some(Why::KeptOnPc);
                assert_eq!(why(&c), expected, "{kind:?} {facts:?}");
            }
        }
        // Not an approval at all.
        let mut c = allowed(RequestKind::Approve);
        c.approval = None;
        assert_eq!(why(&c), Some(Why::NotAnApproval));
    }

    #[test]
    fn none_is_kept_on_the_pc_to_begin_with() {
        let kept = KeptOnPc::default();
        for kind in SensitiveKind::ALL {
            let facts = ApprovalFacts {
                sensitive: Some(kind),
                environment: Some(Environment::Production),
            };
            assert!(!kept.keeps(facts), "{kind:?}");
        }
        let every = KeptOnPc {
            every: true,
            ..KeptOnPc::default()
        };
        assert!(every.keeps(ApprovalFacts::default()));
    }

    #[test]
    fn the_kept_list_reads_and_tidies() {
        let kept: KeptOnPc = serde_json::from_str(
            r#"{"every":false,"productionServers":true,"kinds":["payment","dns","payment"]}"#,
        )
        .unwrap();
        let kept = kept.tidy();
        assert_eq!(kept.kinds, [SensitiveKind::Dns, SensitiveKind::Payment]);
        assert!(serde_json::from_str::<KeptOnPc>(r#"{"other":true}"#).is_err());
        assert_eq!(serde_json::from_str::<KeptOnPc>("{}").unwrap(), KeptOnPc::default());
    }

    #[test]
    fn every_refusal_is_one_plain_sentence() {
        for why in [
            Why::SwitchedOff,
            Why::NotPro,
            Why::UnknownPhone,
            Why::Paused,
            Why::NotSignedIn,
            Why::NotFromANotice,
            Why::KeptOnPc,
            Why::NotAnApproval,
        ] {
            let m = Refusal::new(why).message;
            assert!(m.ends_with('.'), "{m}");
            let lower = m.to_lowercase();
            for word in ["denied", "forbidden", "unauthorized", "error", "runtime"] {
                assert!(!lower.contains(word), "{m}");
            }
        }
    }

    #[test]
    fn every_kind_has_words() {
        let mut labels: Vec<&str> = RequestKind::ALL.iter().map(|k| k.label()).collect();
        let n = labels.len();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), n, "two kinds share words");
        assert!(RequestKind::ReadHome.reads());
        assert!(!RequestKind::Approve.reads());
    }
}
