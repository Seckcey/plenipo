//! What the relay knows while it runs: the PCs connected (by fingerprint), their phones (by
//! connection), each PC's pairing mailbox and dropped passes, and the counts behind the limits.
//! Everything is in memory, under one lock, and gone when a connection ends. Nothing is written
//! anywhere.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use plenipo_relay_contract::wire::{self, codes, RelayToPc, RelayToPhone};
use tokio::sync::mpsc::Sender;
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::Message;

use crate::limits::{AddressKey, Window};
use crate::{Config, Stats};

pub(crate) type ConnId = String;

/// How the hub reaches one connection: its outgoing queue, and a way to end it at once when the
/// queue is full (a peer that does not read).
#[derive(Clone)]
pub(crate) struct Link {
    pub out: Sender<Message>,
    pub kill: Arc<Notify>,
}

impl Link {
    pub fn send_message(&self, m: Message) {
        if self.out.try_send(m).is_err() {
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
}

struct Phone {
    link: Link,
    pc: String,
    phone: Option<String>,
}

#[derive(Default)]
struct AddressUse {
    open: usize,
    new: Window,
    tries: Window,
}

#[derive(Default)]
struct State {
    pcs: HashMap<String, Pc>,
    phones: HashMap<ConnId, Phone>,
    addresses: HashMap<AddressKey, AddressUse>,
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
}

fn lock(m: &Mutex<State>) -> MutexGuard<'_, State> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Hub {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: Mutex::new(State::default()),
        }
    }

    pub fn now(&self) -> i64 {
        (self.config.clock)()
    }

    pub fn stats(&self) -> Stats {
        let st = lock(&self.state);
        Stats {
            connections: st.connections,
            pcs: st.pcs.len(),
            phones: st.phones.len(),
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
        } else if st.connections >= limits.connections {
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

    /// An address with nothing open and nothing counted this minute takes no memory.
    fn forget_idle_address(st: &mut State, address: AddressKey, now: i64) {
        let idle = st
            .addresses
            .get(&address)
            .is_some_and(|u| u.open == 0 && u.new.count(now) == 0 && u.tries.count(now) == 0);
        if idle {
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

    /// A PC proved its key: know it by `fingerprint`. A connection already there for that key is
    /// closed (its phones hear `pc_offline`). Gives this connection's generation.
    pub fn register_pc(&self, fingerprint: &str, key: [u8; 32], link: Link) -> u64 {
        let mut st = lock(&self.state);
        st.next_generation += 1;
        let generation = st.next_generation;
        if let Some(old) = st.pcs.remove(fingerprint) {
            log::info!("pc replaced its connection");
            Self::phones_lose_pc(&mut st, &old);
            old.link.close();
        }
        st.pcs.insert(
            fingerprint.to_owned(),
            Pc {
                link,
                key,
                mailbox: None,
                dropped: HashMap::new(),
                phones: HashSet::new(),
                generation,
            },
        );
        generation
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
            }
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
