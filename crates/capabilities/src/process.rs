//! Which program holds the far end of a loopback connection, and whose process tree it is in
//! (ADR-033, approved programs run as the owner). The tool server honors a grant's ticket only
//! from the AI tool Plenipo started for that grant's step, or from a program that AI tool
//! started; a ticket copied by any other program is refused. Linux reads `/proc`; Windows asks
//! the TCP table and the process snapshot; elsewhere the lookup is unavailable and the caller
//! says so. A lookup that exists but fails names no holder, so the caller refuses: only an
//! operating system with no lookup at all is ever reported as unavailable.

use std::net::SocketAddr;

/// Longest chain of parents followed before giving up (a broken table cannot loop forever).
const MAX_DEPTH: usize = 64;

/// The programs holding the far end of a loopback TCP connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Holders {
    /// The process IDs of every program holding the connection's far end (one, unless the
    /// socket was handed on to another program).
    Pids(Vec<u32>),
    /// No program on this computer was found holding it: it ended, it belongs to another
    /// user, or the lookup itself failed (the connection table could not be read). The caller
    /// refuses the connection.
    Unknown,
    /// This operating system offers no way to look it up at all (macOS today). Only the module
    /// for such systems returns this; a lookup that exists and fails is `Unknown`, so on Linux
    /// and Windows nothing ever builds this answer (hence the allowance below).
    #[cfg_attr(any(target_os = "linux", windows), allow(dead_code))]
    Unavailable,
}

/// The programs holding the far end of the connection from `peer` to `local` (both on the
/// loopback address).
pub fn holders_of(peer: SocketAddr, local: SocketAddr) -> Holders {
    os::holders_of(peer, local)
}

/// The parent of process `pid` (`None`: it has none, or it is gone).
pub fn parent_of(pid: u32) -> Option<u32> {
    os::parent_of(pid)
}

/// Whether `pid` is `root` or a descendant of it, `parent` giving any process's parent.
pub fn descends_from(pid: u32, root: u32, parent: impl Fn(u32) -> Option<u32>) -> bool {
    if root == 0 {
        return false;
    }
    let mut current = pid;
    for _ in 0..MAX_DEPTH {
        if current == root {
            return true;
        }
        match parent(current) {
            Some(p) if p != 0 && p != current => current = p,
            _ => return false,
        }
    }
    false
}

#[cfg(target_os = "linux")]
mod os {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
    use std::path::Path;

    use super::Holders;

    pub fn holders_of(peer: SocketAddr, local: SocketAddr) -> Holders {
        holders_in(Path::new("/proc"), peer, local)
    }

    /// `holders_of`, reading the `proc` file system mounted at `proc_dir`.
    pub(super) fn holders_in(proc_dir: &Path, peer: SocketAddr, local: SocketAddr) -> Holders {
        let tables: Vec<String> = ["net/tcp", "net/tcp6"]
            .iter()
            .filter_map(|f| std::fs::read_to_string(proc_dir.join(f)).ok())
            .collect();
        // No table to read is a failed lookup, not a missing one: refuse, never let through.
        if tables.is_empty() {
            return Holders::Unknown;
        }
        let Some(inode) = tables.iter().find_map(|t| socket_inode(t, peer, local)) else {
            return Holders::Unknown;
        };
        let pids = pids_holding(proc_dir, inode);
        if pids.is_empty() {
            Holders::Unknown
        } else {
            Holders::Pids(pids)
        }
    }

    /// The inode of the socket whose own end is `peer` and whose far end is `local`, in a
    /// `/proc/net/tcp` or `/proc/net/tcp6` table.
    pub(super) fn socket_inode(table: &str, peer: SocketAddr, local: SocketAddr) -> Option<u64> {
        table.lines().skip(1).find_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            let (own, far, inode) = (f.get(1)?, f.get(2)?, f.get(9)?);
            let (own_ip, own_port) = endpoint(own)?;
            let (far_ip, far_port) = endpoint(far)?;
            let matches = own_port == peer.port()
                && far_port == local.port()
                && same_ip(own_ip, peer.ip())
                && same_ip(far_ip, local.ip());
            matches.then(|| inode.parse().ok()).flatten()
        })
    }

    /// `ADDRESS:PORT` as the kernel prints it: hex, each 32-bit word in the machine's order.
    fn endpoint(text: &str) -> Option<(IpAddr, u16)> {
        let (ip, port) = text.split_once(':')?;
        let port = u16::from_str_radix(port, 16).ok()?;
        let ip = match ip.len() {
            8 => IpAddr::V4(Ipv4Addr::from(
                u32::from_str_radix(ip, 16).ok()?.to_ne_bytes(),
            )),
            32 => {
                let mut bytes = [0u8; 16];
                for (i, chunk) in ip.as_bytes().chunks(8).enumerate() {
                    let word = u32::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
                    bytes[i * 4..i * 4 + 4].copy_from_slice(&word.to_ne_bytes());
                }
                IpAddr::V6(Ipv6Addr::from(bytes))
            }
            _ => return None,
        };
        Some((ip, port))
    }

    fn same_ip(a: IpAddr, b: IpAddr) -> bool {
        let plain = |ip: IpAddr| match ip {
            IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
            v4 => v4,
        };
        plain(a) == plain(b)
    }

    /// Every process under `proc_dir` with the socket `inode` open.
    pub(super) fn pids_holding(proc_dir: &Path, inode: u64) -> Vec<u32> {
        let target = format!("socket:[{inode}]");
        let mut pids = Vec::new();
        let Ok(entries) = std::fs::read_dir(proc_dir) else {
            return pids;
        };
        for entry in entries.filter_map(Result::ok) {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|n| n.parse::<u32>().ok())
            else {
                continue;
            };
            let Ok(fds) = std::fs::read_dir(entry.path().join("fd")) else {
                continue;
            };
            if fds.filter_map(Result::ok).any(|fd| {
                std::fs::read_link(fd.path()).is_ok_and(|l| l.as_os_str() == target.as_str())
            }) {
                pids.push(pid);
            }
        }
        pids
    }

    pub fn parent_of(pid: u32) -> Option<u32> {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
        parent_in_status(&status)
    }

    pub(super) fn parent_in_status(status: &str) -> Option<u32> {
        status
            .lines()
            .find_map(|l| l.strip_prefix("PPid:"))
            .and_then(|v| v.trim().parse().ok())
    }
}

#[cfg(windows)]
mod os {
    // The only unsafe code in this crate: calls into Windows for the TCP table and the process
    // list, each with the reason it is sound written next to it (see this crate's lints).
    #![allow(unsafe_code)]

    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_INSUFFICIENT_BUFFER, INVALID_HANDLE_VALUE, NO_ERROR,
    };
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
    };
    use windows_sys::Win32::Networking::WinSock::AF_INET;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS,
    };

    use super::Holders;

    /// Room added beyond the size the table asked for, so that connections opened between the
    /// size query and the copy still fit.
    const SLACK: u32 = 64 * 1024;
    /// How many times the copy is tried when the table outgrew the buffer meanwhile.
    const ATTEMPTS: usize = 16;

    pub fn holders_of(peer: SocketAddr, local: SocketAddr) -> Holders {
        // A table that cannot be read is a failed lookup, not a missing one: refuse, never let
        // through. A program running as the owner can keep the table changing, so the copy
        // below leaves room and tries again rather than giving up early.
        let Some(rows) = tcp_rows() else {
            return Holders::Unknown;
        };
        // Addresses and ports come in network byte order; the port sits in the low 16 bits.
        let ip = |addr: u32| IpAddr::V4(Ipv4Addr::from(addr.to_ne_bytes()));
        let port = |p: u32| u16::from_be((p & 0xFFFF) as u16);
        let found = rows.iter().find(|r| {
            port(r.dwLocalPort) == peer.port()
                && port(r.dwRemotePort) == local.port()
                && ip(r.dwLocalAddr) == peer.ip()
                && ip(r.dwRemoteAddr) == local.ip()
        });
        match found {
            Some(r) if r.dwOwningPid != 0 => Holders::Pids(vec![r.dwOwningPid]),
            _ => Holders::Unknown,
        }
    }

    /// Every IPv4 TCP connection with its owning process (the tool server listens on IPv4).
    fn tcp_rows() -> Option<Vec<MIB_TCPROW_OWNER_PID>> {
        let family = u32::from(AF_INET);
        let mut size: u32 = 0;
        // SAFETY: a null table with size 0 only asks for the size needed.
        let first = unsafe {
            GetExtendedTcpTable(
                std::ptr::null_mut(),
                &mut size,
                0,
                family,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if first != ERROR_INSUFFICIENT_BUFFER && first != NO_ERROR {
            return None;
        }
        // Connections come and go between the two calls: leave room, and try again with the
        // size the call asked for when the table still grew past it.
        for _ in 0..ATTEMPTS {
            let mut want = size.saturating_add(SLACK);
            let mut buffer = vec![0u32; (want as usize).div_ceil(4).max(1)];
            // SAFETY: the buffer holds `want` bytes (aligned for the table's 32-bit fields),
            // and `want` says how many the call may write; the call raises it to the size
            // needed when the buffer is too small, and writes nothing then.
            let result = unsafe {
                GetExtendedTcpTable(
                    buffer.as_mut_ptr().cast(),
                    &mut want,
                    0,
                    family,
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                )
            };
            if result == ERROR_INSUFFICIENT_BUFFER {
                size = want;
                continue;
            }
            if result != NO_ERROR {
                return None;
            }
            let bytes = buffer.len() * 4;
            let head = std::mem::offset_of!(MIB_TCPTABLE_OWNER_PID, table);
            let table = buffer.as_ptr().cast::<MIB_TCPTABLE_OWNER_PID>();
            // SAFETY: the call filled a MIB_TCPTABLE_OWNER_PID (a count, then that many
            // rows); the count is capped to what the buffer can hold.
            let count = unsafe { (*table).dwNumEntries } as usize;
            let count =
                count.min(bytes.saturating_sub(head) / std::mem::size_of::<MIB_TCPROW_OWNER_PID>());
            let rows = unsafe { std::ptr::addr_of!((*table).table).cast::<MIB_TCPROW_OWNER_PID>() };
            return Some(
                (0..count)
                    // SAFETY: row `i` lies inside the buffer (see the cap above).
                    .map(|i| unsafe { rows.add(i).read_unaligned() })
                    .collect(),
            );
        }
        None
    }

    pub fn parent_of(pid: u32) -> Option<u32> {
        // SAFETY: a snapshot of the process list, closed below.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
            return None;
        }
        // SAFETY: PROCESSENTRY32 is plain data; dwSize tells the calls which version it is.
        let mut entry: PROCESSENTRY32 = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32>() as u32;
        let mut parent = None;
        // SAFETY: the snapshot is open and `entry` is sized for the call.
        if unsafe { Process32First(snapshot, &mut entry) } != 0 {
            loop {
                if entry.th32ProcessID == pid {
                    parent = Some(entry.th32ParentProcessID);
                    break;
                }
                // SAFETY: as above.
                if unsafe { Process32Next(snapshot, &mut entry) } == 0 {
                    break;
                }
            }
        }
        // SAFETY: closes the handle opened above.
        unsafe { CloseHandle(snapshot) };
        parent
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod os {
    use std::net::SocketAddr;

    use super::Holders;

    pub fn holders_of(_peer: SocketAddr, _local: SocketAddr) -> Holders {
        Holders::Unavailable
    }

    pub fn parent_of(_pid: u32) -> Option<u32> {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn descent_follows_parents_and_stops_on_broken_tables() {
        // 1 → 100 (the AI tool) → 200 → 300; 400 is another child of 1; 500 and 600 loop.
        let tree: HashMap<u32, u32> = [
            (100, 1),
            (200, 100),
            (300, 200),
            (400, 1),
            (500, 600),
            (600, 500),
        ]
        .into_iter()
        .collect();
        let parent = |pid: u32| tree.get(&pid).copied();
        assert!(descends_from(100, 100, parent));
        assert!(descends_from(200, 100, parent));
        assert!(descends_from(300, 100, parent));
        assert!(!descends_from(400, 100, parent));
        assert!(!descends_from(1, 100, parent));
        assert!(!descends_from(500, 100, parent));
        assert!(!descends_from(999, 100, parent));
        assert!(!descends_from(300, 0, parent));
        // A parent of 0 (or a self-parent) ends the walk.
        assert!(!descends_from(7, 100, |_| Some(0)));
        assert!(!descends_from(7, 100, Some));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_tables_and_status_are_read() {
        use std::net::{Ipv4Addr, SocketAddr};

        let peer = SocketAddr::from((Ipv4Addr::LOCALHOST, 0x1F90));
        let local = SocketAddr::from((Ipv4Addr::LOCALHOST, 0x0016));
        let table = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
                     \x20  0: 0100007F:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 11111 1 0000000000000000 100 0 0 10 0\n\
                     \x20  1: 0100007F:1F90 0100007F:0016 01 00000000:00000000 00:00000000 00000000  1000        0 22222 1 0000000000000000 20 4 30 10 -1\n\
                     \x20  2: 0100007F:0016 0100007F:1F90 01 00000000:00000000 00:00000000 00000000     0        0 33333 1 0000000000000000 20 4 30 10 -1\n";
        assert_eq!(os::socket_inode(table, peer, local), Some(22222));
        assert_eq!(os::socket_inode(table, local, peer), Some(33333));
        let other = SocketAddr::from((Ipv4Addr::LOCALHOST, 0x1F91));
        assert_eq!(os::socket_inode(table, other, local), None);
        // An IPv6 socket connected to the IPv4 loopback address (tcp6, mapped address).
        let table6 = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
                      \x20  0: 0000000000000000FFFF00000100007F:1F90 0000000000000000FFFF00000100007F:0016 01 00000000:00000000 00:00000000 00000000  1000        0 44444 1 0000000000000000 20 4 30 10 -1\n";
        assert_eq!(os::socket_inode(table6, peer, local), Some(44444));
        assert_eq!(
            os::parent_in_status("Name:\tx\nPid:\t42\nPPid:\t7\nUid:\t0\n"),
            Some(7)
        );
        assert_eq!(os::parent_in_status("Name:\tx\n"), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn this_process_is_found_holding_its_own_connection() {
        use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};

        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
        let local = listener.local_addr().unwrap();
        let client = TcpStream::connect(local).unwrap();
        let (_server_side, peer) = listener.accept().unwrap();
        assert_eq!(peer, client.local_addr().unwrap());
        let me = std::process::id();
        assert_eq!(holders_of(peer, local), Holders::Pids(vec![me]));
        // A port nobody holds.
        let nobody = SocketAddr::from((Ipv4Addr::LOCALHOST, 1));
        assert_eq!(holders_of(nobody, local), Holders::Unknown);
        let parent = parent_of(me).expect("this process has a parent");
        assert!(descends_from(me, parent, parent_of));
        assert!(!descends_from(parent, me, parent_of));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_failed_linux_lookup_names_no_holder_and_is_never_unavailable() {
        use std::net::{Ipv4Addr, SocketAddr};
        use std::path::Path;

        let peer = SocketAddr::from((Ipv4Addr::LOCALHOST, 0x1F90));
        let local = SocketAddr::from((Ipv4Addr::LOCALHOST, 0x0016));
        let proc_dir = tempfile::tempdir().unwrap();
        // No connection table can be read: a failed lookup, so the caller refuses; only an
        // operating system with no lookup at all is "unavailable".
        assert_eq!(
            os::holders_in(proc_dir.path(), peer, local),
            Holders::Unknown
        );
        // The table names the socket, but no program's open files can be searched.
        let table = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n\
                     \x20  0: 0100007F:1F90 0100007F:0016 01 00000000:00000000 00:00000000 00000000  1000        0 22222 1 0000000000000000 20 4 30 10 -1\n";
        std::fs::create_dir_all(proc_dir.path().join("net")).unwrap();
        std::fs::write(proc_dir.path().join("net/tcp"), table).unwrap();
        assert_eq!(
            os::holders_in(proc_dir.path(), peer, local),
            Holders::Unknown
        );
        assert!(os::pids_holding(Path::new("/nonexistent/proc"), 22222).is_empty());
    }
}
