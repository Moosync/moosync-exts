use std::ffi::{c_int, c_void};
use std::mem::{size_of, zeroed};
use std::ptr;

use crate::shm::{
    Cmsghdr, Iovec, Msghdr, SCM_RIGHTS, SOL_SOCKET, SockaddrUn, cmsg_align, cmsg_space,
};

pub const MSG_NOSIGNAL: c_int = 0x4000;
pub const POLLIN: i16 = 0x0001;
pub const EINVAL: c_int = 22;
pub const ENAMETOOLONG: c_int = 36;
pub const EIO: c_int = 5;

#[repr(C)]
pub struct PollFd {
    pub fd: c_int,
    pub events: i16,
    pub revents: i16,
}

unsafe extern "C" {
    pub fn socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int;
    pub fn connect(sockfd: c_int, addr: *const SockaddrUn, addrlen: u32) -> c_int;
    pub fn send(sockfd: c_int, buf: *const c_void, len: usize, flags: c_int) -> isize;
    pub fn recvmsg(sockfd: c_int, msg: *mut Msghdr, flags: c_int) -> isize;
    pub fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
    pub fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    pub fn poll(fds: *mut PollFd, nfds: usize, timeout: c_int) -> c_int;
    pub fn eventfd(initval: u32, flags: c_int) -> c_int;
    pub fn close(fd: c_int) -> c_int;
    pub fn mmap(
        addr: *mut c_void,
        length: usize,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: isize,
    ) -> *mut c_void;
    pub fn munmap(addr: *mut c_void, length: usize) -> c_int;
    pub fn __errno_location() -> *mut c_int;
}

/// Receives the shared-memory fd and optional wake `eventfd` from `aaudio_sink` via `SCM_RIGHTS`.
pub unsafe fn recv_shm_fds(sock_fd: c_int) -> (c_int, c_int) {
    let mut byte: u8 = 0;
    let mut iov = Iovec {
        iov_base: (&mut byte as *mut u8).cast::<c_void>(),
        iov_len: 1,
    };
    let mut cmsg_buf = [0u8; cmsg_space(size_of::<[c_int; 2]>())];
    // SAFETY: `Msghdr` is a plain C POD struct of zeros before field initialization.
    let mut msg: Msghdr = unsafe { zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cmsg_buf.as_mut_ptr().cast::<c_void>();
    msg.msg_controllen = cmsg_buf.len();

    // SAFETY: `sock_fd` is a connected AF_UNIX socket and `msg` points to valid stack buffers.
    let n = unsafe { recvmsg(sock_fd, &mut msg, 0) };
    if n <= 0 || msg.msg_controllen < size_of::<Cmsghdr>() {
        return (-1, -1);
    }
    // SAFETY: `msg.msg_control` is aligned and at least `size_of::<Cmsghdr>()` bytes.
    let cmsg = unsafe { &*(msg.msg_control as *const Cmsghdr) };
    if cmsg.cmsg_level == SOL_SOCKET && cmsg.cmsg_type == SCM_RIGHTS {
        let header_len = cmsg_align(size_of::<Cmsghdr>());
        let payload_len = cmsg.cmsg_len.saturating_sub(header_len);
        // SAFETY: `data_ptr` lies within `cmsg_buf` after the `Cmsghdr` header.
        let data_ptr = unsafe { (msg.msg_control as *const u8).add(header_len) };
        if payload_len >= size_of::<[c_int; 2]>() {
            let mut fds = [-1 as c_int; 2];
            // SAFETY: `payload_len >= size_of::<[c_int; 2]>()`, so reading two `c_int`s is in bounds.
            unsafe {
                ptr::copy_nonoverlapping(
                    data_ptr,
                    fds.as_mut_ptr().cast::<u8>(),
                    size_of::<[c_int; 2]>(),
                );
            }
            return (fds[0], fds[1]);
        }
        if payload_len >= size_of::<c_int>() {
            let mut fd: c_int = -1;
            // SAFETY: `payload_len >= size_of::<c_int>()`, so reading one `c_int` is in bounds.
            unsafe {
                ptr::copy_nonoverlapping(
                    data_ptr,
                    (&mut fd as *mut c_int).cast::<u8>(),
                    size_of::<c_int>(),
                );
            }
            return (fd, -1);
        }
    }
    (-1, -1)
}
