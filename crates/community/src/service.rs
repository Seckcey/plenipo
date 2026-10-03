//! Community on this PC (ADR-162, ADR-170): the switch, **Coming soon**, signing in with a code,
//! joining, signing out, and leaving.
//!
//! The app gives it a [`Transport`] (Guard's check, then HTTPS), a [`Store`] (the Vault for the
//! signed-in PC, a small file for the switch), a [`Clock`], and a [`Recorder`] (the Ledger).
//!
//! **Nothing here contacts 8 West by itself** (ADR-115, ADR-162 §5, ADR-170 §2): only pressing the
//! switch, **Check again**, or **Sign in** starts a request, and once this PC is signed in the app
//! may ask who it is ([`Community::refresh`]). A copy whose owner never turns Community on never
//! sends anything.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use crate::client::{self, Answer, ErrorCode, Failure, Request, Transport};
use crate::profile::{self, ProfileDraft, Tile};
use crate::session::{self, AgeCheck, Finish, Opening, SignedInPc, SigningIn};
use crate::wire;

/// Where Plenipo keeps Community on this PC.
pub trait Store: Send + Sync {
    /// The signed-in PC, as the Vault keeps it ([`SignedInPc::write`]).
    fn read_pc(&self) -> Result<Option<String>, String>;
    fn write_pc(&self, kept: &str) -> Result<(), String>;
    fn erase_pc(&self) -> Result<(), String>;
    /// The switch, and what this PC remembers; not secret.
    fn read_settings(&self) -> Result<Option<Settings>, String>;
    fn write_settings(&self, settings: &Settings) -> Result<(), String>;
}

/// What the Ledger records about Community: which event, and facts that are never a pass, a key,
/// a code, a birth date, or anyone's words.
pub trait Recorder: Send + Sync {
    fn record(&self, event: &str, payload: serde_json::Value);
}

/// The time, in Unix seconds.
pub trait Clock: Send + Sync {
    fn now(&self) -> i64;
}

/// The switch, and what this PC remembers about Community. Not secret.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    /// The person turned Community on, and this PC joined it.
    pub switched_on: bool,
    /// The last check found Community not open yet: the switch says **Coming soon** until a check
    /// finds it open (ADR-170 §3).
    pub coming_soon: bool,
    /// What you add to your tile, and which parts are shown (ADR-163 §2).
    #[serde(default)]
    pub profile: Option<ProfileDraft>,
    /// The picture last sent, as a SHA-256 in hex, so the same one is not sent again.
    #[serde(default)]
    pub picture_sent: Option<String>,
}

/// Where Community stands on this PC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Stage {
    /// The switch is off.
    Off,
    /// 8 West's service says Community is not open yet: **Coming soon**.
    ComingSoon,
    /// This version of Plenipo may not use Community: **Update Plenipo to use Community**.
    UpdateNeeded,
    /// No answer, or the service is busy. Nothing was changed.
    Unreachable,
    /// Showing the code, waiting for **Allow** on the account site.
    SigningIn,
    /// Signed in, but not a member yet: the age box, the name, and the terms.
    Joining,
    /// Signed in, and a member.
    SignedIn,
    /// The switch is on, but this PC is not signed in (signed out, or removed on the account
    /// site): **Sign in**.
    SignedOut,
    /// Signed in, but 8 West closed Community for now (ADR-170 §4). Everything here is kept.
    Closed,
}

/// The member, as Settings → Community shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemberView {
    /// The Community name, without the `@`.
    pub name: String,
    /// 18 or older.
    pub adult: bool,
    /// May start things: an adult, with Pro, in good standing (contract §3).
    pub can_start: bool,
    /// `ok`, `paused`, or `ended`.
    pub standing: String,
    #[ts(type = "number | null")]
    pub paused_until: Option<i64>,
    pub appear_offline: bool,
    /// Parts of the profile 8 West hid (ADR-167 §9): they stay hidden until 8 West shows them.
    pub hidden_parts: Vec<String>,
}

/// Settings → Community, and the switch, as the screen shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommunityView {
    pub stage: Stage,
    /// The switch: on once this PC joined, and while it is signed in or signed out.
    pub switched_on: bool,
    /// The last check found Community not open yet.
    pub coming_soon: bool,
    /// "Enter this code: 4KQ-7TD", while signing in.
    pub code: Option<String>,
    /// When the code runs out (Unix seconds).
    #[ts(type = "number | null")]
    pub code_expires_at: Option<i64>,
    /// "Signed in as Frank Gonzalez".
    pub account_name: Option<String>,
    /// The terms version to accept when joining, as the service names it.
    pub terms: Option<String>,
    pub member: Option<MemberView>,
    /// This PC is on Pro: starting things is part of Pro (ADR-162 §5).
    pub pro: bool,
    /// Linked organizations and collaborators are open yet (ADR-171).
    pub links_open: bool,
    pub collaborators_open: bool,
    /// What went wrong last, in plain words, or none.
    pub problem: Option<String>,
    /// Your profile's parts and boxes, for **What people see** (ADR-163 §1, §2).
    pub profile: ProfileDraft,
}

/// Why something the person asked for did not happen, in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

struct State {
    settings: Settings,
    stage: Stage,
    signing_in: Option<SigningIn>,
    pc: Option<Arc<SignedInPc>>,
    me: Option<wire::Me>,
    open: Option<wire::Open>,
    problem: Option<String>,
    /// A step under way, so two presses never run at once.
    busy: bool,
}

/// Community on this PC.
pub struct Community<T: Transport> {
    transport: T,
    store: Arc<dyn Store>,
    recorder: Arc<dyn Recorder>,
    clock: Arc<dyn Clock>,
    app_version: String,
    device_name: String,
    state: Mutex<State>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

const NOT_REACHED: &str = "Community can't be reached right now. Nothing was changed.";
const TOO_YOUNG: &str = "Community is for people 13 and older.";

/// Plain words for a failure the person sees.
pub(crate) fn words(failure: &Failure) -> String {
    match failure {
        Failure::Service(e) if !e.message.trim().is_empty() => e.message.clone(),
        Failure::NotSignedIn => "Sign in to Community first.".into(),
        _ => NOT_REACHED.into(),
    }
}

impl<T: Transport> Community<T> {
    /// Community as this PC left it: the switch and the signed-in PC, read back. Sends nothing.
    pub fn load(
        transport: T,
        store: Arc<dyn Store>,
        recorder: Arc<dyn Recorder>,
        clock: Arc<dyn Clock>,
        app_version: &str,
        device_name: &str,
    ) -> Self {
        let settings = store.read_settings().ok().flatten().unwrap_or_default();
        let pc = store
            .read_pc()
            .ok()
            .flatten()
            .and_then(|kept| SignedInPc::read(&kept).ok())
            .map(Arc::new);
        let stage = match (settings.switched_on, &pc) {
            (true, Some(_)) => Stage::SignedIn,
            (true, None) => Stage::SignedOut,
            (false, _) if settings.coming_soon => Stage::ComingSoon,
            (false, _) => Stage::Off,
        };
        Self {
            transport,
            store,
            recorder,
            clock,
            app_version: app_version.to_owned(),
            device_name: device_name.to_owned(),
            state: Mutex::new(State {
                settings,
                stage,
                signing_in: None,
                pc,
                me: None,
                open: None,
                problem: None,
                busy: false,
            }),
        }
    }

    /// Settings → Community, as the screen shows it. `pro`: this PC is on Pro.
    pub fn view(&self, pro: bool) -> CommunityView {
        let s = lock(&self.state);
        let member =
            s.me.as_ref()
                .and_then(|me| me.member.as_ref())
                .map(|m| MemberView {
                    name: m.name.clone(),
                    adult: m.age_group == wire::AgeGroup::Adult,
                    can_start: m.can_start,
                    standing: match m.standing {
                        wire::Standing::Ok => "ok",
                        wire::Standing::Paused => "paused",
                        wire::Standing::Ended => "ended",
                    }
                    .into(),
                    paused_until: m.paused_until,
                    appear_offline: m.appear_offline,
                    hidden_parts: m
                        .hidden_parts
                        .iter()
                        .filter_map(|p| serde_json::to_value(p).ok())
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect(),
                });
        let profile = s.settings.profile.clone().unwrap_or_else(|| ProfileDraft {
            display_name: s
                .me
                .as_ref()
                .map(|me| me.account.name.clone())
                .unwrap_or_default(),
            ..ProfileDraft::default()
        });
        CommunityView {
            stage: s.stage,
            switched_on: s.settings.switched_on,
            coming_soon: s.settings.coming_soon,
            code: s.signing_in.as_ref().map(|si| si.user_code.clone()),
            code_expires_at: s.signing_in.as_ref().map(|si| si.expires_at),
            account_name: s.me.as_ref().map(|me| me.account.name.clone()),
            terms: s.me.as_ref().map(|me| me.terms.clone()),
            member,
            pro,
            links_open: s.open.as_ref().is_some_and(|o| o.links),
            collaborators_open: s.open.as_ref().is_some_and(|o| o.collaborators),
            problem: s.problem.clone(),
            profile,
        }
    }

    /// This PC signed in, when it is: for the app's other Community work (picking up, sending).
    pub fn signed_in_pc(&self) -> Option<Arc<SignedInPc>> {
        let s = lock(&self.state);
        matches!(s.stage, Stage::SignedIn | Stage::Joining | Stage::Closed)
            .then(|| s.pc.clone())
            .flatten()
    }

    /// The transport, for the app's other Community work.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Send one request as this member, for the app's other Community work (people, messages,
    /// rewards): only while signed in and joined. The answer comes back when its status is `ok`.
    /// An answer that says this PC's sign-in ended signs it out here; one that says Community
    /// closed, or that Plenipo must be updated, shows on Settings → Community.
    pub async fn as_member(&self, request: &Request, ok: u16) -> Result<Answer, Failure> {
        let pc = {
            let s = lock(&self.state);
            (s.stage == Stage::SignedIn).then(|| s.pc.clone()).flatten()
        };
        let Some(pc) = pc else {
            return Err(Failure::NotSignedIn);
        };
        let result = client::send(&self.transport, request, Some(pc.pass()))
            .await
            .and_then(|answer| client::check(answer, ok));
        if let Err(failure) = &result {
            match failure.code() {
                Some(ErrorCode::Unauthorized) => self.forget_here("removed"),
                Some(ErrorCode::NotOpen) => lock(&self.state).stage = Stage::Closed,
                Some(ErrorCode::UpdateNeeded) => lock(&self.state).stage = Stage::UpdateNeeded,
                _ => {}
            }
        }
        result
    }

    /// The Ledger's shared record, for the app's other Community work.
    pub(crate) fn record(&self, event: &str, payload: serde_json::Value) {
        self.recorder.record(event, payload);
    }

    fn save_settings(&self, s: &mut State) {
        if let Err(why) = self.store.write_settings(&s.settings) {
            log::warn!("Community's settings could not be saved: {why}");
        }
    }

    /// Start a step, unless one is under way.
    fn begin(&self) -> Result<(), Refused> {
        let mut s = lock(&self.state);
        if s.busy {
            return Err(Refused("Wait a moment: Community is still busy.".into()));
        }
        s.busy = true;
        s.problem = None;
        Ok(())
    }

    fn end(&self) {
        lock(&self.state).busy = false;
    }

    /// The person pressed **Community** (or **Check again**, or **Sign in**): ask whether
    /// Community is open, and if it is, start signing in (ADR-170 §2, §3).
    pub async fn turn_on(&self) -> Result<(), Refused> {
        self.begin()?;
        let result = self.turn_on_now().await;
        self.end();
        result
    }

    async fn turn_on_now(&self) -> Result<(), Refused> {
        let opening = session::check_open(&self.transport).await;
        let open = match opening {
            Opening::Open(open) => open,
            Opening::NotOpen => {
                let mut s = lock(&self.state);
                s.settings.coming_soon = true;
                if !s.settings.switched_on {
                    s.stage = Stage::ComingSoon;
                }
                self.save_settings(&mut s);
                return Ok(());
            }
            Opening::UpdateNeeded => {
                lock(&self.state).stage = Stage::UpdateNeeded;
                return Ok(());
            }
            Opening::Unreachable => {
                let mut s = lock(&self.state);
                s.problem = Some(NOT_REACHED.into());
                if !s.settings.switched_on {
                    s.stage = Stage::Unreachable;
                }
                return Ok(());
            }
        };
        {
            let mut s = lock(&self.state);
            s.open = Some(open);
            s.settings.coming_soon = false;
            self.save_settings(&mut s);
            if s.pc.is_some() {
                // Already signed in: nothing to start.
                return Ok(());
            }
        }
        let now = self.clock.now();
        match session::start(&self.transport, &self.device_name, &self.app_version, now).await {
            Ok(signing_in) => {
                let mut s = lock(&self.state);
                s.signing_in = Some(signing_in);
                s.stage = Stage::SigningIn;
                Ok(())
            }
            Err(failure) => {
                let mut s = lock(&self.state);
                s.stage = if s.settings.switched_on {
                    Stage::SignedOut
                } else {
                    Stage::Off
                };
                s.problem = Some(words(&failure));
                Err(Refused(words(&failure)))
            }
        }
    }

    /// While signing in: ask whether the person pressed **Allow** yet. Returns how many seconds
    /// to wait before asking again, or none when signing in is over (signed in, refused, or run
    /// out).
    pub async fn poll(&self) -> Option<u32> {
        let signing_in = {
            let mut s = lock(&self.state);
            if s.busy || s.stage != Stage::SigningIn {
                return None;
            }
            let signing_in = s.signing_in.take()?;
            if self.clock.now() >= signing_in.expires_at {
                s.stage = if s.settings.switched_on {
                    Stage::SignedOut
                } else {
                    Stage::Off
                };
                s.problem = Some("The code ran out. Press Community to try again.".into());
                return None;
            }
            s.busy = true;
            signing_in
        };
        let result = session::finish(&self.transport, signing_in).await;
        let mut s = lock(&self.state);
        s.busy = false;
        if s.stage != Stage::SigningIn {
            // Cancelled while asking.
            return None;
        }
        let back = |s: &mut State, problem: &str| {
            s.stage = if s.settings.switched_on {
                Stage::SignedOut
            } else {
                Stage::Off
            };
            s.problem = Some(problem.into());
        };
        match result {
            Ok((Finish::Waiting, Some(signing_in))) => {
                let wait = signing_in.interval;
                s.signing_in = Some(signing_in);
                Some(wait)
            }
            Ok((Finish::SlowDown, Some(mut signing_in))) => {
                signing_in.interval += 5;
                let wait = signing_in.interval;
                s.signing_in = Some(signing_in);
                Some(wait)
            }
            Ok((Finish::Denied, _)) => {
                back(&mut s, "The sign-in was not allowed. Nothing was kept.");
                None
            }
            Ok((Finish::Expired, _)) => {
                back(&mut s, "The code ran out. Press Community to try again.");
                None
            }
            Ok((Finish::SignedIn(pc, me), _)) => {
                if let Err(why) = self.store.write_pc(&pc.write()) {
                    log::warn!("Community's sign-in could not be kept in the Vault: {why}");
                    back(
                        &mut s,
                        "Plenipo couldn't keep the sign-in in the Vault. Try again.",
                    );
                    return None;
                }
                self.recorder.record(
                    "community.signed_in",
                    json!({ "account": me.account.name, "pc": self.device_name }),
                );
                let joined = me.member.is_some();
                s.pc = Some(Arc::new(*pc));
                s.me = Some(*me);
                s.stage = if joined {
                    Stage::SignedIn
                } else {
                    Stage::Joining
                };
                if joined {
                    s.settings.switched_on = true;
                    self.save_settings(&mut s);
                }
                None
            }
            // Waiting or slow down without the sign-in: never, but not a sign-in either.
            Ok((_, None)) => {
                back(&mut s, NOT_REACHED);
                None
            }
            Err(Failure::Unreachable(_)) | Err(Failure::BadAnswer) => {
                // No answer this time: keep the code until it runs out. It cannot be used again
                // only once it has worked.
                back(&mut s, NOT_REACHED);
                None
            }
            Err(failure) => {
                back(&mut s, &words(&failure));
                None
            }
        }
    }

    /// Stop signing in: nothing was kept.
    pub fn cancel_sign_in(&self) {
        let mut s = lock(&self.state);
        if s.stage == Stage::SigningIn {
            s.signing_in = None;
            s.stage = if s.settings.switched_on {
                Stage::SignedOut
            } else {
                Stage::Off
            };
        }
    }

    /// Join Community, after the age box (ADR-162 §4): under 13, nothing is sent, nothing of the
    /// answer is kept, and this PC signs out again. `month` and `year` are the birth month and
    /// year; `terms` is the version the person accepted on screen.
    pub async fn join(
        &self,
        name: &str,
        month: u8,
        year: u16,
        terms: &str,
        draft: &ProfileDraft,
        tile: &Tile,
    ) -> Result<(), Refused> {
        // The profile is checked first, so a join never leaves a profile that can't be sent.
        profile::profile_of(tile, draft)?;
        let pc = {
            let s = lock(&self.state);
            if s.stage != Stage::Joining {
                return Err(Refused("Sign in to your 8 West account first.".into()));
            }
            s.pc.clone()
        };
        let Some(pc) = pc else {
            return Err(Refused("Sign in to your 8 West account first.".into()));
        };
        let (today_month, today_year) = session::month_and_year(self.clock.now());
        match session::check_age(month, year, today_month, today_year) {
            AgeCheck::NotADate => {
                return Err(Refused("Choose your birth month and year.".into()));
            }
            AgeCheck::TooYoung => {
                // Nothing of the answer is sent or kept. This PC signs out again.
                self.forget(&pc, "too_young").await;
                let mut s = lock(&self.state);
                s.problem = Some(TOO_YOUNG.into());
                return Err(Refused(TOO_YOUNG.into()));
            }
            AgeCheck::OldEnough => {}
        }
        self.begin()?;
        let body = wire::Join {
            name: name.trim().to_owned(),
            birth_month: month,
            birth_year: year,
            terms: terms.to_owned(),
        };
        let result = session::join(&self.transport, &pc, &body).await;
        self.end();
        match result {
            Ok(me) => {
                let name = me
                    .member
                    .as_ref()
                    .map(|m| m.name.clone())
                    .unwrap_or_default();
                self.recorder
                    .record("community.joined", json!({ "name": name }));
                {
                    let mut s = lock(&self.state);
                    s.me = Some(me);
                    s.stage = Stage::SignedIn;
                    s.settings.switched_on = true;
                    s.settings.coming_soon = false;
                    self.save_settings(&mut s);
                }
                // Then the profile, as What people see showed it (ADR-163 §1).
                if let Err(refused) = self.save_profile(draft, tile).await {
                    lock(&self.state).problem = Some(refused.0);
                }
                Ok(())
            }
            Err(failure) if failure.code() == Some(&ErrorCode::TooYoung) => {
                self.forget(&pc, "too_young").await;
                lock(&self.state).problem = Some(TOO_YOUNG.into());
                Err(Refused(TOO_YOUNG.into()))
            }
            Err(failure) => Err(Refused(words(&failure))),
        }
    }

    /// **Save** your profile: the parts shown, from your tile and what you added (ADR-163 §2).
    /// Unticked parts are hidden at 8 West at once.
    pub async fn save_profile(&self, draft: &ProfileDraft, tile: &Tile) -> Result<(), Refused> {
        let profile = profile::profile_of(tile, draft)?;
        let pc = {
            let s = lock(&self.state);
            if s.stage != Stage::SignedIn {
                return Err(Refused("Join Community first.".into()));
            }
            s.pc.clone()
        };
        let Some(pc) = pc else {
            return Err(Refused("Join Community first.".into()));
        };
        self.begin()?;
        let result = self.send_profile(&pc, &profile, draft, tile).await;
        self.end();
        {
            let mut s = lock(&self.state);
            s.settings.profile = Some(draft.clone());
            self.save_settings(&mut s);
        }
        result.map_err(|failure| Refused(words(&failure)))?;
        let shown = draft.shown;
        let parts: Vec<&str> = [
            ("picture", shown.picture),
            ("name", shown.name),
            ("status", shown.status),
            ("mood", shown.mood),
            ("message", shown.message),
            ("company", shown.company),
            ("business", shown.business),
            ("region", shown.region),
        ]
        .into_iter()
        .filter_map(|(part, on)| on.then_some(part))
        .collect();
        self.recorder
            .record("community.profile_changed", json!({ "shown": parts }));
        Ok(())
    }

    /// Your tile changed: send it again, unless you appear offline (ADR-163 §8). Nothing is sent
    /// before you have saved a profile, or while you are not signed in.
    pub async fn tile_changed(&self, tile: &Tile) {
        let (pc, draft) = {
            let s = lock(&self.state);
            let offline =
                s.me.as_ref()
                    .and_then(|me| me.member.as_ref())
                    .is_none_or(|m| m.appear_offline);
            if s.stage != Stage::SignedIn || offline || s.busy {
                return;
            }
            (s.pc.clone(), s.settings.profile.clone())
        };
        let (Some(pc), Some(draft)) = (pc, draft) else {
            return;
        };
        let Ok(profile) = profile::profile_of(tile, &draft) else {
            return;
        };
        if let Err(failure) = self.send_profile(&pc, &profile, &draft, tile).await {
            log::info!("Community: your tile could not be sent ({failure:?})");
        }
    }

    /// Send the profile, then the picture if it changed (or remove it when it is not shown).
    async fn send_profile(
        &self,
        pc: &SignedInPc,
        profile: &wire::Profile,
        draft: &ProfileDraft,
        tile: &Tile,
    ) -> Result<(), Failure> {
        let answer = client::send(
            &self.transport,
            &client::set_profile(profile),
            Some(pc.pass()),
        )
        .await?;
        let me: wire::Me = client::read(answer, 200)?;
        let hidden_by_8_west = me
            .member
            .as_ref()
            .is_some_and(|m| m.hidden_parts.contains(&wire::HiddenPart::Picture));
        let has_picture = me.member.as_ref().is_some_and(|m| m.has_picture);
        lock(&self.state).me = Some(me);

        let picture = tile.picture.as_ref().filter(|_| draft.shown.picture);
        let sent = lock(&self.state).settings.picture_sent.clone();
        match picture {
            Some(png) if !hidden_by_8_west => {
                use sha2::{Digest as _, Sha256};
                let hash: String = Sha256::digest(png)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                if sent.as_deref() != Some(hash.as_str()) || !has_picture {
                    let upload = wire::PictureUpload {
                        png: crate::b64::encode(png),
                    };
                    let answer = client::send(
                        &self.transport,
                        &client::set_picture(&upload),
                        Some(pc.pass()),
                    )
                    .await?;
                    let me: wire::Me = client::read(answer, 200)?;
                    let mut s = lock(&self.state);
                    s.me = Some(me);
                    s.settings.picture_sent = Some(hash);
                    self.save_settings(&mut s);
                }
            }
            _ => {
                if has_picture || sent.is_some() {
                    let answer =
                        client::send(&self.transport, &client::remove_picture(), Some(pc.pass()))
                            .await?;
                    let me: wire::Me = client::read(answer, 200)?;
                    let mut s = lock(&self.state);
                    s.me = Some(me);
                    s.settings.picture_sent = None;
                    self.save_settings(&mut s);
                }
            }
        }
        Ok(())
    }

    /// **Appear offline**, or not (ADR-163 §5): out of the directory, New this week, and the
    /// leaderboard at once, and people you know see you as Offline.
    pub async fn set_appear_offline(&self, offline: bool) -> Result<(), Refused> {
        let pc = {
            let s = lock(&self.state);
            if s.stage != Stage::SignedIn {
                return Err(Refused("Join Community first.".into()));
            }
            s.pc.clone()
        };
        let Some(pc) = pc else {
            return Err(Refused("Join Community first.".into()));
        };
        self.begin()?;
        let body = wire::Presence {
            appear_offline: offline,
        };
        let result = async {
            let answer = client::send(
                &self.transport,
                &client::set_presence(&body),
                Some(pc.pass()),
            )
            .await?;
            client::read::<wire::Me>(answer, 200)
        }
        .await;
        self.end();
        let me = result.map_err(|failure| Refused(words(&failure)))?;
        lock(&self.state).me = Some(me);
        self.recorder.record(
            if offline {
                "community.appeared_offline"
            } else {
                "community.appeared_online"
            },
            json!({}),
        );
        Ok(())
    }

    /// Ask who this PC is now (contract §3): only while signed in. A pass that no longer works
    /// (signed out, or removed on the account site) is forgotten; a closed Community is kept.
    pub async fn refresh(&self) {
        let Some(pc) = lock(&self.state).pc.clone() else {
            return;
        };
        match session::me(&self.transport, &pc).await {
            Ok(me) => {
                let mut s = lock(&self.state);
                if s.stage == Stage::SigningIn {
                    return;
                }
                let joined = me.member.is_some();
                s.me = Some(me);
                s.stage = if joined {
                    Stage::SignedIn
                } else {
                    Stage::Joining
                };
            }
            Err(failure) => match failure.code() {
                Some(ErrorCode::Unauthorized) => {
                    self.forget_here("removed");
                }
                Some(ErrorCode::NotOpen) => {
                    lock(&self.state).stage = Stage::Closed;
                }
                Some(ErrorCode::UpdateNeeded) => {
                    lock(&self.state).stage = Stage::UpdateNeeded;
                }
                _ => {
                    lock(&self.state).problem = Some(NOT_REACHED.into());
                }
            },
        }
    }

    /// **Sign out of your account** on this PC (ADR-162 §2). Membership stays; the switch stays
    /// on, and says **Sign in**.
    pub async fn sign_out(&self) -> Result<(), Refused> {
        let Some(pc) = lock(&self.state).pc.clone() else {
            return Ok(());
        };
        self.begin()?;
        let result = session::sign_out(&self.transport, &pc).await;
        self.end();
        self.forget_here("you");
        match result {
            Ok(()) => Ok(()),
            Err(_) => {
                // This PC forgot its pass anyway; 8 West still lists it until it is removed.
                let words = "Signed out here. 8 West couldn't be reached to end this sign-in: \
                             remove it on the account site to end it there too.";
                lock(&self.state).problem = Some(words.into());
                Ok(())
            }
        }
    }

    /// **Leave Community** (ADR-167 §15): turning the switch off. 8 West deletes your profile,
    /// your listing, and what still waits there, and signs every PC of yours out. What is on this
    /// PC stays until you delete it.
    pub async fn leave(&self) -> Result<(), Refused> {
        let (pc, joined) = {
            let s = lock(&self.state);
            (
                s.pc.clone(),
                s.me.as_ref().is_some_and(|me| me.member.is_some()) || s.settings.switched_on,
            )
        };
        let Some(pc) = pc else {
            // Not signed in here: just turn the switch off.
            let mut s = lock(&self.state);
            s.settings.switched_on = false;
            s.signing_in = None;
            s.stage = Stage::Off;
            self.save_settings(&mut s);
            return Ok(());
        };
        if !joined {
            // Signed in but never joined: signing out is all there is.
            self.forget(&pc, "you").await;
            return Ok(());
        }
        self.begin()?;
        let result = session::leave(&self.transport, &pc).await;
        self.end();
        match result {
            Ok(()) => {
                self.recorder.record("community.left", json!({}));
                self.forget_here("left");
                Ok(())
            }
            Err(failure) => Err(Refused(words(&failure))),
        }
    }

    /// Sign this PC out at 8 West (best effort), then forget it here.
    async fn forget(&self, pc: &SignedInPc, why: &str) {
        let _ = session::sign_out(&self.transport, pc).await;
        self.forget_here(why);
    }

    /// Forget the signed-in PC here: the Vault entry goes, and the switch is off unless the
    /// person only signed out.
    fn forget_here(&self, why: &str) {
        if let Err(why) = self.store.erase_pc() {
            log::warn!("Community's sign-in could not be removed from the Vault: {why}");
        }
        let mut s = lock(&self.state);
        let was_signed_in = s.pc.take().is_some();
        s.me = None;
        s.signing_in = None;
        let keep_switch = why == "you" || why == "removed";
        s.settings.switched_on = keep_switch && s.settings.switched_on;
        s.stage = if s.settings.switched_on {
            Stage::SignedOut
        } else {
            Stage::Off
        };
        self.save_settings(&mut s);
        drop(s);
        if was_signed_in && why != "left" {
            self.recorder
                .record("community.signed_out", json!({ "why": why }));
        }
    }
}
