mod ring;

use std::ffi::{CString, c_char, c_int, c_void};
use std::mem::size_of;
use std::process::ExitCode;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use pw_aaudio_shm::{
    AF_UNIX, EFD_CLOEXEC, EFD_NONBLOCK, MAP_SHARED, PROT_READ, PROT_WRITE, PW_AAUDIO_CMD_ACTIVE,
    PW_AAUDIO_CMD_CONFIG, PW_AAUDIO_CMD_QUIT, PW_AAUDIO_FMT_F32_LE, PW_AAUDIO_SHM_MAGIC,
    PW_AAUDIO_SHM_VERSION, PW_SHM_RING_BYTES, PwAaudioShm, SOCK_CLOEXEC, SOCK_STREAM, SockaddrUn,
    build_unix_sockaddr,
};
use ring::{consume_ring_to_buf, send_shm_fds};

#[cfg(target_arch = "aarch64")]
const SYS_MEMFD_CREATE: i64 = 279;
#[cfg(target_arch = "x86_64")]
const SYS_MEMFD_CREATE: i64 = 319;

const AAUDIO_OK: i32 = 0;
const AAUDIO_DIRECTION_OUTPUT: i32 = 0;
const AAUDIO_FORMAT_PCM_I16: i32 = 1;
const AAUDIO_FORMAT_PCM_FLOAT: i32 = 2;
const AAUDIO_SHARING_MODE_SHARED: i32 = 1;
const AAUDIO_PERFORMANCE_MODE_LOW_LATENCY: i32 = 12;
const AAUDIO_USAGE_MEDIA: i32 = 1;
const AAUDIO_CONTENT_TYPE_MUSIC: i32 = 2;
const AAUDIO_CALLBACK_RESULT_CONTINUE: i32 = 0;

type AAudioStream = c_void;
type AAudioStreamBuilder = c_void;

type AAudioDataCallback = unsafe extern "C" fn(
    stream: *mut AAudioStream,
    user_data: *mut c_void,
    audio_data: *mut c_void,
    num_frames: i32,
) -> i32;

type AAudioErrorCallback =
    unsafe extern "C" fn(stream: *mut AAudioStream, user_data: *mut c_void, error: i32);

unsafe extern "C" {
    fn syscall(number: i64, ...) -> i64;
    fn ftruncate(fd: c_int, length: isize) -> c_int;
    fn mmap(
        addr: *mut c_void,
        length: usize,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: isize,
    ) -> *mut c_void;
    fn socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int;
    fn bind(sockfd: c_int, addr: *const SockaddrUn, addrlen: u32) -> c_int;
    fn listen(sockfd: c_int, backlog: c_int) -> c_int;
    fn accept(sockfd: c_int, addr: *mut c_void, addrlen: *mut u32) -> c_int;
    fn recv(sockfd: c_int, buf: *mut c_void, len: usize, flags: c_int) -> isize;
    fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
    fn eventfd(initval: u32, flags: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn unlink(pathname: *const c_char) -> c_int;

    fn AAudio_createStreamBuilder(builder: *mut *mut AAudioStreamBuilder) -> i32;
    fn AAudioStreamBuilder_delete(builder: *mut AAudioStreamBuilder) -> i32;
    fn AAudioStreamBuilder_openStream(
        builder: *mut AAudioStreamBuilder,
        stream: *mut *mut AAudioStream,
    ) -> i32;
    fn AAudioStreamBuilder_setDirection(builder: *mut AAudioStreamBuilder, direction: i32);
    fn AAudioStreamBuilder_setSharingMode(builder: *mut AAudioStreamBuilder, sharing_mode: i32);
    fn AAudioStreamBuilder_setPerformanceMode(builder: *mut AAudioStreamBuilder, mode: i32);
    fn AAudioStreamBuilder_setUsage(builder: *mut AAudioStreamBuilder, usage: i32);
    fn AAudioStreamBuilder_setContentType(builder: *mut AAudioStreamBuilder, content_type: i32);
    fn AAudioStreamBuilder_setSampleRate(builder: *mut AAudioStreamBuilder, sample_rate: i32);
    fn AAudioStreamBuilder_setChannelCount(builder: *mut AAudioStreamBuilder, channel_count: i32);
    fn AAudioStreamBuilder_setFormat(builder: *mut AAudioStreamBuilder, format: i32);
    fn AAudioStreamBuilder_setDataCallback(
        builder: *mut AAudioStreamBuilder,
        callback: AAudioDataCallback,
        user_data: *mut c_void,
    );
    fn AAudioStreamBuilder_setErrorCallback(
        builder: *mut AAudioStreamBuilder,
        callback: AAudioErrorCallback,
        user_data: *mut c_void,
    );
    fn AAudioStream_requestStart(stream: *mut AAudioStream) -> i32;
    fn AAudioStream_requestStop(stream: *mut AAudioStream) -> i32;
    fn AAudioStream_close(stream: *mut AAudioStream) -> i32;
}

struct SinkState {
    shm: *mut PwAaudioShm,
    stream: *mut AAudioStream,
    wake_efd: c_int,
    last_flush_seq: u32,
}

unsafe extern "C" fn aaudio_data_cb(
    _stream: *mut AAudioStream,
    user_data: *mut c_void,
    audio_data: *mut c_void,
    num_frames: i32,
) -> i32 {
    // SAFETY: `user_data` is `&mut SinkState` passed to `AAudioStreamBuilder_setDataCallback`, alive for stream lifetime.
    let st = unsafe { &mut *(user_data as *mut SinkState) };
    // SAFETY: `st.shm` is a valid shared-memory mapping initialized before opening the stream.
    let shm = unsafe { &mut *st.shm };
    let frame_size = (shm.frame_size as usize).max(4);
    let want_bytes = (num_frames.max(0) as usize) * frame_size;
    // SAFETY: AAudio guarantees `audio_data` is writable for `num_frames * frame_size` bytes during this callback.
    let out = unsafe { std::slice::from_raw_parts_mut(audio_data.cast::<u8>(), want_bytes) };

    if consume_ring_to_buf(shm, out, &mut st.last_flush_seq) && st.wake_efd >= 0 {
        let one: u64 = 1;
        // SAFETY: `st.wake_efd` is a valid `EFD_NONBLOCK` eventfd and `&one` is an 8-byte `u64`.
        let _ = unsafe { write(st.wake_efd, (&one as *const u64).cast::<c_void>(), 8) };
    }
    AAUDIO_CALLBACK_RESULT_CONTINUE
}

unsafe extern "C" fn aaudio_error_cb(_stream: *mut AAudioStream, _user_data: *mut c_void, _err: i32) {
}

fn open_aaudio_stream(st: &mut SinkState) -> Result<(), String> {
    if !st.stream.is_null() {
        // SAFETY: `st.stream` is a valid non-null `AAudioStream` previously opened by `AAudioStreamBuilder_openStream`.
        unsafe {
            AAudioStream_requestStop(st.stream);
            AAudioStream_close(st.stream);
        }
        st.stream = ptr::null_mut();
    }

    let mut builder: *mut AAudioStreamBuilder = ptr::null_mut();
    // SAFETY: `&mut builder` is a valid out-pointer to receive a newly allocated `AAudioStreamBuilder`.
    let res = unsafe { AAudio_createStreamBuilder(&mut builder) };
    if res != AAUDIO_OK || builder.is_null() {
        return Err(format!("createStreamBuilder failed: {}", res));
    }

    // SAFETY: `st.shm` is a valid mapped `PwAaudioShm` alive for the process lifetime.
    let shm = unsafe { &mut *st.shm };
    let fmt = if shm.format == PW_AAUDIO_FMT_F32_LE {
        AAUDIO_FORMAT_PCM_FLOAT
    } else {
        AAUDIO_FORMAT_PCM_I16
    };

    // SAFETY: `builder` is valid and `st` outlives the stream opened from `builder`.
    unsafe {
        AAudioStreamBuilder_setDirection(builder, AAUDIO_DIRECTION_OUTPUT);
        AAudioStreamBuilder_setSharingMode(builder, AAUDIO_SHARING_MODE_SHARED);
        AAudioStreamBuilder_setPerformanceMode(builder, AAUDIO_PERFORMANCE_MODE_LOW_LATENCY);
        AAudioStreamBuilder_setUsage(builder, AAUDIO_USAGE_MEDIA);
        AAudioStreamBuilder_setContentType(builder, AAUDIO_CONTENT_TYPE_MUSIC);
        AAudioStreamBuilder_setSampleRate(builder, shm.sample_rate as i32);
        AAudioStreamBuilder_setChannelCount(builder, shm.channels as i32);
        AAudioStreamBuilder_setFormat(builder, fmt);
        AAudioStreamBuilder_setDataCallback(
            builder,
            aaudio_data_cb,
            (st as *mut SinkState).cast::<c_void>(),
        );
        AAudioStreamBuilder_setErrorCallback(
            builder,
            aaudio_error_cb,
            (st as *mut SinkState).cast::<c_void>(),
        );
    }

    // SAFETY: `builder` and `&mut st.stream` are valid pointers; `builder` is deleted immediately after.
    let res = unsafe { AAudioStreamBuilder_openStream(builder, &mut st.stream) };
    // SAFETY: `builder` is no longer needed after `openStream`.
    unsafe { AAudioStreamBuilder_delete(builder) };
    if res != AAUDIO_OK || st.stream.is_null() {
        return Err(format!("openStream failed: {}", res));
    }

    // SAFETY: `shm.active` is an aligned `u32` atomic flag; `st.stream` was just opened.
    if unsafe { AtomicU32::from_ptr(&mut shm.active) }.load(Ordering::Acquire) != 0 {
        // SAFETY: `st.stream` is valid and open.
        unsafe { AAudioStream_requestStart(st.stream) };
    }
    Ok(())
}

fn create_shm() -> Result<(c_int, *mut PwAaudioShm), String> {
    // SAFETY: `memfd_create` syscall with static NUL-terminated name and `MFD_CLOEXEC` (1).
    let shm_fd = unsafe { syscall(SYS_MEMFD_CREATE, c"pw_aaudio_shm".as_ptr(), 1u32) } as c_int;
    if shm_fd < 0 {
        return Err(format!("memfd_create failed: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: `shm_fd` is a valid memfd descriptor returned by `memfd_create`.
    if unsafe { ftruncate(shm_fd, size_of::<PwAaudioShm>() as isize) } != 0 {
        unsafe { close(shm_fd) };
        return Err(format!("ftruncate failed: {}", std::io::Error::last_os_error()));
    }

    // SAFETY: Mapping `size_of::<PwAaudioShm>()` bytes backed by `shm_fd` with `MAP_SHARED`.
    let shm_ptr = unsafe {
        mmap(
            ptr::null_mut(),
            size_of::<PwAaudioShm>(),
            PROT_READ | PROT_WRITE,
            MAP_SHARED,
            shm_fd,
            0,
        )
    } as *mut PwAaudioShm;
    if shm_ptr as usize == usize::MAX {
        unsafe { close(shm_fd) };
        return Err(format!("mmap failed: {}", std::io::Error::last_os_error()));
    }

    // SAFETY: `shm_ptr` is a valid, freshly zeroed/mapped `PwAaudioShm` exclusively owned at startup.
    unsafe {
        ptr::write_bytes(shm_ptr, 0, 1);
        (*shm_ptr).magic = PW_AAUDIO_SHM_MAGIC;
        (*shm_ptr).version = PW_AAUDIO_SHM_VERSION;
        (*shm_ptr).volume_q16 = 0x10000;
        (*shm_ptr).ring_bytes = PW_SHM_RING_BYTES as u32;
    }
    Ok((shm_fd, shm_ptr))
}

fn bind_unix_socket(sock_path: &str) -> Result<c_int, String> {
    if !sock_path.starts_with('@') {
        let c_sock = CString::new(sock_path).map_err(|e| e.to_string())?;
        // SAFETY: `c_sock` is a valid NUL-terminated path string.
        unsafe { unlink(c_sock.as_ptr()) };
    }
    let (addr, addrlen) =
        build_unix_sockaddr(sock_path).ok_or_else(|| "socket path invalid or too long".to_string())?;
    // SAFETY: Standard POSIX `socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0)`.
    let listen_fd = unsafe { socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0) };
    if listen_fd < 0 {
        return Err(format!("socket failed: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: `listen_fd` and `&addr` with `addrlen <= sizeof(SockaddrUn)` are valid.
    if unsafe { bind(listen_fd, &addr, addrlen) } != 0 {
        unsafe { close(listen_fd) };
        return Err(format!("bind({}) failed: {}", sock_path, std::io::Error::last_os_error()));
    }
    // SAFETY: `listen_fd` is bound and valid.
    if unsafe { listen(listen_fd, 4) } != 0 {
        unsafe { close(listen_fd) };
        return Err(format!("listen failed: {}", std::io::Error::last_os_error()));
    }
    Ok(listen_fd)
}

fn handle_client(client_fd: c_int, shm_fd: c_int, st: &mut SinkState) {
    if !send_shm_fds(client_fd, shm_fd, st.wake_efd) {
        // SAFETY: `client_fd` is valid and failed handshake.
        unsafe { close(client_fd) };
        return;
    }

    let mut cmd = 0u8;
    // SAFETY: `client_fd` is valid and `&mut cmd` is a 1-byte writable buffer.
    while unsafe { recv(client_fd, (&mut cmd as *mut u8).cast::<c_void>(), 1, 0) } == 1 {
        if cmd == PW_AAUDIO_CMD_CONFIG {
            if let Err(e) = open_aaudio_stream(st) {
                eprintln!("aaudio_sink: {}", e);
            }
        } else if cmd == PW_AAUDIO_CMD_ACTIVE && !st.stream.is_null() {
            // SAFETY: `st.shm` is valid and `active` is an aligned `u32`.
            let active = unsafe { AtomicU32::from_ptr(&mut (*st.shm).active) }
                .load(Ordering::Acquire);
            // SAFETY: `st.stream` is non-null and open.
            unsafe {
                if active != 0 {
                    AAudioStream_requestStart(st.stream);
                } else {
                    AAudioStream_requestStop(st.stream);
                }
            }
        } else if cmd == PW_AAUDIO_CMD_QUIT {
            break;
        }
    }
    // SAFETY: `client_fd` is closed after peer disconnects or sends `PW_AAUDIO_CMD_QUIT`.
    unsafe { close(client_fd) };
    if !st.stream.is_null() {
        // SAFETY: `st.stream` is non-null and open.
        unsafe { AAudioStream_requestStop(st.stream) };
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let sock_path = args
        .windows(2)
        .find(|w| w[0] == "--socket")
        .map(|w| w[1].clone())
        .ok_or_else(|| format!("Usage: {} --socket <path>", args[0]))?;

    let (shm_fd, shm_ptr) = create_shm()?;
    let listen_fd = bind_unix_socket(&sock_path)?;

    // SAFETY: `eventfd` syscall with `EFD_NONBLOCK | EFD_CLOEXEC`.
    let wake_efd = unsafe { eventfd(0, EFD_NONBLOCK | EFD_CLOEXEC) };
    if wake_efd < 0 {
        return Err(format!("eventfd failed: {}", std::io::Error::last_os_error()));
    }

    let mut st = SinkState {
        shm: shm_ptr,
        stream: ptr::null_mut(),
        wake_efd,
        last_flush_seq: 0,
    };

    loop {
        // SAFETY: `listen_fd` is a valid listening socket.
        let client_fd = unsafe { accept(listen_fd, ptr::null_mut(), ptr::null_mut()) };
        if client_fd >= 0 {
            handle_client(client_fd, shm_fd, &mut st);
        }
    }
}

fn main() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("aaudio_sink: {}", err);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
