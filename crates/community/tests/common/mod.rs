//! What the tests share: people on their own PCs, each with a Vault, a settings file, and a
//! Ledger in memory, against one stand-in service and one clock.

#![allow(dead_code)]

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use plenipo_community::profile::{ProfileDraft, Tile, TileStatus};
use plenipo_community::service::{Clock, Community, Recorder, Settings, Store};
use plenipo_community::stand_in::{AccountId, StandIn};
use plenipo_ledger::Ledger;

pub const START: i64 = 1_790_000_000;

/// The Vault and the settings file, in memory.
#[derive(Default)]
pub struct Memory {
    pc: Mutex<Option<String>>,
    settings: Mutex<Option<Settings>>,
}

impl Store for Memory {
    fn read_pc(&self) -> Result<Option<String>, String> {
        Ok(self.pc.lock().unwrap().clone())
    }
    fn write_pc(&self, kept: &str) -> Result<(), String> {
        *self.pc.lock().unwrap() = Some(kept.to_owned());
        Ok(())
    }
    fn erase_pc(&self) -> Result<(), String> {
        *self.pc.lock().unwrap() = None;
        Ok(())
    }
    fn read_settings(&self) -> Result<Option<Settings>, String> {
        Ok(self.settings.lock().unwrap().clone())
    }
    fn write_settings(&self, settings: &Settings) -> Result<(), String> {
        *self.settings.lock().unwrap() = Some(settings.clone());
        Ok(())
    }
}

/// The Ledger's events, in memory.
#[derive(Default)]
pub struct Events(pub Mutex<Vec<(String, serde_json::Value)>>);

impl Recorder for Events {
    fn record(&self, event: &str, payload: serde_json::Value) {
        self.0.lock().unwrap().push((event.to_owned(), payload));
    }
}

impl Events {
    pub fn names(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|(e, _)| e.clone())
            .collect()
    }
    pub fn all_text(&self) -> String {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|(e, p)| format!("{e} {p}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub struct Time(AtomicI64);

impl Clock for Time {
    fn now(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// One person's PC.
pub struct Pc {
    pub community: Community<StandIn>,
    pub events: Arc<Events>,
    pub ledger: Ledger,
    /// The 8 West account it signed in with, once it did.
    pub account: Option<AccountId>,
}

impl Pc {
    pub fn member_id(&self) -> String {
        self.community.member_id().expect("a member")
    }
}

pub struct World {
    pub service: StandIn,
    time: Arc<Time>,
}

impl World {
    pub fn new() -> Self {
        let service = StandIn::new();
        service.set_now(START);
        Self {
            service,
            time: Arc::new(Time(AtomicI64::new(START))),
        }
    }

    pub fn later(&self, secs: i64) {
        let now = self.time.0.fetch_add(secs, Ordering::SeqCst) + secs;
        self.service.set_now(now);
    }

    pub fn account(&self, name: &str, email: &str, pro: bool) -> AccountId {
        self.service.add_account(name, email, pro)
    }

    /// A PC that has not signed in.
    pub fn pc(&self) -> Pc {
        let events = Arc::new(Events::default());
        let community = Community::load(
            self.service.clone(),
            Arc::new(Memory::default()),
            events.clone(),
            self.time.clone(),
            env!("CARGO_PKG_VERSION"),
            "A-PC",
        );
        Pc {
            community,
            events,
            ledger: Ledger::open_in_memory().expect("a Ledger in memory"),
            account: None,
        }
    }

    /// Press the switch, see the code, allow it on the account site, and wait for it.
    pub async fn sign_in(&self, pc: &Pc, account: AccountId) {
        pc.community.turn_on().await.unwrap();
        let code = pc.community.view(true).code.expect("a code to type");
        assert!(self.service.allow(&code, account));
        self.later(5);
        assert_eq!(pc.community.poll().await, None, "signed in");
    }

    /// A person with an 8 West account, signed in and joined on their own PC.
    pub async fn person(&self, account: &str, name: &str, birth_year: u16, pro: bool) -> Pc {
        let id = self.account(account, &format!("{name}@example.com"), pro);
        let mut pc = self.pc();
        self.sign_in(&pc, id).await;
        pc.account = Some(id);
        let terms = pc.community.view(true).terms.expect("the terms version");
        let tile = Tile {
            status: TileStatus::Available,
            mood: None,
            message: String::new(),
            picture: None,
        };
        pc.community
            .join(name, 3, birth_year, &terms, &ProfileDraft::default(), &tile)
            .await
            .unwrap();
        pc
    }

    /// Another PC of the same person, signed in to their account.
    pub async fn another_pc(&self, of: &Pc) -> Pc {
        let account = of.account.expect("a person's PC");
        let mut pc = self.pc();
        self.sign_in(&pc, account).await;
        pc.account = Some(account);
        pc
    }
}
