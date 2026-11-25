use std::ffi::{c_char, c_int, c_void};
use std::mem::{size_of, zeroed};

pub const PW_AAUDIO_SHM_MAGIC: u32 = 0x5057_4141; // "PWAA"
pub const PW_AAUDIO_SHM_VERSION: u32 = 1;
pub const PW_SHM_RING_BYTES: usize = 256 * 1024; // 256 KiB power-of-two ring

pub const PW_AAUDIO_FMT_UNKNOWN: u32 = 0;
pub const PW_AAUDIO_FMT_S16_LE: u32 = 1;
pub const PW_AAUDIO_FMT_F32_LE: u32 = 2;

pub const PW_AAUDIO_CMD_CONFIG: u8 = b'C';
pub const PW_AAUDIO_CMD_ACTIVE: u8 = b'A';
pub const PW_AAUDIO_CMD_FLUSH: u8 = b'F';
pub const PW_AAUDIO_CMD_VOLUME: u8 = b'V';
pub const PW_AAUDIO_CMD_QUIT: u8 = b'Q';

pub const AF_UNIX: c_int = 1;
pub const SOCK_STREAM: c_int = 1;
pub const SOCK_CLOEXEC: c_int = 0o2000000;
pub const SOL_SOCKET: c_int = 1;
pub const SCM_RIGHTS: c_int = 1;
pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;
pub const MAP_SHARED: c_int = 1;
pub const EFD_NONBLOCK: c_int = 0o4000;
pub const EFD_CLOEXEC: c_int = 0o2000000;

#[repr(C)]
pub struct PwAaudioShm {
    pub magic: u32,
    pub version: u32,
    pub read_idx: u32,
    pub write_idx: u32,
    pub sample_rate: u32,
    pub channels: u32,
    pub format: u32,
    pub frame_size: u32,
    pub volume_q16: u32,
    pub active: u32,
    pub flush_seq: u32,
    pub ring_bytes: u32,
    pub data: [u8; PW_SHM_RING_BYTES],
}

#[repr(C)]
pub struct Iovec {
    pub iov_base: *mut c_void,
    pub iov_len: usize,
}

#[repr(C)]
pub struct Msghdr {
    pub msg_name: *mut c_void,
    pub msg_namelen: u32,
    pub msg_iov: *mut Iovec,
    pub msg_iovlen: usize,
    pub msg_control: *mut c_void,
    pub msg_controllen: usize,
    pub msg_flags: c_int,
}

#[repr(C)]
pub struct Cmsghdr {
    pub cmsg_len: usize,
    pub cmsg_level: c_int,
    pub cmsg_type: c_int,
}

#[repr(C)]
pub struct SockaddrUn {
    pub sun_family: u16,
    pub sun_path: [c_char; 108],
}

pub const fn cmsg_align(len: usize) -> usize {
    (len + size_of::<usize>() - 1) & !(size_of::<usize>() - 1)
}

pub const fn cmsg_space(len: usize) -> usize {
    cmsg_align(len) + cmsg_align(size_of::<Cmsghdr>())
}

/// Builds a `SockaddrUn` and its `addrlen` for filesystem or `@abstract` Unix socket paths.
pub fn build_unix_sockaddr(path: &str) -> Option<(SockaddrUn, u32)> {
    if path.is_empty() || path.len() >= 108 {
        return None;
    }
    // SAFETY: `SockaddrUn` is a POD struct of zeros before populating `sun_family` and `sun_path`.
    let mut addr: SockaddrUn = unsafe { zeroed() };
    addr.sun_family = AF_UNIX as u16;
    for (i, b) in path.bytes().enumerate() {
        addr.sun_path[i] = if i == 0 && b == b'@' { 0 } else { b as c_char };
    }
    let addrlen = if path.starts_with('@') {
        (2 + path.len()) as u32
    } else {
        (2 + path.len() + 1) as u32
    };
    Some((addr, addrlen))
}
