use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::mem::size_of;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::process::Command;
use std::ptr;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::Duration;

use simple_dns::rdata::RData;
use simple_dns::{CLASS, Name, Packet, PacketFlag, Question, TYPE};

const AF_INET: c_int = 2;
const AF_INET6: c_int = 10;
const SOCK_STREAM: c_int = 1;
const IPPROTO_TCP: c_int = 6;
const AI_NUMERICHOST: c_int = 0x0004;
const EAI_NONAME: c_int = -2;
const EAI_FAIL: c_int = -4;
const RTLD_NEXT: *mut c_void = (-1isize) as *mut c_void;

#[repr(C)]
pub struct AddrInfo {
    pub ai_flags: c_int,
    pub ai_family: c_int,
    pub ai_socktype: c_int,
    pub ai_protocol: c_int,
    pub ai_addrlen: u32,
    pub ai_addr: *mut SockAddrIn,
    pub ai_canonname: *mut c_char,
    pub ai_next: *mut AddrInfo,
}

#[repr(C)]
pub struct SockAddrIn {
    pub sin_family: u16,
    pub sin_port: u16,
    pub sin_addr: u32,
    pub sin_zero: [u8; 8],
}

#[repr(C)]
struct OwnedAddrInfo {
    info: AddrInfo,
    addr: SockAddrIn,
}

// `OwnedAddrInfo` is only ever read/dropped via `OWN_ALLOCS` after C finishes with `res`.
// SAFETY: `OwnedAddrInfo` contains plain C POD structs and pointers into its own heap allocation.
unsafe impl Send for OwnedAddrInfo {}

type GetAddrInfoFn = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    *const AddrInfo,
    *mut *mut AddrInfo,
) -> c_int;
type FreeAddrInfoFn = unsafe extern "C" fn(*mut AddrInfo);

unsafe extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

static OWN_ALLOCS: Mutex<Option<HashMap<usize, Vec<Box<OwnedAddrInfo>>>>> = Mutex::new(None);

fn store_owned_alloc(head: usize, nodes: Vec<Box<OwnedAddrInfo>>) {
    let mut guard = OWN_ALLOCS.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(head, nodes);
}

fn remove_owned_alloc(ptr: *mut AddrInfo) -> bool {
    let mut guard = OWN_ALLOCS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(map) = guard.as_mut() {
        return map.remove(&(ptr as usize)).is_some();
    }
    false
}

fn parse_port(service: *const c_char) -> u16 {
    if service.is_null() {
        return 0;
    }
    // SAFETY: Caller guarantees `service` is either null (checked above) or a valid NUL-terminated C string.
    let Ok(s) = (unsafe { CStr::from_ptr(service) }).to_str() else {
        return 0;
    };
    if let Ok(port) = s.parse::<u16>() {
        return port;
    }
    match s {
        "https" => 443,
        "http" => 80,
        _ => 0,
    }
}

fn system_dns_servers() -> Vec<SocketAddr> {
    let mut servers = Vec::new();
    if let Ok(contents) = std::fs::read_to_string("/etc/resolv.conf") {
        for line in contents.lines() {
            let trimmed = line.split('#').next().unwrap_or("").trim();
            if let Some(rest) = trimmed.strip_prefix("nameserver") {
                if let Ok(ip) = IpAddr::from_str(rest.trim()) {
                    servers.push(SocketAddr::new(ip, 53));
                }
            }
        }
    }
    for prop in ["net.dns1", "net.dns2", "net.dns3", "net.dns4"] {
        if let Ok(output) = Command::new("/system/bin/getprop").arg(prop).output() {
            if output.status.success() {
                let val = String::from_utf8_lossy(&output.stdout);
                if let Ok(ip) = IpAddr::from_str(val.trim()) {
                    let addr = SocketAddr::new(ip, 53);
                    if !servers.contains(&addr) {
                        servers.push(addr);
                    }
                }
            }
        }
    }
    servers
}

pub fn build_dns_query(hostname: &str, tx_id: u16) -> Option<Vec<u8>> {
    let mut packet = Packet::new_query(tx_id);
    packet.set_flags(PacketFlag::RECURSION_DESIRED);
    let qname = Name::new(hostname.trim_end_matches('.')).ok()?;
    packet.questions.push(Question::new(
        qname,
        TYPE::A.into(),
        CLASS::IN.into(),
        false,
    ));
    packet.build_bytes_vec().ok()
}

pub fn parse_dns_a_records(bytes: &[u8], expected_tx_id: u16) -> Vec<Ipv4Addr> {
    let Ok(packet) = Packet::parse(bytes) else {
        return Vec::new();
    };
    if packet.id() != expected_tx_id {
        return Vec::new();
    }
    packet
        .answers
        .iter()
        .filter_map(|rr| match &rr.rdata {
            RData::A(a) => Some(Ipv4Addr::from(a.address)),
            _ => None,
        })
        .collect()
}

fn resolve_ipv4_via_system_dns(hostname: &str) -> Vec<Ipv4Addr> {
    let tx_id: u16 = 0x4D53;
    let Some(query) = build_dns_query(hostname, tx_id) else {
        return Vec::new();
    };
    let servers = system_dns_servers();
    if servers.is_empty() {
        return Vec::new();
    }

    let Ok(sock) = UdpSocket::bind("0.0.0.0:0") else {
        return Vec::new();
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(1500)));

    let mut buf = [0u8; 512];
    for server in servers {
        if sock.send_to(&query, server).is_err() {
            continue;
        }
        if let Ok((n, _)) = sock.recv_from(&mut buf) {
            let addrs = parse_dns_a_records(&buf[..n], tx_id);
            if !addrs.is_empty() {
                return addrs;
            }
        }
    }
    Vec::new()
}

fn call_next_getaddrinfo(
    node: *const c_char,
    service: *const c_char,
    hints: *const AddrInfo,
    res: *mut *mut AddrInfo,
) -> c_int {
    // SAFETY: `RTLD_NEXT` and static NUL-terminated `"getaddrinfo"` are valid `dlsym` arguments.
    let sym = unsafe { dlsym(RTLD_NEXT, c"getaddrinfo".as_ptr()) };
    if sym.is_null() {
        return EAI_FAIL;
    }
    // SAFETY: `dlsym` cannot type-check C symbols at runtime; we checked non-null and rely on POSIX libc `getaddrinfo` ABI.
    let next: GetAddrInfoFn = unsafe { std::mem::transmute(sym) };
    // SAFETY: Caller's `node`, `service`, `hints`, `res` pointers are forwarded unchanged to libc.
    unsafe { next(node, service, hints, res) }
}

/// Intercepts `getaddrinfo` for glibc on Android, resolving hostnames via system DNS servers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn getaddrinfo(
    node: *const c_char,
    service: *const c_char,
    hints: *const AddrInfo,
    res: *mut *mut AddrInfo,
) -> c_int {
    if res.is_null() {
        return EAI_FAIL;
    }
    // SAFETY: Caller guarantees `hints` is either null or points to a valid `AddrInfo`.
    let (flags, family, hint_socktype, hint_protocol) = if hints.is_null() {
        (0, 0, 0, 0)
    } else {
        let h = unsafe { &*hints };
        (h.ai_flags, h.ai_family, h.ai_socktype, h.ai_protocol)
    };
    if node.is_null() || (flags & AI_NUMERICHOST) != 0 || family == AF_INET6 {
        return call_next_getaddrinfo(node, service, hints, res);
    }

    // SAFETY: Caller guarantees `node` is non-null (checked above) and NUL-terminated.
    let Ok(host) = (unsafe { CStr::from_ptr(node) }).to_str() else {
        return call_next_getaddrinfo(node, service, hints, res);
    };
    if host.is_empty()
        || host.eq_ignore_ascii_case("localhost")
        || Ipv4Addr::from_str(host).is_ok()
        || Ipv6Addr::from_str(host).is_ok()
    {
        return call_next_getaddrinfo(node, service, hints, res);
    }

    let addrs = resolve_ipv4_via_system_dns(host);
    if addrs.is_empty() {
        return EAI_NONAME;
    }

    let port = parse_port(service);
    let socktype = if hint_socktype != 0 { hint_socktype } else { SOCK_STREAM };
    let protocol = if hint_protocol != 0 { hint_protocol } else { IPPROTO_TCP };

    let mut nodes: Vec<Box<OwnedAddrInfo>> = addrs
        .into_iter()
        .map(|ip| {
            let mut node_box = Box::new(OwnedAddrInfo {
                info: AddrInfo {
                    ai_flags: 0,
                    ai_family: AF_INET,
                    ai_socktype: socktype,
                    ai_protocol: protocol,
                    ai_addrlen: size_of::<SockAddrIn>() as u32,
                    ai_addr: ptr::null_mut(),
                    ai_canonname: ptr::null_mut(),
                    ai_next: ptr::null_mut(),
                },
                addr: SockAddrIn {
                    sin_family: AF_INET as u16,
                    sin_port: port.to_be(),
                    sin_addr: u32::from_ne_bytes(ip.octets()),
                    sin_zero: [0; 8],
                },
            });
            node_box.info.ai_addr = &raw mut node_box.addr;
            node_box
        })
        .collect();

    if nodes.is_empty() {
        return EAI_FAIL;
    }
    for i in 0..nodes.len() - 1 {
        let next_ptr: *mut AddrInfo = &raw mut nodes[i + 1].info;
        nodes[i].info.ai_next = next_ptr;
    }
    let head: *mut AddrInfo = &raw mut nodes[0].info;
    store_owned_alloc(head as usize, nodes);
    // SAFETY: Caller guarantees `res` is a valid non-null out-pointer.
    unsafe { *res = head };
    0
}

/// Releases `addrinfo` lists allocated by this shim or delegates to libc `freeaddrinfo`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn freeaddrinfo(res: *mut AddrInfo) {
    if res.is_null() {
        return;
    }
    if remove_owned_alloc(res) {
        return;
    }
    // SAFETY: `RTLD_NEXT` and static NUL-terminated `"freeaddrinfo"` are valid `dlsym` arguments.
    let sym = unsafe { dlsym(RTLD_NEXT, c"freeaddrinfo".as_ptr()) };
    if !sym.is_null() {
        // SAFETY: `dlsym` cannot type-check C symbols at runtime; we checked non-null and rely on POSIX libc `freeaddrinfo` ABI.
        let next: FreeAddrInfoFn = unsafe { std::mem::transmute(sym) };
        // SAFETY: `res` was allocated by libc's `getaddrinfo`, so forwarding to libc `freeaddrinfo` is valid.
        unsafe { next(res) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple_dns::ResourceRecord;
    use simple_dns::rdata::A;

    #[test]
    fn test_parse_port_variants() {
        assert_eq!(parse_port(ptr::null()), 0);
        assert_eq!(parse_port(c"8080".as_ptr()), 8080);
        assert_eq!(parse_port(c"https".as_ptr()), 443);
        assert_eq!(parse_port(c"http".as_ptr()), 80);
        assert_eq!(parse_port(c"unknown".as_ptr()), 0);
    }

    #[test]
    fn test_build_dns_query_and_parse_a_records() {
        let query = build_dns_query("api.spotify.com.", 0x1234).unwrap();
        let req = Packet::parse(&query).unwrap();
        assert_eq!(req.id(), 0x1234);
        assert_eq!(req.questions.len(), 1);

        let mut resp = Packet::new_reply(0x1234);
        resp.answers.push(ResourceRecord::new(
            Name::new("api.spotify.com").unwrap(),
            CLASS::IN,
            60,
            RData::A(A {
                address: u32::from(Ipv4Addr::new(35, 186, 224, 25)),
            }),
        ));
        let bytes = resp.build_bytes_vec().unwrap();

        assert_eq!(
            parse_dns_a_records(&bytes, 0x1234),
            vec![Ipv4Addr::new(35, 186, 224, 25)]
        );
        assert!(parse_dns_a_records(&bytes, 0x9999).is_empty());
        assert!(parse_dns_a_records(&[0, 1, 2], 0x1234).is_empty());
    }

    #[test]
    fn test_getaddrinfo_numeric_and_null_res() {
        assert_eq!(
            unsafe { getaddrinfo(c"127.0.0.1".as_ptr(), ptr::null(), ptr::null(), ptr::null_mut()) },
            EAI_FAIL
        );

        let mut res: *mut AddrInfo = ptr::null_mut();
        let hints = AddrInfo {
            ai_flags: AI_NUMERICHOST,
            ai_family: AF_INET,
            ai_socktype: SOCK_STREAM,
            ai_protocol: IPPROTO_TCP,
            ai_addrlen: 0,
            ai_addr: ptr::null_mut(),
            ai_canonname: ptr::null_mut(),
            ai_next: ptr::null_mut(),
        };
        let rc = unsafe { getaddrinfo(c"127.0.0.1".as_ptr(), c"443".as_ptr(), &hints, &mut res) };
        assert_eq!(rc, 0);
        assert!(!res.is_null());
        unsafe { freeaddrinfo(res) };
    }
}
