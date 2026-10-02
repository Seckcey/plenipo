//! What the phone and the PC say inside the sealed line (ADR-145). Each sealed message is one
//! JSON object with a `t` field.
//!
//! The phone may **ask** for the kinds in [`Ask`], and nothing else: there is no "any command"
//! kind. Each ask carries a random ID; the PC answers with a [`Reply`] naming it, refuses an ID
//! it has already seen in that sign-in, and keeps each answer for that time so a phone whose
//! connection dropped can ask what happened ([`Ask::Outcome`]).

use plenipo_guard::remote::{RequestKind, Why};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::webauthn::{NewPasskey, PasskeyAnswer};

/// The longest ID accepted in a request (an organization's, an approval's, a task's).
pub const MAX_ID: usize = 128;
/// The longest objective text accepted from a phone (the PC's own limit applies too).
pub const MAX_TEXT: usize = 64 * 1024;
/// The longest device name.
pub const MAX_NAME: usize = 40;

/// The third message of a first meeting carries this: what the phone calls itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PairHello {
    /// The name the owner gave the phone ("Frank's iPhone").
    pub name: String,
    /// What it is ("Safari on iPhone").
    pub browser: String,
}

/// The first message of an everyday meeting carries this.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
#[ts(export)]
pub struct MeetingHello {
    /// Sent by a notice's button (part 14C): it may only say no (ADR-142 §5).
    pub notice: bool,
    /// The page's version.
    pub page: String,
}

/// The PC's answer to an everyday meeting (its second message).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MeetingWelcome {
    /// The phone is still signed in (within 30 minutes of its last request, 12 hours at most).
    pub signed_in: bool,
    /// When not: the challenge for its passkey (base64url).
    #[ts(optional)]
    pub challenge: Option<String>,
    /// The PC's name.
    pub pc_name: String,
    /// Plenipo's version on the PC.
    pub version: String,
    /// The PC's notice key (P-256, base64url), for the phone to sign up for notices (part 14C). A
    /// PC that sends no notices (before 1.19.2) has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub notice_key: Option<String>,
}

/// A notice, as the phone opens it (part 14C, ADR-144 §3). It is sealed for that phone alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PhoneNotice {
    /// Its version: 1.
    pub v: u8,
    /// What kind, as Settings → Notifications names them: `approvals`, `checks`, `problems`,
    /// `finished`, `lessons`, `plenipo`, or `spending`.
    pub kind: String,
    /// The organization it is from.
    pub org: String,
    pub title: String,
    pub body: String,
    /// The one thing it is about, when it can be answered from the notice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub about: Option<NoticeAbout>,
    /// One per thing: a notice sent again shows once.
    pub tag: String,
    /// When the PC sent it (Unix milliseconds).
    #[ts(type = "number")]
    pub at: u64,
}

/// What a notice is about, by its ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export, rename = "PhoneNoticeAbout")]
pub enum NoticeAbout {
    Approval {
        id: String,
    },
    Lesson {
        id: String,
    },
    /// Stop all stopped the PC's browser, desktop, and server work: **Allow again** opens Plenipo.
    Stopped,
}

/// A phone's request, by kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export, rename = "PhoneAsk")]
pub enum Ask {
    /// Sign in with the passkey's answer to the meeting's challenge.
    SignIn {
        answer: PasskeyAnswer,
    },
    /// What happened to an earlier request (by its ID), after a dropped connection.
    Outcome {
        of: String,
    },
    // ---- Reading (part 14A) ----
    ReadOrganizations,
    ReadHome {
        org: String,
    },
    /// One organization's approvals, or every organization's.
    ReadApprovals {
        #[ts(optional)]
        org: Option<String>,
    },
    ReadOrganization {
        org: String,
    },
    /// The projects, or one project.
    ReadProjects {
        org: String,
        #[ts(optional)]
        project: Option<String>,
    },
    /// The workers, or one position's worker.
    ReadWorkers {
        org: String,
        #[ts(optional)]
        position: Option<String>,
    },
    /// The tasks, one task, or one conversation.
    ReadTasks {
        org: String,
        #[ts(optional)]
        task: Option<String>,
        #[ts(optional)]
        conversation: Option<String>,
    },
    /// The Activity trail, newest first; `before` pages back.
    ReadActivity {
        org: String,
        #[ts(optional, type = "number")]
        before: Option<i64>,
    },
    ReadAiTools,
    ReadDiagnostics,
    ReadControl,
    ReadLessons {
        org: String,
    },
    // ---- Answering and stopping (part 14A) ----
    Approve {
        org: String,
        approval: String,
    },
    Refuse {
        org: String,
        approval: String,
    },
    StopAll,
    SignOut,
    RemoveThisPhone,
    // ---- Everything else that is safe from the page (part 14B) ----
    AllowAgain,
    StopTask {
        org: String,
        conversation: String,
    },
    RunAgain {
        org: String,
        task: String,
    },
    /// Leave the work that stopped when Plenipo did stopped: one organization's notice.
    LeaveStopped {
        org: String,
        notice: String,
    },
    KeepLesson {
        org: String,
        lesson: String,
    },
    DiscardLesson {
        org: String,
        lesson: String,
    },
    SendObjective {
        org: String,
        position: String,
        #[ts(optional)]
        project: Option<String>,
        text: String,
    },
    // ---- Notices (part 14C) ----
    NoticesOn {
        subscription: Subscription,
    },
    NoticesOff,
}

/// Where the phone's notice service takes its notices (the browser's push subscription).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename = "NoticeSubscription")]
pub struct Subscription {
    pub endpoint: String,
    /// The phone's notice key (P-256, base64url).
    pub p256dh: String,
    /// The phone's notice secret (16 bytes, base64url).
    pub auth: String,
}

impl Ask {
    /// The kind Guard decides (`None`: signing in, and asking about an earlier answer, which are
    /// the sign-in itself and the phone's own answer).
    pub fn kind(&self) -> Option<RequestKind> {
        Some(match self {
            Self::SignIn { .. } | Self::Outcome { .. } => return None,
            Self::ReadOrganizations => RequestKind::ReadOrganizations,
            Self::ReadHome { .. } => RequestKind::ReadHome,
            Self::ReadApprovals { .. } => RequestKind::ReadApprovals,
            Self::ReadOrganization { .. } => RequestKind::ReadOrganization,
            Self::ReadProjects { .. } => RequestKind::ReadProjects,
            Self::ReadWorkers { .. } => RequestKind::ReadWorkers,
            Self::ReadTasks { .. } => RequestKind::ReadTasks,
            Self::ReadActivity { .. } => RequestKind::ReadActivity,
            Self::ReadAiTools => RequestKind::ReadAiTools,
            Self::ReadDiagnostics => RequestKind::ReadDiagnostics,
            Self::ReadControl => RequestKind::ReadControl,
            Self::ReadLessons { .. } => RequestKind::ReadLessons,
            Self::Approve { .. } => RequestKind::Approve,
            Self::Refuse { .. } => RequestKind::Refuse,
            Self::StopAll => RequestKind::StopAll,
            Self::SignOut => RequestKind::SignOut,
            Self::RemoveThisPhone => RequestKind::RemoveThisPhone,
            Self::AllowAgain => RequestKind::AllowAgain,
            Self::StopTask { .. } => RequestKind::StopTask,
            Self::RunAgain { .. } => RequestKind::RunAgain,
            Self::LeaveStopped { .. } => RequestKind::LeaveStopped,
            Self::KeepLesson { .. } => RequestKind::KeepLesson,
            Self::DiscardLesson { .. } => RequestKind::DiscardLesson,
            Self::SendObjective { .. } => RequestKind::SendObjective,
            Self::NoticesOn { .. } => RequestKind::NoticesOn,
            Self::NoticesOff => RequestKind::NoticesOff,
        })
    }

    /// The organization it concerns, if one.
    pub fn org(&self) -> Option<&str> {
        match self {
            Self::ReadHome { org }
            | Self::ReadOrganization { org }
            | Self::ReadProjects { org, .. }
            | Self::ReadWorkers { org, .. }
            | Self::ReadTasks { org, .. }
            | Self::ReadActivity { org, .. }
            | Self::ReadLessons { org }
            | Self::Approve { org, .. }
            | Self::Refuse { org, .. }
            | Self::StopTask { org, .. }
            | Self::RunAgain { org, .. }
            | Self::LeaveStopped { org, .. }
            | Self::KeepLesson { org, .. }
            | Self::DiscardLesson { org, .. }
            | Self::SendObjective { org, .. } => Some(org),
            Self::ReadApprovals { org } => org.as_deref(),
            _ => None,
        }
    }

    /// The ID of what it acts on (an approval, a task, a lesson), for the record.
    pub fn target(&self) -> Option<&str> {
        match self {
            Self::Approve { approval, .. } | Self::Refuse { approval, .. } => Some(approval),
            Self::StopTask { conversation, .. } => Some(conversation),
            Self::RunAgain { task, .. } => Some(task),
            Self::LeaveStopped { notice, .. } => Some(notice),
            Self::KeepLesson { lesson, .. } | Self::DiscardLesson { lesson, .. } => Some(lesson),
            Self::SendObjective { position, .. } => Some(position),
            Self::ReadProjects { project, .. } => project.as_deref(),
            Self::ReadWorkers { position, .. } => position.as_deref(),
            Self::ReadTasks {
                task, conversation, ..
            } => task.as_deref().or(conversation.as_deref()),
            _ => None,
        }
    }

    /// Sizes and shapes the PC accepts at all (before Guard).
    pub fn sane(&self) -> bool {
        let id = |s: &str| {
            !s.is_empty()
                && s.len() <= MAX_ID
                && s.bytes().all(|b| b.is_ascii_graphic() && b != b'"')
        };
        let ids_ok = self.org().is_none_or(id) && self.target().is_none_or(id);
        let rest_ok = match self {
            Self::Outcome { of } => crate::b64::is_id(of, 16),
            Self::SendObjective { text, .. } => !text.trim().is_empty() && text.len() <= MAX_TEXT,
            // Only the address's shape here: where notices may go is Guard's to say, when the
            // phone signs up (`Host::notice_address`) and again for every notice.
            Self::NoticesOn { subscription } => {
                !subscription.endpoint.is_empty()
                    && subscription.endpoint.len() <= 1024
                    && subscription.endpoint.bytes().all(|b| b.is_ascii_graphic())
                    && crate::b64::decode(&subscription.p256dh, 65).is_some_and(|k| k.len() == 65)
                    && crate::b64::decode(&subscription.auth, 16).is_some_and(|k| k.len() == 16)
            }
            _ => true,
        };
        ids_ok && rest_ok
    }
}

/// The phone to the PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "t",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export, rename = "PhoneSays")]
pub enum PhoneSays {
    /// A request. `again`: the same page read again after a change (not recorded again).
    Ask {
        id: String,
        #[serde(default)]
        again: bool,
        ask: Ask,
    },
    /// Pairing: the passkey the phone just made.
    Passkey { passkey: NewPasskey },
}

/// Guard's refusal, as the phone sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename = "PhoneRefused")]
pub struct Refused {
    /// Which check refused (`null`: not one of Guard's, for example a copied request).
    #[ts(optional)]
    pub why: Option<Why>,
    /// One plain sentence.
    pub message: String,
}

/// The PC's answer to one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename = "PhoneReply")]
pub struct Reply {
    /// The request's ID.
    pub re: String,
    /// What the request gave (its shape depends on its kind).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    pub ok: Option<serde_json::Value>,
    /// Guard refused it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub refused: Option<Refused>,
    /// It was allowed, but did not work (in plain words).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub failed: Option<String>,
}

impl Reply {
    pub fn ok(re: &str, value: serde_json::Value) -> Self {
        Self {
            re: re.to_owned(),
            ok: Some(value),
            refused: None,
            failed: None,
        }
    }

    pub fn refused(re: &str, why: Option<Why>, message: impl Into<String>) -> Self {
        Self {
            re: re.to_owned(),
            ok: None,
            refused: Some(Refused {
                why,
                message: message.into(),
            }),
            failed: None,
        }
    }

    pub fn failed(re: &str, message: impl Into<String>) -> Self {
        Self {
            re: re.to_owned(),
            ok: None,
            refused: None,
            failed: Some(message.into()),
        }
    }
}

/// What changed on the PC, so the phone reads that page again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "PhoneChanged")]
pub enum Changed {
    Approvals,
    Home,
    Control,
    Lessons,
    Tasks,
    Activity,
    Organizations,
    AiTools,
}

/// Why the phone was signed out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SignedOutWhy {
    /// The phone's own **Sign out**.
    You,
    /// 30 minutes with no request.
    Idle,
    /// 12 hours since signing in.
    TwelveHours,
    /// Removed on the PC (or on the phone).
    Removed,
    /// Phone access switched off on the PC.
    SwitchedOff,
    /// Pro ended: phone access pauses.
    ProEnded,
    /// Paused after too many failed checks.
    Paused,
}

impl SignedOutWhy {
    pub fn word(self) -> &'static str {
        match self {
            Self::You => "you",
            Self::Idle => "idle",
            Self::TwelveHours => "twelve_hours",
            Self::Removed => "removed",
            Self::SwitchedOff => "switched_off",
            Self::ProEnded => "pro_ended",
            Self::Paused => "paused",
        }
    }
}

/// Something the PC tells the phone without being asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export, rename = "PhoneEvent")]
pub enum Event {
    /// Something changed on the PC: read that page again.
    Changed {
        #[ts(optional)]
        org: Option<String>,
        what: Changed,
    },
    /// The phone is signed out.
    SignedOut { why: SignedOutWhy },
}

/// What the PC asks the phone's passkey to be made for (WebAuthn's `create()` options).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct PasskeyRequest {
    /// base64url.
    pub challenge: String,
    /// The passkeys' site (`remote.getplenipo.com`).
    pub rp_id: String,
    /// The user the passkey is for (base64url), and its name.
    pub user: String,
    pub user_name: String,
}

/// A step of pairing, from the PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "step",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export)]
pub enum PairStep {
    /// The PC is asking the owner "Is this your phone?".
    Waiting,
    /// The owner said no, or did not answer in time.
    Refused { message: String },
    /// The owner said yes: what the phone keeps, and its passkey to make.
    Accepted {
        /// The phone's ID on the PC.
        device: String,
        /// The phone's ID at the relay, and its pass.
        phone: String,
        pass: String,
        /// The PC: its relay key's fingerprint, and its name.
        pc: String,
        pc_name: String,
        passkey: PasskeyRequest,
    },
    /// Paired.
    Done,
    /// The passkey did not check.
    Failed { message: String },
}

/// The PC to the phone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "t",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
#[ts(export)]
pub enum PcSays {
    Reply { reply: Reply },
    Event { event: Event },
    Pair { pair: PairStep },
}

/// What a successful sign-in gives the phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SignedIn {
    /// A fresh relay pass (ADR-147).
    pub pass: String,
    /// When the sign-in ends at the latest (Unix milliseconds).
    #[ts(type = "number")]
    pub ends_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_reads_only_as_the_list_says() {
        let m: PhoneSays = serde_json::from_str(
            r#"{"t":"ask","id":"AAAAAAAAAAAAAAAAAAAAAA","ask":{"kind":"approve","org":"first","approval":"a1"}}"#,
        )
        .unwrap();
        let PhoneSays::Ask { id, again, ask } = m else {
            panic!()
        };
        assert_eq!(id, "AAAAAAAAAAAAAAAAAAAAAA");
        assert!(!again);
        assert_eq!(ask.kind(), Some(RequestKind::Approve));
        assert_eq!(ask.org(), Some("first"));
        assert_eq!(ask.target(), Some("a1"));
        assert!(ask.sane());
        // Not on the list, or with more than the list allows.
        for bad in [
            r#"{"t":"ask","id":"x","ask":{"kind":"runProgram","command":"rm"}}"#,
            r#"{"t":"ask","id":"x","ask":{"kind":"approve","org":"first","approval":"a1","also":"x"}}"#,
            r#"{"t":"ask","id":"x","ask":{"kind":"setSwitches"}}"#,
            r#"{"t":"ask","id":"x","ask":{"kind":"addDevice"}}"#,
            r#"{"t":"shell","command":"rm -rf /"}"#,
        ] {
            assert!(serde_json::from_str::<PhoneSays>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn every_kind_on_the_list_has_an_ask() {
        // Guard's list and the phone's asks are the same list.
        let asks = [
            Ask::ReadOrganizations,
            Ask::ReadHome { org: "o".into() },
            Ask::ReadApprovals { org: None },
            Ask::ReadOrganization { org: "o".into() },
            Ask::ReadProjects {
                org: "o".into(),
                project: None,
            },
            Ask::ReadWorkers {
                org: "o".into(),
                position: None,
            },
            Ask::ReadTasks {
                org: "o".into(),
                task: None,
                conversation: None,
            },
            Ask::ReadActivity {
                org: "o".into(),
                before: None,
            },
            Ask::ReadAiTools,
            Ask::ReadDiagnostics,
            Ask::ReadControl,
            Ask::ReadLessons { org: "o".into() },
            Ask::Approve {
                org: "o".into(),
                approval: "a".into(),
            },
            Ask::Refuse {
                org: "o".into(),
                approval: "a".into(),
            },
            Ask::StopAll,
            Ask::SignOut,
            Ask::RemoveThisPhone,
            Ask::AllowAgain,
            Ask::StopTask {
                org: "o".into(),
                conversation: "c".into(),
            },
            Ask::RunAgain {
                org: "o".into(),
                task: "t".into(),
            },
            Ask::LeaveStopped {
                org: "o".into(),
                notice: "n".into(),
            },
            Ask::KeepLesson {
                org: "o".into(),
                lesson: "l".into(),
            },
            Ask::DiscardLesson {
                org: "o".into(),
                lesson: "l".into(),
            },
            Ask::SendObjective {
                org: "o".into(),
                position: "p".into(),
                project: None,
                text: "Make the site faster.".into(),
            },
            Ask::NoticesOn {
                subscription: Subscription {
                    endpoint: "https://fcm.googleapis.com/fcm/send/x".into(),
                    p256dh: crate::b64::encode(&[4u8; 65]),
                    auth: crate::b64::encode(&[1u8; 16]),
                },
            },
            Ask::NoticesOff,
        ];
        let kinds: Vec<RequestKind> = asks.iter().filter_map(Ask::kind).collect();
        assert_eq!(kinds, RequestKind::ALL);
        for ask in &asks {
            assert!(ask.sane(), "{ask:?}");
            let text = serde_json::to_string(ask).unwrap();
            assert_eq!(&serde_json::from_str::<Ask>(&text).unwrap(), ask);
        }
    }

    #[test]
    fn odd_sizes_and_shapes_are_not_sane() {
        let long = "x".repeat(MAX_ID + 1);
        for ask in [
            Ask::ReadHome { org: long.clone() },
            Ask::ReadHome { org: String::new() },
            Ask::Approve {
                org: "o".into(),
                approval: "a b".into(),
            },
            Ask::SendObjective {
                org: "o".into(),
                position: "p".into(),
                project: None,
                text: "   ".into(),
            },
            Ask::SendObjective {
                org: "o".into(),
                position: "p".into(),
                project: None,
                text: "x".repeat(MAX_TEXT + 1),
            },
            Ask::Outcome {
                of: "not-an-id".into(),
            },
            Ask::NoticesOn {
                subscription: Subscription {
                    endpoint: "https://fcm.googleapis.com/a b".into(),
                    p256dh: crate::b64::encode(&[4u8; 65]),
                    auth: crate::b64::encode(&[1u8; 16]),
                },
            },
        ] {
            assert!(!ask.sane(), "{ask:?}");
        }
    }

    #[test]
    fn replies_events_and_pairing_steps_read_back() {
        for m in [
            PcSays::Reply {
                reply: Reply::ok("r", serde_json::json!({"a":1})),
            },
            PcSays::Reply {
                reply: Reply::refused("r", Some(Why::KeptOnPc), "Approve this on your PC."),
            },
            PcSays::Event {
                event: Event::Changed {
                    org: Some("first".into()),
                    what: Changed::Approvals,
                },
            },
            PcSays::Event {
                event: Event::SignedOut {
                    why: SignedOutWhy::Idle,
                },
            },
            PcSays::Pair {
                pair: PairStep::Waiting,
            },
        ] {
            let text = serde_json::to_string(&m).unwrap();
            assert_eq!(serde_json::from_str::<PcSays>(&text).unwrap(), m, "{text}");
        }
    }
}
