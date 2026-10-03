//! The relay's limits (the contract's "Limits the relay keeps"): per internet address, per PC,
//! per connection, and in all. Each has a plain default; the program may change them.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

/// Every limit, with its default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Open connections in all (PCs and phones). Over it, the door answers `503`.
    pub connections: usize,
    /// Open connections from one address (IPv6 by its /64). Over it, the door answers `429`.
    pub connections_per_address: usize,
    /// New connections from one address in a minute.
    pub new_per_address_per_minute: u32,
    /// Refusals (a bad pass, a bad hello, a closed mailbox…) for one address in a minute. Over
    /// it, the door answers `429` for the rest of the minute.
    pub tries_per_address_per_minute: u32,
    /// Addresses the relay remembers at once (each with its open connections and this minute's
    /// counts). Idle ones are forgotten on the timer; at the cap, they are forgotten at once, and
    /// if the table is still full, the door answers `503`.
    pub addresses_remembered: usize,
    /// How often idle addresses are forgotten.
    pub address_sweep: Duration,
    /// Phone connections one PC may have at once (`too_many_tries` beyond it).
    pub phones_per_pc: usize,
    /// PCs one license (one weekly answer's key ID) may have connected at once
    /// (`too_many_tries` beyond it). A subscription is one person on any of their own PCs
    /// (ADR-110), so this is a brake on a leaked answer, not a count of a person's PCs.
    pub pcs_per_license: usize,
    /// PCs one address may have connected at once (`too_many_tries` beyond it). An office or a
    /// home shares one address, so this allows several.
    pub pcs_per_address: usize,
    /// Messages one connection may send in a minute.
    pub messages_per_minute: u32,
    /// Bytes one connection may send in a minute.
    pub bytes_per_minute: usize,
    /// How long a connection has to say its first message (the PC's hello, the phone's pass).
    pub first_message: Duration,
    /// A connection that sends nothing (not even a pong) for this long is closed.
    pub idle: Duration,
    /// The relay pings every connection this often.
    pub ping_every: Duration,
    /// A pairing mailbox stays open this long at most.
    pub mailbox_life: Duration,
    /// Phone connections a mailbox takes in all.
    pub mailbox_tries: u32,
    /// Dropped passes remembered per PC (the oldest goes when full).
    pub dropped_per_pc: usize,
    /// Messages waiting to go out to one connection. A peer that does not read is closed.
    pub outgoing_queue: usize,
    /// Bytes waiting to go out to one connection, in all. Over it, the peer is closed at once:
    /// what bounds the memory a peer that does not read can hold.
    pub outgoing_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            connections: 512,
            connections_per_address: 32,
            new_per_address_per_minute: 120,
            tries_per_address_per_minute: 30,
            addresses_remembered: 100_000,
            address_sweep: Duration::from_secs(60),
            phones_per_pc: 40,
            pcs_per_license: 10,
            pcs_per_address: 8,
            messages_per_minute: 1200,
            bytes_per_minute: 16 * 1024 * 1024,
            first_message: Duration::from_secs(10),
            idle: Duration::from_secs(90),
            ping_every: Duration::from_secs(30),
            mailbox_life: Duration::from_secs(600),
            mailbox_tries: 3,
            dropped_per_pc: 1000,
            outgoing_queue: 256,
            outgoing_bytes: 1024 * 1024,
        }
    }
}

/// An internet address as the limits count it: IPv4 whole, IPv6 by its /64 (one home or phone
/// usually has a whole /64, and could otherwise look like billions of addresses).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressKey {
    V4(Ipv4Addr),
    V6([u8; 8]),
}

impl From<IpAddr> for AddressKey {
    fn from(ip: IpAddr) -> Self {
        match ip {
            IpAddr::V4(v4) => Self::V4(v4),
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => Self::V4(v4),
                None => {
                    let mut prefix = [0u8; 8];
                    prefix.copy_from_slice(&v6.octets()[..8]);
                    Self::V6(prefix)
                }
            },
        }
    }
}

impl fmt::Display for AddressKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::V4(v4) => write!(f, "{v4}"),
            Self::V6(p) => write!(
                f,
                "{:x}:{:x}:{:x}:{:x}::/64",
                u16::from_be_bytes([p[0], p[1]]),
                u16::from_be_bytes([p[2], p[3]]),
                u16::from_be_bytes([p[4], p[5]]),
                u16::from_be_bytes([p[6], p[7]])
            ),
        }
    }
}

/// A count for the current minute (a fixed window: simple, small, and good enough).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Window {
    minute: i64,
    count: u64,
}

impl Window {
    /// Count one more at `now` (Unix seconds); the new count for this minute.
    pub fn bump(&mut self, now: i64, by: u64) -> u64 {
        let minute = now.div_euclid(60);
        if minute != self.minute {
            self.minute = minute;
            self.count = 0;
        }
        self.count = self.count.saturating_add(by);
        self.count
    }

    /// The count for the minute `now` is in.
    pub fn count(&self, now: i64) -> u64 {
        if now.div_euclid(60) == self.minute {
            self.count
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv6_counts_by_its_64() {
        let a: AddressKey = "2001:db8:1:2:aaaa::1".parse::<IpAddr>().unwrap().into();
        let b: AddressKey = "2001:db8:1:2:ffff::9".parse::<IpAddr>().unwrap().into();
        let c: AddressKey = "2001:db8:1:3::1".parse::<IpAddr>().unwrap().into();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.to_string(), "2001:db8:1:2::/64");
        let v4: AddressKey = "203.0.113.9".parse::<IpAddr>().unwrap().into();
        let mapped: AddressKey = "::ffff:203.0.113.9".parse::<IpAddr>().unwrap().into();
        assert_eq!(v4, mapped);
        assert_eq!(v4.to_string(), "203.0.113.9");
    }

    #[test]
    fn a_window_counts_one_minute_at_a_time() {
        let mut w = Window::default();
        assert_eq!(w.bump(120, 1), 1);
        assert_eq!(w.bump(179, 1), 2);
        assert_eq!(w.count(179), 2);
        assert_eq!(w.count(180), 0);
        assert_eq!(w.bump(180, 5), 5);
    }
}
