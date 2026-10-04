//! macOS network interfaces and per-interface byte/packet counters.
//!
//! Two separate mechanisms are needed:
//!
//! * `getifaddrs(3)` — interface names, IPv4 addresses and hardware
//!   (MAC) addresses. Available unprivileged.
//! * the `NET_RT_IFLIST2` routing sysctl — cumulative traffic counters.
//!   Available unprivileged, and only through raw `sysctl(3)`; the named
//!   `net.route.*` aliases need root, and even the numeric family has no
//!   name binding (`sysctlbyname("net.route.0.0.18.0")` answers `ENOENT`).
//!
//! The equivalent routing-socket request (`write(RTM_IFINFO2)` to a
//! `PF_ROUTE`/`SOCK_RAW` socket) was tried first and is *not* usable here: it
//! returns `EPERM` for an unprivileged process.
//!
//! Two constants in this file look interchangeable and are not:
//! `NET_RT_IFLIST2` is the sysctl index (6) and `RTM_IFINFO2` is the message
//! type inside the returned stream (18). Passing 18 as the index silently
//! yields `EINVAL`. The struct layouts come from the SDK headers
//! (`net/if.h`, `net/if_var.h`) and are pinned by
//! `tests::layout_constants_match_the_sdk`.

use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uint, c_void};

const AF_INET: u8 = 2;
const AF_LINK: u8 = 18;
const IFF_UP: i32 = 0x1;

#[repr(C)]
struct SockaddrIn {
    sin_len: u8,
    sin_family: u8,
    sin_port: u16,
    sin_addr: [u8; 4],
    sin_zero: [u8; 8],
}

#[repr(C)]
struct SockaddrDl {
    sdl_len: u8,
    sdl_family: u8,
    sdl_index: u16,
    sdl_type: u8,
    sdl_nlen: u8,
    sdl_alen: u8,
    sdl_slen: u8,
    sdl_type2: i16,
    sdl_data: [u8; 12],
}

#[repr(C)]
struct Ifaddrs {
    ifa_next: *mut Ifaddrs,
    ifa_name: *mut c_char,
    ifa_flags: c_uint,
    ifa_addr: *mut c_void,
    ifa_netmask: *mut c_void,
    ifa_dstaddr: *mut c_void,
    ifa_data: *mut c_void,
}

unsafe extern "C" {
    fn getifaddrs(ifap: *mut *mut Ifaddrs) -> c_int;
    fn freeifaddrs(ifa: *mut Ifaddrs);
    fn if_nametoindex(ifname: *const c_char) -> c_uint;
    fn sysctl(
        name: *mut c_int,
        namelen: u32,
        oldp: *mut c_void,
        oldlenp: *mut usize,
        newp: *mut c_void,
        newlen: usize,
    ) -> c_int;
}

/// One interface's counters.
#[derive(Debug, Clone, Default)]
pub struct IfStats {
    pub index: u16,
    pub is_up: bool,
    pub mtu: u32,
    pub link_speed_bps: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
}

/// Identity information for one interface, from `getifaddrs`.
#[derive(Debug, Clone, Default)]
pub struct IfIdentity {
    pub name: String,
    pub ipv4: Option<String>,
    pub mac: Option<String>,
}

// ---------------------------------------------------------------------------
// Verified `struct if_data64` layout (Darwin `net/if_var.h`).
// ---------------------------------------------------------------------------

/// `struct if_msghdr2` from `<net/if.h>`.
///
/// `#[repr(C)]` reproduces the two padding bytes the compiler inserts between
/// `ifm_index` (offset 12) and `ifm_snd_len` (offset 16), so `size_of` is
/// exactly `IF_MSGHDR2_DATA_OFFSET`. Declaring the struct rather than reading
/// hand-counted offsets means a layout change is a compile error rather than a
/// silently wrong number in the UI.
#[repr(C)]
#[derive(Clone, Copy)]
struct IfMsghdr2 {
    ifm_msglen: u16,
    ifm_version: u8,
    ifm_type: u8,
    ifm_addrs: i32,
    ifm_flags: i32,
    ifm_index: u16,
    ifm_snd_len: i32,
    ifm_snd_maxlen: i32,
    ifm_snd_drops: i32,
    ifm_timer: i32,
}

/// `struct if_data64` from `<net/if_var.h>` — the full public layout.
///
/// Fields Hamon never reads are still declared, because dropping one would shift
/// the `u_int64_t` counters and silently report the wrong quantity.
/// `tests::layout_constants_match_the_sdk` asserts every offset against the
/// header.
///
/// The header wraps this struct in `#pragma pack(4)`; the natural alignment
/// Rust gives it happens to be identical, which the same test pins down.
///
/// The kernel builds `ifm_msglen` from a *private*, larger `struct if_data`
/// (148 bytes as of macOS 26), so a live record is longer than this prefix.
/// Trailing kernel-only fields are simply not modelled.
#[repr(C)]
#[derive(Clone, Copy)]
struct IfData64 {
    ifi_type: u8,
    ifi_typelen: u8,
    ifi_physical: u8,
    ifi_addrlen: u8,
    ifi_hdrlen: u8,
    ifi_recvquota: u8,
    ifi_xmitquota: u8,
    ifi_unused1: u8,
    ifi_mtu: u32,
    ifi_metric: u32,
    ifi_baudrate: u64,
    ifi_ipackets: u64,
    ifi_ierrors: u64,
    ifi_opackets: u64,
    ifi_oerrors: u64,
    ifi_collisions: u64,
    ifi_ibytes: u64,
    ifi_obytes: u64,
    ifi_imcasts: u64,
    ifi_omcasts: u64,
    ifi_iqdrops: u64,
    ifi_noproto: u64,
    ifi_recvtiming: u32,
    ifi_xmittiming: u32,
    /// `struct IF_DATA_TIMEVAL` is a `timeval32` on LP64, so it is a pair of
    /// `u_int32_t` rather than the usual 64-bit-seconds `timeval`.
    ifi_lastchange_sec: u32,
    ifi_lastchange_usec: u32,
}

/// Where `if_data64` starts inside a message: right after the header.
const IF_MSGHDR2_DATA_OFFSET: usize = std::mem::size_of::<IfMsghdr2>();

/// How many bytes of `if_data64` this module knows how to read.
const IF_DATA64_LEN: usize = std::mem::size_of::<IfData64>();

// ---------------------------------------------------------------------------
// getifaddrs
// ---------------------------------------------------------------------------

/// Frees the list on every exit path.
struct IfaddrsGuard(*mut Ifaddrs);

impl Drop for IfaddrsGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: pointer originated from `getifaddrs`.
            unsafe { freeifaddrs(self.0) };
        }
    }
}

fn with_ifaddrs<T>(f: impl FnOnce(*mut Ifaddrs) -> T) -> Option<T> {
    let mut head: *mut Ifaddrs = std::ptr::null_mut();
    // SAFETY: `head` is a valid out-pointer.
    if unsafe { getifaddrs(&mut head) } != 0 || head.is_null() {
        return None;
    }
    // The guard owns `head` for the rest of the statement, so the list is
    // freed even if `f` panics.
    let guard = IfaddrsGuard(head);
    Some(f(guard.0))
}

/// Identity for every interface, keyed by name.
///
/// `getifaddrs` emits one node per address, so an interface appears several
/// times; nodes are merged and loopback/link-level addresses are filtered
/// out so each interface reports its routable IPv4 address.
pub fn interface_identities() -> HashMap<String, IfIdentity> {
    let mut out: HashMap<String, IfIdentity> = HashMap::new();
    with_ifaddrs(|head| {
        let mut cur = head;
        while !cur.is_null() {
            // SAFETY: `getifaddrs` guarantees a valid linked list; each
            // node's `ifa_name` is NUL-terminated and `ifa_addr` (when
            // non-null) is a `sockaddr` matching its family.
            unsafe {
                let e = &*cur;
                if !e.ifa_name.is_null() {
                    let name = CStr::from_ptr(e.ifa_name).to_string_lossy().into_owned();
                    let entry = out.entry(name.clone()).or_insert_with(|| IfIdentity {
                        name,
                        ..Default::default()
                    });
                    if !e.ifa_addr.is_null() {
                        let family = read_family(e.ifa_addr);
                        if family == AF_INET {
                            let sa = &*(e.ifa_addr as *const SockaddrIn);
                            let ip = format!(
                                "{}.{}.{}.{}",
                                sa.sin_addr[0], sa.sin_addr[1], sa.sin_addr[2], sa.sin_addr[3]
                            );
                            // 127.x is the loopback interface's own address.
                            if entry.ipv4.is_none() || !ip.starts_with("127.") {
                                entry.ipv4 = Some(ip);
                            }
                        } else if family == AF_LINK {
                            let dl = &*(e.ifa_addr as *const SockaddrDl);
                            if dl.sdl_alen == 6 && entry.mac.is_none() {
                                let mac = &dl.sdl_data[..6];
                                if mac.iter().any(|b| *b != 0) {
                                    entry.mac = Some(
                                        mac.iter()
                                            .map(|b| format!("{b:02x}"))
                                            .collect::<Vec<_>>()
                                            .join(":"),
                                    );
                                }
                            }
                        }
                    }
                }
                cur = e.ifa_next;
            }
        }
    });
    out
}

/// Reads `sa_family` from a BSD `sockaddr`.
///
/// # Safety
///
/// `addr` must be a live, non-null `sockaddr` pointer whose second byte is
/// within the allocation.
unsafe fn read_family(addr: *const c_void) -> u8 {
    // `sa_family` is the second byte of every BSD `sockaddr`.
    // SAFETY: upheld by the caller — the pointer came from `ifa_addr`, which
    // `getifaddrs` guarantees points at a complete `sockaddr`.
    unsafe { *addr.cast::<u8>().add(1) }
}

/// Interface names paired with their kernel index, in index order.
pub fn interface_index_map() -> HashMap<String, u16> {
    let mut out = HashMap::new();
    for name in interface_identities().into_keys() {
        let c = std::ffi::CString::new(name.clone()).unwrap_or_default();
        // SAFETY: `c` is a valid NUL-terminated string for the duration of
        // the call; `if_nametoindex` does not retain it.
        let idx = unsafe { if_nametoindex(c.as_ptr()) };
        if idx != 0 {
            out.insert(name, idx as u16);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// NET_RT_IFLIST2
// ---------------------------------------------------------------------------

const CTL_NET: c_int = 4;
/// `AF_ROUTE` / `PF_ROUTE` from `<sys/socket.h>`, address family 17.
const PF_ROUTE: c_int = 17;

/// `NET_RT_IFLIST2` from `<net/route.h>`: the sysctl *index*. Distinct from
/// `RTM_IFINFO2` (18), which is the message type the kernel puts in the stream.
const NET_RT_IFLIST2: c_int = 6;

/// `RTM_IFINFO2`: the only record type in the stream carrying `if_data64`.
/// Other records (addresses, routes) share the variable-length framing and are
/// skipped by length rather than interpreted.
const RTM_IFINFO2: u8 = 0x12;

/// `RTM_VERSION` from `<net/route.h>`. A reply with anything else is not ours.
const RTM_VERSION: u8 = 5;

/// Reads every interface's traffic counters, keyed by interface index.
///
/// Two calls, as `sysctl(3)` requires: the first asks how much space the
/// interface list needs, the second fills a buffer of exactly that size. Doing
/// it in one call with a guessed size works only until the interface set grows.
pub fn net_stats_by_index() -> HashMap<u16, IfStats> {
    let mut out = HashMap::new();
    let Some(buf) = read_iflist() else {
        return out;
    };
    parse_iflist(&buf, &mut out);
    out
}

/// The raw `NET_RT_IFLIST2` byte stream.
fn read_iflist() -> Option<Vec<u8>> {
    // CTL_NET, PF_ROUTE, 0, 0, NET_RT_IFLIST2, 0
    let mut mib = [CTL_NET, PF_ROUTE, 0, 0, NET_RT_IFLIST2, 0];

    // Pass one: ask for the size. `oldp` must be null here, and the result
    // comes back in `len` without any bytes being written.
    let mut len = 0usize;
    // SAFETY: `mib` has the six entries the route handler requires.
    if unsafe {
        sysctl(
            mib.as_mut_ptr(),
            6,
            std::ptr::null_mut(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || len < IF_MSGHDR2_DATA_OFFSET + IF_DATA64_LEN
    {
        return None;
    }

    // Pass two: fill it.
    let mut buf = vec![0u8; len];
    // SAFETY: `buf` is writable for exactly `len` bytes, which is passed in.
    if unsafe {
        sysctl(
            mib.as_mut_ptr(),
            6,
            buf.as_mut_ptr().cast::<c_void>(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return None;
    }
    // The kernel reports what it actually wrote, which may be less than asked.
    buf.truncate(len);
    Some(buf)
}

/// Decodes the message stream into `out`.
///
/// Messages are variable length and framed by `ifm_msglen`, so any record type
/// can be skipped without understanding it — which matters because address and
/// route records are interleaved with the interface ones.
fn parse_iflist(buf: &[u8], out: &mut HashMap<u16, IfStats>) {
    let mut off = 0usize;
    while off + IF_MSGHDR2_DATA_OFFSET <= buf.len() {
        // Records are only `u_short`-aligned, so an aligned reference would be
        // undefined; `read_unaligned` copies the header out instead.
        // SAFETY: `off + IF_MSGHDR2_DATA_OFFSET <= buf.len()` is the loop
        // condition, and `size_of::<IfMsghdr2>() == IF_MSGHDR2_DATA_OFFSET`.
        let header: IfMsghdr2 = unsafe { read_unaligned_at(buf, off) };
        let msglen = usize::from(header.ifm_msglen);
        // A record must at least carry its own header and must fit inside the
        // buffer. Address and route records are legitimately shorter than
        // `if_msghdr2 + if_data64`, so only the header sets the floor here —
        // requiring the full payload would stop the walk at the first address
        // record and yield exactly one interface.
        if msglen < IF_MSGHDR2_DATA_OFFSET || off + msglen > buf.len() {
            return;
        }
        // `ifm_version` guards against a kernel speaking a newer dialect and
        // `ifm_type` against reading an address record as statistics. A record
        // whose declared length cannot hold the counters is skipped too.
        if msglen >= IF_MSGHDR2_DATA_OFFSET + IF_DATA64_LEN
            && header.ifm_version == RTM_VERSION
            && header.ifm_type == RTM_IFINFO2
        {
            // SAFETY: the length check above guarantees `msglen`, and so the
            // remaining buffer, covers a whole `IfData64`.
            let data: IfData64 = unsafe { read_unaligned_at(buf, off + IF_MSGHDR2_DATA_OFFSET) };
            out.insert(
                header.ifm_index,
                IfStats {
                    index: header.ifm_index,
                    is_up: header.ifm_flags & IFF_UP != 0,
                    mtu: data.ifi_mtu,
                    link_speed_bps: data.ifi_baudrate,
                    rx_bytes: data.ifi_ibytes,
                    tx_bytes: data.ifi_obytes,
                    rx_packets: data.ifi_ipackets,
                    tx_packets: data.ifi_opackets,
                    rx_errors: data.ifi_ierrors,
                    tx_errors: data.ifi_oerrors,
                },
            );
        }
        off += msglen;
    }
}

/// Copies a `T` out of `buf` at `off`, tolerating any alignment.
///
/// # Safety
///
/// Callers must guarantee `off + size_of::<T>() <= buf.len()` and that `T`
/// matches the kernel's layout. Every call site checks the former first.
unsafe fn read_unaligned_at<T: Copy>(buf: &[u8], off: usize) -> T {
    debug_assert!(off + std::mem::size_of::<T>() <= buf.len());
    // SAFETY: upheld by the caller; `read_unaligned` is alignment-agnostic, and
    // the byte range was bounds-checked above.
    unsafe { std::ptr::read_unaligned(buf.as_ptr().add(off).cast::<T>()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a syntactically valid `RTM_IFINFO2` message for parser tests.
    fn message(index: u16, version: u8, msg_type: u8, len: usize) -> Vec<u8> {
        let mut m = vec![0u8; len];
        let msglen = len as u16;
        m[0..2].copy_from_slice(&msglen.to_le_bytes());
        m[2] = version;
        m[3] = msg_type;
        m[12..14].copy_from_slice(&index.to_le_bytes());
        m
    }

    #[test]
    fn layout_constants_match_the_sdk() {
        // Pinned against `<net/if.h>` and `<net/if_var.h>`. If a field is
        // removed or reordered above, this fails rather than the UI quietly
        // reporting the wrong counter.
        assert_eq!(IF_MSGHDR2_DATA_OFFSET, 32);
        assert_eq!(IF_DATA64_LEN, 128);
        assert_eq!(std::mem::offset_of!(IfMsghdr2, ifm_flags), 8);
        assert_eq!(std::mem::offset_of!(IfMsghdr2, ifm_index), 12);
        assert_eq!(std::mem::offset_of!(IfData64, ifi_mtu), 8);
        assert_eq!(std::mem::offset_of!(IfData64, ifi_baudrate), 16);
        assert_eq!(std::mem::offset_of!(IfData64, ifi_ipackets), 24);
        assert_eq!(std::mem::offset_of!(IfData64, ifi_ierrors), 32);
        assert_eq!(std::mem::offset_of!(IfData64, ifi_opackets), 40);
        assert_eq!(std::mem::offset_of!(IfData64, ifi_oerrors), 48);
        assert_eq!(std::mem::offset_of!(IfData64, ifi_ibytes), 64);
        assert_eq!(std::mem::offset_of!(IfData64, ifi_obytes), 72);
        assert_eq!(std::mem::offset_of!(IfData64, ifi_imcasts), 80);
        assert_eq!(std::mem::offset_of!(IfData64, ifi_noproto), 104);
        // `#pragma pack(4)` means the `u_int32_t` tail is not padded up to 8.
        assert_eq!(std::mem::offset_of!(IfData64, ifi_recvtiming), 112);
        assert_eq!(std::mem::offset_of!(IfData64, ifi_lastchange_sec), 120);
    }

    #[test]
    fn only_well_formed_ifinfo2_records_are_read() {
        let full = IF_MSGHDR2_DATA_OFFSET + IF_DATA64_LEN;
        let mut out = HashMap::new();

        // Right length, wrong version: a newer kernel dialect, not ours.
        parse_iflist(&message(7, RTM_VERSION - 1, RTM_IFINFO2, full), &mut out);
        assert!(out.is_empty(), "wrong version was accepted");

        // Right version, wrong record type: an address record must not be
        // decoded as statistics.
        parse_iflist(&message(7, RTM_VERSION, 0xc, full), &mut out);
        assert!(out.is_empty(), "non-IFINFO2 record was accepted");

        // `RTM_IFINFO2` whose declared length cannot hold the counters.
        parse_iflist(
            &message(7, RTM_VERSION, RTM_IFINFO2, IF_MSGHDR2_DATA_OFFSET),
            &mut out,
        );
        assert!(out.is_empty(), "truncated record was accepted");

        // Zero `ifm_msglen` would otherwise spin forever.
        let mut spin = message(7, RTM_VERSION, RTM_IFINFO2, full);
        spin[0..2].copy_from_slice(&0u16.to_le_bytes());
        parse_iflist(&spin, &mut out);
        assert!(out.is_empty(), "zero-length record was accepted");

        // A record length past the end of the buffer truncates the walk rather
        // than reading out of bounds.
        let mut over = message(7, RTM_VERSION, RTM_IFINFO2, full);
        over[0..2].copy_from_slice(&(full as u16 + 1000).to_le_bytes());
        parse_iflist(&over, &mut out);
        assert!(out.is_empty(), "over-long record was accepted");

        // Finally, a good record.
        parse_iflist(&message(7, RTM_VERSION, RTM_IFINFO2, full), &mut out);
        assert_eq!(out[&7].index, 7);
    }

    #[test]
    fn records_after_an_unknown_one_are_still_found() {
        // Address and route records are interleaved with interface ones, and are
        // *shorter* than `if_msghdr2 + if_data64`; the walk must skip them by
        // length rather than stop at the first one.
        let full = IF_MSGHDR2_DATA_OFFSET + IF_DATA64_LEN;
        let mut stream = message(3, RTM_VERSION, 0xc, 60); // RTM_NEWADDR
        stream.extend(message(5, RTM_VERSION, RTM_IFINFO2, full));
        stream.extend(message(9, RTM_VERSION, 0x4, 48)); // RTM_ADD
        stream.extend(message(11, RTM_VERSION, RTM_IFINFO2, full));
        let mut out = HashMap::new();
        parse_iflist(&stream, &mut out);
        assert_eq!(
            out.keys()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            [5, 11].into()
        );
    }

    #[test]
    fn every_interface_on_this_machine_is_reported() {
        // The real stream leads with one `RTM_IFINFO2` record per interface,
        // each followed by its address records, so anything beyond the first
        // interface is only found if the short records are stepped over.
        let stats = net_stats_by_index();
        let identities = interface_index_map();
        assert!(
            stats.len() >= 2,
            "only {} interface(s) parsed out of {} known interfaces: {stats:?}",
            stats.len(),
            identities.len()
        );
        // Every parsed index must correspond to a real interface.
        for index in stats.keys() {
            assert!(
                identities.values().any(|i| *i == *index),
                "if index {index} does not exist"
            );
        }
    }

    #[test]
    fn interface_identities_include_loopback() {
        let ids = interface_identities();
        let lo = ids.get("lo0").expect("lo0 should exist on macOS");
        assert_eq!(lo.ipv4.as_deref(), Some("127.0.0.1"));
        assert!(lo.mac.is_none(), "loopback has no hardware address");
    }

    #[test]
    fn indices_are_assigned_to_real_interfaces() {
        let map = interface_index_map();
        assert!(map.contains_key("lo0"));
        assert!(map.get("lo0").copied().unwrap() >= 1);
    }

    #[test]
    fn counters_move_for_a_live_interface() {
        let stats = net_stats_by_index();
        assert!(!stats.is_empty(), "no interface stats returned");
        let lo = stats.get(&1).expect("lo0 is index 1");
        assert!(lo.is_up, "lo0 must always be up");
        assert!(lo.rx_packets > 0, "loopback should have received packets");
        // Loopback in == loopback out on a quiet machine.
        assert_eq!(lo.rx_bytes, lo.tx_bytes);
    }

    #[test]
    fn a_down_link_moves_no_traffic() {
        // Guards against misreading `ifm_flags` at the wrong offset: a link that
        // is down cannot pass bytes.
        //
        // The invariant is on the *rate*, not on the counters. A down interface
        // keeps the totals it accumulated while it was up, so requiring zero
        // here would fail on any machine that has ever had a VPN or a USB
        // tether up. Two reads a moment apart pin down what actually matters.
        let first = net_stats_by_index();
        let second = net_stats_by_index();
        let mut checked = 0;
        for (index, a) in &first {
            let Some(b) = second.get(index) else { continue };
            // Only compare interfaces down in *both* reads; one that came up in
            // between is legitimately carrying traffic.
            if a.is_up || b.is_up {
                continue;
            }
            assert_eq!(
                (b.rx_bytes, b.tx_bytes),
                (a.rx_bytes, a.tx_bytes),
                "if_{index} is down but its counters moved"
            );
            checked += 1;
        }
        assert!(
            checked > 0,
            "no down interfaces found; is the flag offset wrong?"
        );
    }
}
