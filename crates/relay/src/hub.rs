//! What the relay knows while it runs: the PCs connected (by fingerprint), their phones (by
//! connection), each PC's pairing mailbox and dropped passes, and the counts behind the limits.
//! Everything is in memory, under one lock, and gone when a connection ends. Nothing is written
//! anywhere.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use plenipo_relay_contract::wire::{self, codes, RelayToPc, RelayToPhone};
use sha2::{Digest as _, Sha256};
use tokio::sync::mpsc::Sender;
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::Message;

use crate::limits::{AddressKey, Limits, Window};
use crate::{Config, Stats};

pub(crate) type ConnId = String;

/// How the hub reaches one connection: its outgoing queue, and a way to end it at once when the
/// queue is full (a peer that does not read), by messages or by bytes.
#[derive(Clone)]
pub(crate) struct Link {
    pub out: Sender<Message>,
    pub kill: Arc<Notify>,
    /// Bytes queued and not yet written to the socket (the writer takes them off).
    pub queued: Arc<AtomicUsize>,
    /// The most `queued` may hold: over it, the peer is killed instead.
    pub most_queued: usize,
}

impl Link {
    pub fn send_message(&self, m: Message) {
        let len = m.len();
        let before = self.queued.fetch_add(len, Ordering::SeqCst);
        if before.saturating_add(len) > self.most_queued || self.out.try_send(m).is_err() {
            // The message is dropped and the count stays as it is: this connection is ending,
            // and nothing reads the count once it has.
            self.kill.notify_one();
        }
    }

    pub fn send(&self, text: String) {
        self.send_message(Message::text(text));
    }

    pub fn close(&self) {
        self.send_message(Message::Close(None));
    }
}

struct Mailbox {
    name: String,
    opened: Instant,
    tries: u32,
}

struct Pc {
    link: Link,
    key: [u8; 32],
    mailbox: Option<Mailbox>,
    /// Phone ID → refused until (Unix seconds). Forgotten with the connection.
    dropped: HashMap<String, i64>,
    phones: HashSet<ConnId>,
    /// Which connection this is: a replaced connection must not undo its replacement.
    generation: u64,
    /// Its license's mark ([`Hub::license_mark`]), to count the PCs on one license.
    license: [u8; 32],
    address: AddressKey,
}

struct Phone {
    link: Link,
    pc: String,
    phone: Option<String>,
}

#[derive(Default)]
struct AddressUse {
    open: usize,
    /// PCs connected from this address.
    pcs: usize,
    new: Window,
    tries: Window,
}

#[derive(Default)]
struct State {
    pcs: HashMap<String, Pc>,
    phones: HashMap<ConnId, Phone>,
    addresses: HashMap<AddressKey, AddressUse>,
    /// License mark → PCs connected on it. An entry goes when its count does.
    licenses: HashMap<[u8; 32], usize>,
    connections: usize,
    next_conn: u64,
    next_generation: u64,
    off: bool,
    refused: BTreeMap<String, u64>,
    turned_away: u64,
    #[cfg(feature = "test-hooks")]
    refused_in_order: Vec<String>,
    #[cfg(feature = "test-hooks")]
    seen: Vec<Vec<u8>>,
}

/// Why the door turned a connection away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Turned {
    Off,
    Full,
    TooMany,
}

pub(crate) struct Hub {
    pub config: Config,
    state: Mutex<State>,
    /// Random, made when the relay starts, known to nothing else: what makes a license mark
    /// impossible to turn back into a key ID, or to match against another relay's or another
    /// run's.
    salt: [u8; 32],
}

fn lock(m: &Mutex<State>) -> MutexGuard<'_, State> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Hub {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: Mutex::new(State::default()),
            salt: crate::random32(),
        }
    }

    pub fn now(&self) -> i64 {
        (self.config.clock)()
    }

    /// A license's mark: a one-way hash of its key ID with this run's salt, so the relay can
    /// count the PCs on one license while it keeps no key ID (the contract). Never logged.
    pub fn license_mark(&self, key_id: &str) -> [u8; 32] {
        Sha256::new()
            .chain_update(self.salt)
            .chain_update(key_id.as_bytes())
            .finalize()
            .into()
    }

    pub fn stats(&self) -> Stats {
        let st = lock(&self.state);
        Stats {
            connections: st.connections,
            pcs: st.pcs.len(),
            phones: st.phones.len(),
            addresses: st.addresses.len(),
            refused: st.refused.clone(),
            turned_away: st.turned_away,
            off: st.off,
        }
    }

    pub fn is_off(&self) -> bool {
        lock(&self.state).off
    }

    pub fn set_off(&self, off: bool) {
        let was = {
            let mut st = lock(&self.state);
            std::mem::replace(&mut st.off, off)
        };
        if off && !was {
            self.close_all();
        }
    }

    /// Close every connection now: phones hear `pc_offline` first.
    pub fn close_all(&self) {
        let st = lock(&self.state);
        for phone in st.phones.values() {
            phone.link.send(wire::write(&RelayToPhone::PcOffline));
            phone.link.close();
        }
        for pc in st.pcs.values() {
            pc.link.close();
        }
    }

    // ---- The door ----------------------------------------------------------------------

    /// A new connection from `address` asks in. `Ok`: it is counted; call [`Hub::leave`] when it
    /// ends.
    pub fn enter(&self, address: AddressKey) -> Result<(), Turned> {
        let now = self.now();
        let limits = &self.config.limits;
        let mut st = lock(&self.state);
        let turned = if st.off {
            Some(Turned::Off)
        } else if st.connections >= limits.connections
            || Self::no_room_for(&mut st, address, limits, now)
        {
            Some(Turned::Full)
        } else {
            let use_ = st.addresses.entry(address).or_default();
            if use_.open >= limits.connections_per_address
                || use_.tries.count(now) >= u64::from(limits.tries_per_address_per_minute)
                || use_.new.bump(now, 1) > u64::from(limits.new_per_address_per_minute)
            {
                Some(Turned::TooMany)
            } else {
                None
            }
        };
        if let Some(t) = turned {
            st.turned_away += 1;
            Self::forget_idle_address(&mut st, address, now);
            return Err(t);
        }
        st.connections += 1;
        st.addresses.entry(address).or_default().open += 1;
        Ok(())
    }

    /// A counted connection ended.
    pub fn leave(&self, address: AddressKey) {
        let now = self.now();
        let mut st = lock(&self.state);
        st.connections = st.connections.saturating_sub(1);
        if let Some(use_) = st.addresses.get_mut(&address) {
            use_.open = use_.open.saturating_sub(1);
        }
        Self::forget_idle_address(&mut st, address, now);
    }

    /// Forget every idle address. The relay's timer calls this ([`crate::Relay::start`]), so an
    /// address that came once and never again takes no memory past its minute; the door calls
    /// it too when the table is full.
    pub fn sweep_addresses(&self) {
        let now = self.now();
        let mut st = lock(&self.state);
        Self::sweep(&mut st, now);
    }

    fn sweep(st: &mut State, now: i64) {
        st.addresses.retain(|_, u| !Self::idle(u, now));
    }

    /// Nothing open, no PC, and nothing counted this minute.
    fn idle(u: &AddressUse, now: i64) -> bool {
        u.open == 0 && u.pcs == 0 && u.new.count(now) == 0 && u.tries.count(now) == 0
    }

    /// Is there no room in the table for a new address? At the cap, the idle ones are forgotten
    /// at once; if it is still full, the ones that are only counted this minute go too (their
    /// counts start over), since the table is first of all for addresses with something open:
    /// first those under both their limits, and only if still full those over one (an address
    /// the door is turning away keeps its brake as long as it can). Only a table full of
    /// addresses with something open turns a new one away, and they are bounded by the
    /// connections in all.
    fn no_room_for(st: &mut State, address: AddressKey, limits: &Limits, now: i64) -> bool {
        let most = limits.addresses_remembered;
        if st.addresses.contains_key(&address) || st.addresses.len() < most {
            return false;
        }
        Self::sweep(st, now);
        if st.addresses.len() >= most {
            st.addresses
                .retain(|_, u| u.open > 0 || u.pcs > 0 || Self::braked(u, limits, now));
        }
        if st.addresses.len() >= most {
            st.addresses.retain(|_, u| u.open > 0 || u.pcs > 0);
        }
        st.addresses.len() >= most
    }

    /// Is the door turning this address away this minute (over its refusals or its new
    /// connections)?
    fn braked(u: &AddressUse, limits: &Limits, now: i64) -> bool {
        u.tries.count(now) >= u64::from(limits.tries_per_address_per_minute)
            || u.new.count(now) >= u64::from(limits.new_per_address_per_minute)
    }

    /// An idle address takes no memory once its connection or refusal is done with.
    fn forget_idle_address(st: &mut State, address: AddressKey, now: i64) {
        if st
            .addresses
            .get(&address)
            .is_some_and(|u| Self::idle(u, now))
        {
            st.addresses.remove(&address);
        }
    }

    /// Count a refusal, by code, against `address`.
    pub fn refused(&self, code: &str, address: AddressKey) {
        let now = self.now();
        let mut st = lock(&self.state);
        *st.refused.entry(code.to_owned()).or_default() += 1;
        st.addresses.entry(address).or_default().tries.bump(now, 1);
        #[cfg(feature = "test-hooks")]
        st.refused_in_order.push(code.to_owned());
        log::info!("refused {code} ({address})");
    }

    // ---- PCs ---------------------------------------------------------------------------

    /// A PC proved its key and its license: know it by `fingerprint`. A connection already there
    /// for that key is closed (its phones hear `pc_offline`) and this one takes its place. Gives
    /// this connection's generation, or the code when the license or the address already has
    /// as many PCs as it may.
    pub fn register_pc(
        &self,
        fingerprint: &str,
        key: [u8; 32],
        link: Link,
        license: [u8; 32],
        address: AddressKey,
    ) -> Result<u64, &'static str> {
        let limits = &self.config.limits;
        let mut st = lock(&self.state);
        // The connection this one replaces, if any, is not counted against it.
        let old = st.pcs.get(fingerprint);
        let replaces_on_license = old.is_some_and(|o| o.license == license);
        let replaces_at_address = old.is_some_and(|o| o.address == address);
        let on_license = st
            .licenses
            .get(&license)
            .copied()
            .unwrap_or(0)
            .saturating_sub(usize::from(replaces_on_license));
        let at_address = st
            .addresses
            .get(&address)
            .map_or(0, |u| u.pcs)
            .saturating_sub(usize::from(replaces_at_address));
        if on_license >= limits.pcs_per_license || at_address >= limits.pcs_per_address {
            return Err(codes::TOO_MANY_TRIES);
        }
        st.next_generation += 1;
        let generation = st.next_generation;
        if let Some(old) = st.pcs.remove(fingerprint) {
            log::info!("pc replaced its connection");
            Self::phones_lose_pc(&mut st, &old);
            Self::forget_pc(&mut st, &old);
            old.link.close();
        }
        *st.licenses.entry(license).or_default() += 1;
        st.addresses.entry(address).or_default().pcs += 1;
        st.pcs.insert(
            fingerprint.to_owned(),
            Pc {
                link,
                key,
                mailbox: None,
                dropped: HashMap::new(),
                phones: HashSet::new(),
                generation,
                license,
                address,
            },
        );
        Ok(generation)
    }

    /// This PC connection ended: its phones hear `pc_offline` and are closed.
    pub fn unregister_pc(&self, fingerprint: &str, generation: u64) {
        let mut st = lock(&self.state);
        if st
            .pcs
            .get(fingerprint)
            .is_some_and(|p| p.generation == generation)
        {
            if let Some(pc) = st.pcs.remove(fingerprint) {
                Self::phones_lose_pc(&mut st, &pc);
                Self::forget_pc(&mut st, &pc);
            }
        }
    }

    /// A PC is gone: its license and its address have one PC fewer.
    fn forget_pc(st: &mut State, pc: &Pc) {
        if let Some(n) = st.licenses.get_mut(&pc.license) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                st.licenses.remove(&pc.license);
            }
        }
        if let Some(u) = st.addresses.get_mut(&pc.address) {
            u.pcs = u.pcs.saturating_sub(1);
        }
    }

    fn phones_lose_pc(st: &mut State, pc: &Pc) {
        for conn in &pc.phones {
            if let Some(phone) = st.phones.remove(conn) {
                phone.link.send(wire::write(&RelayToPhone::PcOffline));
                phone.link.close();
            }
        }
    }

    fn pc_mut<'a>(st: &'a mut State, fingerprint: &str, generation: u64) -> Option<&'a mut Pc> {
        st.pcs
            .get_mut(fingerprint)
            .filter(|p| p.generation == generation)
    }

    pub fn open_mailbox(&self, fingerprint: &str, generation: u64, name: String) {
        let mut st = lock(&self.state);
        if let Some(pc) = Self::pc_mut(&mut st, fingerprint, generation) {
            pc.mailbox = Some(Mailbox {
                name,
                opened: Instant::now(),
                tries: 0,
            });
        }
    }

    pub fn close_mailbox(&self, fingerprint: &str, generation: u64) {
        let mut st = lock(&self.state);
        if let Some(pc) = Self::pc_mut(&mut st, fingerprint, generation) {
            pc.mailbox = None;
        }
    }

    /// The PC drops a phone's pass until `until`: each of that phone's connections hears
    /// `bad_pass` and is closed, and its pass is refused until then.
    pub fn drop_phone(&self, fingerprint: &str, generation: u64, phone: &str, until: i64) {
        let most = self.config.limits.dropped_per_pc;
        let now = self.now();
        let mut st = lock(&self.state);
        let Some(pc) = Self::pc_mut(&mut st, fingerprint, generation) else {
            return;
        };
        pc.dropped.retain(|_, u| *u > now);
        if pc.dropped.len() >= most {
            // Full even after forgetting the expired ones: the one ending soonest goes.
            if let Some(oldest) = pc
                .dropped
                .iter()
                .min_by_key(|(_, u)| **u)
                .map(|(k, _)| k.clone())
            {
                pc.dropped.remove(&oldest);
            }
        }
        pc.dropped.insert(phone.to_owned(), until);
        let candidates: Vec<ConnId> = pc.phones.iter().cloned().collect();
        let gone: Vec<ConnId> = candidates
            .into_iter()
            .filter(|c| {
                st.phones
                    .get(c)
                    .is_some_and(|l| l.phone.as_deref() == Some(phone))
            })
            .collect();
        for conn in gone {
            if let Some(pc) = Self::pc_mut(&mut st, fingerprint, generation) {
                pc.phones.remove(&conn);
            }
            if let Some(l) = st.phones.remove(&conn) {
                l.link.send(wire::write(&RelayToPhone::Refused {
                    code: codes::BAD_PASS.into(),
                }));
                l.link.close();
                *st.refused.entry(codes::BAD_PASS.to_owned()).or_default() += 1;
                #[cfg(feature = "test-hooks")]
                st.refused_in_order.push(codes::BAD_PASS.to_owned());
            }
        }
    }

    /// One sealed message from the PC to one of its phones' connections. `Err`: not one of its
    /// phones' connections.
    pub fn send_to_phone(&self, fingerprint: &str, conn: &str, data: String) -> Result<(), ()> {
        let mut st = lock(&self.state);
        if !st.phones.get(conn).is_some_and(|p| p.pc == fingerprint) {
            return Err(());
        }
        Self::keep_seen(&mut st, &data);
        let phone = st.phones.get(conn).ok_or(())?;
        phone.link.send(wire::write(&RelayToPhone::Data { data }));
        Ok(())
    }

    /// The PC closes one of its phones' connections (the phone hears nothing but the close).
    pub fn close_phone(&self, fingerprint: &str, conn: &str) {
        let mut st = lock(&self.state);
        if st.phones.get(conn).is_some_and(|p| p.pc == fingerprint) {
            if let Some(phone) = st.phones.remove(conn) {
                phone.link.close();
            }
            if let Some(pc) = st.pcs.get_mut(fingerprint) {
                pc.phones.remove(conn);
            }
        }
    }

    // ---- Phones ------------------------------------------------------------------------

    /// A phone shows a pass: let it through to its PC, or say why not (the code).
    pub fn join_with_pass(&self, pass: &str, link: Link) -> Result<ConnId, &'static str> {
        use plenipo_relay_contract::pass;
        let now = self.now();
        let Some(fingerprint) = pass::pc_of(pass) else {
            return Err(codes::BAD_PASS);
        };
        let mut st = lock(&self.state);
        let Some(pc) = st.pcs.get(&fingerprint) else {
            return Err(codes::PC_OFFLINE);
        };
        let payload = match pass::check(pass, &pc.key, now) {
            Ok(p) if pc.dropped.get(&p.phone).is_none_or(|&until| until <= now) => p,
            _ => return Err(codes::BAD_PASS),
        };
        if pc.phones.len() >= self.config.limits.phones_per_pc {
            return Err(codes::TOO_MANY_TRIES);
        }
        Ok(Self::add_phone(
            &mut st,
            fingerprint,
            Some(payload.phone),
            link,
        ))
    }

    /// A phone joins a pairing mailbox: let it through to the PC waiting there, or say why not.
    pub fn join_mailbox(&self, name: &str, link: Link) -> Result<ConnId, &'static str> {
        let life = self.config.limits.mailbox_life;
        let tries = self.config.limits.mailbox_tries;
        let most = self.config.limits.phones_per_pc;
        let mut st = lock(&self.state);
        // A mailbox open too long is gone.
        for pc in st.pcs.values_mut() {
            if pc
                .mailbox
                .as_ref()
                .is_some_and(|m| m.opened.elapsed() > life)
            {
                pc.mailbox = None;
            }
        }
        let found = st
            .pcs
            .iter_mut()
            .find(|(_, p)| p.mailbox.as_ref().is_some_and(|m| m.name == name));
        let fingerprint = match found {
            Some((fingerprint, pc)) => {
                if pc.phones.len() >= most {
                    return Err(codes::TOO_MANY_TRIES);
                }
                let Some(mailbox) = pc.mailbox.as_mut() else {
                    return Err(codes::MAILBOX_CLOSED);
                };
                if mailbox.tries >= tries {
                    return Err(codes::TOO_MANY_TRIES);
                }
                mailbox.tries += 1;
                fingerprint.clone()
            }
            None => return Err(codes::MAILBOX_CLOSED),
        };
        Ok(Self::add_phone(&mut st, fingerprint, None, link))
    }

    fn add_phone(st: &mut State, fingerprint: String, phone: Option<String>, link: Link) -> ConnId {
        st.next_conn += 1;
        let conn = format!("c{:06}", st.next_conn);
        if let Some(pc) = st.pcs.get_mut(&fingerprint) {
            pc.phones.insert(conn.clone());
            pc.link.send(wire::write(&RelayToPc::Joined {
                conn: conn.clone(),
                phone: phone.clone(),
                mailbox: phone.is_none(),
            }));
        }
        st.phones.insert(
            conn.clone(),
            Phone {
                link,
                pc: fingerprint,
                phone,
            },
        );
        conn
    }

    /// One sealed message from a phone to its PC. `Err`: the PC is gone.
    pub fn send_to_pc(&self, conn: &str, data: String) -> Result<(), ()> {
        let mut st = lock(&self.state);
        let Some(fingerprint) = st.phones.get(conn).map(|p| p.pc.clone()) else {
            return Err(());
        };
        Self::keep_seen(&mut st, &data);
        let pc = st.pcs.get(&fingerprint).ok_or(())?;
        pc.link.send(wire::write(&RelayToPc::Data {
            conn: conn.to_owned(),
            data,
        }));
        Ok(())
    }

    /// This phone connection ended: its PC hears `left` (unless the PC closed it itself).
    pub fn unregister_phone(&self, conn: &str) {
        let mut st = lock(&self.state);
        if let Some(phone) = st.phones.remove(conn) {
            if let Some(pc) = st.pcs.get_mut(&phone.pc) {
                pc.phones.remove(conn);
                pc.link.send(wire::write(&RelayToPc::Left {
                    conn: conn.to_owned(),
                }));
            }
        }
    }

    // ---- The tests' hooks --------------------------------------------------------------

    /// The relay keeps no copy of anything it passes. Only a copy built for the tests keeps the
    /// sealed messages, so a test can prove the words never showed.
    #[cfg(feature = "test-hooks")]
    fn keep_seen(st: &mut State, data: &str) {
        if st.seen.len() < 20_000 {
            if let Some(bytes) = plenipo_relay_contract::b64::decode(data, wire::MAX_DATA) {
                st.seen.push(bytes);
            }
        }
    }

    #[cfg(not(feature = "test-hooks"))]
    fn keep_seen(_st: &mut State, _data: &str) {}

    #[cfg(feature = "test-hooks")]
    pub fn refused_in_order(&self) -> Vec<String> {
        lock(&self.state).refused_in_order.clone()
    }

    #[cfg(feature = "test-hooks")]
    pub fn mailbox_open(&self) -> Option<String> {
        lock(&self.state)
            .pcs
            .values()
            .find_map(|p| p.mailbox.as_ref().map(|m| m.name.clone()))
    }

    #[cfg(feature = "test-hooks")]
    pub fn seen(&self) -> Vec<Vec<u8>> {
        lock(&self.state).seen.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;

    /// A hub with a clock the test moves (Unix seconds).
    fn hub_with_clock(limits: Limits) -> (Hub, Arc<AtomicI64>) {
        let time = Arc::new(AtomicI64::new(1_791_000_000));
        let clock: crate::Clock = {
            let time = time.clone();
            Arc::new(move || time.load(Ordering::SeqCst))
        };
        let hub = Hub::new(Config {
            limits,
            clock,
            ..Config::default()
        });
        (hub, time)
    }

    /// A distinct IPv4 address for each `n`.
    fn address(n: u32) -> AddressKey {
        AddressKey::V4(Ipv4Addr::from(0x0A00_0000 + n))
    }

    #[test]
    fn idle_addresses_are_forgotten_by_the_sweep() {
        let (hub, time) = hub_with_clock(Limits::default());
        for n in 0..10_000 {
            hub.enter(address(n)).unwrap();
            hub.leave(address(n));
        }
        // Each came this minute: still counted, so still remembered.
        assert_eq!(hub.stats().addresses, 10_000);
        hub.sweep_addresses();
        assert_eq!(hub.stats().addresses, 10_000);
        // A minute on, nothing counts for any of them.
        time.fetch_add(61, Ordering::SeqCst);
        hub.sweep_addresses();
        assert_eq!(hub.stats().addresses, 0);
    }

    #[test]
    fn the_sweep_keeps_an_address_with_something_open_or_counted() {
        let (hub, time) = hub_with_clock(Limits::default());
        hub.enter(address(1)).unwrap();
        hub.enter(address(2)).unwrap();
        hub.leave(address(2));
        hub.refused("bad_pass", address(2));
        time.fetch_add(61, Ordering::SeqCst);
        hub.refused("bad_pass", address(3));
        hub.sweep_addresses();
        // 1 is open; 2's refusal was last minute; 3's is this minute.
        assert_eq!(hub.stats().addresses, 2);
        hub.leave(address(1));
        time.fetch_add(61, Ordering::SeqCst);
        hub.sweep_addresses();
        assert_eq!(hub.stats().addresses, 0);
    }

    #[test]
    fn a_full_table_is_swept_at_once_and_then_lets_the_counted_only_ones_go() {
        let (hub, time) = hub_with_clock(Limits {
            addresses_remembered: 100,
            ..Limits::default()
        });
        // One address with a connection open, one the door is turning away this minute (too
        // many refusals), and 98 that came and went this minute.
        hub.enter(address(0)).unwrap();
        for _ in 0..hub.config.limits.tries_per_address_per_minute {
            hub.refused("bad_pass", address(1));
        }
        for n in 2..100 {
            hub.enter(address(n)).unwrap();
            hub.leave(address(n));
        }
        // Full, and nothing idle to forget yet: the ones that are only counted go, so the new
        // address comes in; the busy one stays, and so does the one under the brake.
        hub.enter(address(100)).unwrap();
        assert_eq!(hub.stats().addresses, 3);
        assert_eq!(hub.enter(address(1)), Err(Turned::TooMany), "still braked");
        hub.leave(address(100));
        // Full again with nothing but braked ones left over (every counted-only address over
        // its refusals): then those go too, last of all; the busy one still stays.
        for _ in 0..hub.config.limits.tries_per_address_per_minute {
            hub.refused("bad_pass", address(100));
        }
        for n in 2..99 {
            hub.enter(address(n)).unwrap();
            hub.leave(address(n));
            for _ in 0..hub.config.limits.tries_per_address_per_minute {
                hub.refused("bad_pass", address(n));
            }
        }
        hub.enter(address(101)).unwrap();
        assert_eq!(hub.stats().addresses, 2, "the busy one and the new one");
        hub.leave(address(101));
        // A minute on, the sweep alone makes room: nothing busy is touched either way.
        for n in 2..100 {
            hub.enter(address(n)).unwrap();
            hub.leave(address(n));
        }
        time.fetch_add(61, Ordering::SeqCst);
        hub.enter(address(200)).unwrap();
        assert_eq!(hub.stats().addresses, 2);
    }

    #[tokio::test]
    async fn the_byte_bound_alone_kills_a_link_that_queues_too_much() {
        let (out, mut queue) = tokio::sync::mpsc::channel::<Message>(256);
        let link = Link {
            out,
            kill: Arc::new(Notify::new()),
            queued: Arc::new(AtomicUsize::new(0)),
            most_queued: 100,
        };
        // Two messages, far under the count of 256, but over the bytes.
        link.send(String::from_utf8(vec![b'a'; 60]).unwrap());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), link.kill.notified())
                .await
                .is_err(),
            "one message fits"
        );
        link.send(String::from_utf8(vec![b'b'; 60]).unwrap());
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), link.kill.notified())
                .await
                .is_ok(),
            "the second is over the bytes: killed"
        );
        // Only the first was queued.
        assert_eq!(queue.try_recv().map(|m| m.len()), Ok(60));
        assert!(queue.try_recv().is_err());
    }
}
