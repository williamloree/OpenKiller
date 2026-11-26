//! Listening TCP and bound UDP sockets (IPv4 + IPv6) via iphlpapi, enriched with process details.

use crate::model::{PortEntry, Proto};
use crate::process;
use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
    MIB_UDP6ROW_OWNER_PID, MIB_UDPROW_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RawSocket {
    pub proto: Proto,
    pub ip: IpAddr,
    pub port: u16,
    pub pid: u32,
}

/// One socket per (port, protocol, PID); when IPv4 and IPv6 both exist, the IPv4 one wins.
/// Sorted by port, then protocol, then PID.
pub fn dedupe(raw: &[RawSocket]) -> Vec<RawSocket> {
    let mut map: BTreeMap<(u16, Proto, u32), RawSocket> = BTreeMap::new();
    for s in raw {
        map.entry((s.port, s.proto, s.pid))
            .and_modify(|cur| {
                if cur.ip.is_ipv6() && s.ip.is_ipv4() {
                    *cur = *s;
                }
            })
            .or_insert(*s);
    }
    map.into_values().collect()
}

/// netstat-like "0.0.0.0:3000" / "[::]:3000" (same as v1).
pub fn format_addr(ip: IpAddr, port: u16) -> String {
    std::net::SocketAddr::new(ip, port).to_string()
}

/// Calls a Get*Table function until the buffer is big enough; returns the raw table.
fn table(call: impl Fn(Option<*mut core::ffi::c_void>, &mut u32) -> u32) -> Option<Vec<u64>> {
    let mut size = 0u32;
    for _ in 0..5 {
        // u64 storage keeps the rows aligned.
        let mut buf = vec![0u64; (size as usize).div_ceil(8).max(1)];
        let ret = call(Some(buf.as_mut_ptr().cast()), &mut size);
        if ret == NO_ERROR.0 {
            return Some(buf);
        }
        if ret != ERROR_INSUFFICIENT_BUFFER.0 {
            return None;
        }
    }
    None
}

/// Rows of a MIB_*TABLE_OWNER_PID: a u32 count followed by the rows.
fn rows<T: Copy>(buf: &[u64]) -> Vec<T> {
    let bytes = buf.len() * 8;
    let count = unsafe { *(buf.as_ptr() as *const u32) } as usize;
    let offset = std::mem::align_of::<T>().max(4);
    let max = bytes.saturating_sub(offset) / size_of::<T>();
    let first = unsafe { (buf.as_ptr() as *const u8).add(offset) as *const T };
    (0..count.min(max))
        .map(|i| unsafe { *first.add(i) })
        .collect()
}

fn port_of(raw: u32) -> u16 {
    u16::from_be(raw as u16)
}

pub fn raw_sockets() -> Vec<RawSocket> {
    let mut out = Vec::new();
    let (v4, v6) = (AF_INET.0 as u32, AF_INET6.0 as u32);
    unsafe {
        if let Some(b) =
            table(|p, s| GetExtendedTcpTable(p, s, false, v4, TCP_TABLE_OWNER_PID_LISTENER, 0))
        {
            for r in rows::<MIB_TCPROW_OWNER_PID>(&b) {
                let ip = IpAddr::V4(Ipv4Addr::from(u32::from_be(r.dwLocalAddr)));
                out.push(RawSocket {
                    proto: Proto::Tcp,
                    ip,
                    port: port_of(r.dwLocalPort),
                    pid: r.dwOwningPid,
                });
            }
        }
        if let Some(b) =
            table(|p, s| GetExtendedTcpTable(p, s, false, v6, TCP_TABLE_OWNER_PID_LISTENER, 0))
        {
            for r in rows::<MIB_TCP6ROW_OWNER_PID>(&b) {
                let ip = IpAddr::V6(Ipv6Addr::from(r.ucLocalAddr));
                out.push(RawSocket {
                    proto: Proto::Tcp,
                    ip,
                    port: port_of(r.dwLocalPort),
                    pid: r.dwOwningPid,
                });
            }
        }
        if let Some(b) = table(|p, s| GetExtendedUdpTable(p, s, false, v4, UDP_TABLE_OWNER_PID, 0))
        {
            for r in rows::<MIB_UDPROW_OWNER_PID>(&b) {
                let ip = IpAddr::V4(Ipv4Addr::from(u32::from_be(r.dwLocalAddr)));
                out.push(RawSocket {
                    proto: Proto::Udp,
                    ip,
                    port: port_of(r.dwLocalPort),
                    pid: r.dwOwningPid,
                });
            }
        }
        if let Some(b) = table(|p, s| GetExtendedUdpTable(p, s, false, v6, UDP_TABLE_OWNER_PID, 0))
        {
            for r in rows::<MIB_UDP6ROW_OWNER_PID>(&b) {
                let ip = IpAddr::V6(Ipv6Addr::from(r.ucLocalAddr));
                out.push(RawSocket {
                    proto: Proto::Udp,
                    ip,
                    port: port_of(r.dwLocalPort),
                    pid: r.dwOwningPid,
                });
            }
        }
    }
    out
}

/// Full scan: sockets, then name / RAM / path once per PID.
pub fn scan() -> Vec<PortEntry> {
    let sockets = dedupe(&raw_sockets());
    let names = process::snapshot_names();
    let mut cache: HashMap<u32, (String, Option<u64>, Option<String>)> = HashMap::new();
    sockets
        .into_iter()
        .map(|s| {
            let (name, memory_mb, path) = cache
                .entry(s.pid)
                .or_insert_with(|| {
                    let (m, p) = process::details(s.pid);
                    (process::name_of(s.pid, &names), m, p)
                })
                .clone();
            PortEntry {
                port: s.port,
                proto: s.proto,
                address: format_addr(s.ip, s.port),
                pid: s.pid,
                name,
                memory_mb,
                path,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(proto: Proto, ip: &str, port: u16, pid: u32) -> RawSocket {
        RawSocket {
            proto,
            ip: ip.parse().unwrap(),
            port,
            pid,
        }
    }

    #[test]
    fn ipv4_wins_over_ipv6() {
        let d = dedupe(&[
            s(Proto::Tcp, "::", 3000, 7),
            s(Proto::Tcp, "0.0.0.0", 3000, 7),
            s(Proto::Udp, "::", 3000, 7),
            s(Proto::Tcp, "::1", 80, 9),
            s(Proto::Tcp, "127.0.0.1", 3000, 8),
        ]);
        assert_eq!(d.len(), 4);
        assert_eq!(d[0], s(Proto::Tcp, "::1", 80, 9));
        assert_eq!(d[1], s(Proto::Tcp, "0.0.0.0", 3000, 7));
        assert_eq!(d[2], s(Proto::Tcp, "127.0.0.1", 3000, 8));
        assert_eq!(d[3], s(Proto::Udp, "::", 3000, 7));
    }

    #[test]
    fn addresses() {
        assert_eq!(
            format_addr("0.0.0.0".parse().unwrap(), 3000),
            "0.0.0.0:3000"
        );
        assert_eq!(format_addr("::".parse().unwrap(), 445), "[::]:445");
    }

    #[test]
    fn real_scan_sees_own_listener() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let t = std::time::Instant::now();
        let all = scan();
        let ms = t.elapsed().as_millis();
        let me = all
            .iter()
            .find(|p| p.port == port && p.proto == Proto::Tcp)
            .expect("own listener missing");
        assert_eq!(me.pid, std::process::id());
        assert!(me.memory_mb.is_some() && me.path.is_some());
        println!("scan: {} rows in {ms} ms", all.len());
    }
}
