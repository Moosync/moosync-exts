use std::ffi::{c_char, c_int, c_void};
use std::mem::{size_of, zeroed};
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use crate::event_loop::PwLoop;
use crate::ffi::{
    EIO, EINVAL, ENAMETOOLONG, MSG_NOSIGNAL, __errno_location, close, connect, mmap, munmap,
    recv_shm_fds, send, socket, write,
};
use crate::shm::{
    AF_UNIX, MAP_SHARED, PROT_READ, PROT_WRITE, PW_AAUDIO_CMD_ACTIVE, PW_AAUDIO_CMD_CONFIG,
    PW_AAUDIO_CMD_FLUSH, PW_AAUDIO_CMD_QUIT, PW_AAUDIO_CMD_VOLUME, PW_AAUDIO_FMT_F32_LE,
    PW_AAUDIO_FMT_S16_LE, PwAaudioShm, SOCK_CLOEXEC, SOCK_STREAM, build_unix_sockaddr,
};
use crate::spa::{SpaBuffer, SpaChunk, SpaData, SpaPod, parse_format_pod};

pub const PW_BUFFER_STAGING_BYTES: usize = 16384;
pub const PW_STREAM_FLAG_INACTIVE: u32 = 1 << 2;
const ENV_PW_ANDROID_SOCK_PATH: &str = "PW_ANDROID_SOCK_PATH";

#[repr(C)]
pub struct PwBuffer {
    pub buffer: *mut SpaBuffer,
    pub user_data: *mut c_void,
    pub size: u64,
    pub requested: u64,
    pub time: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PwStreamState {
    Error = -1,
    Unconnected = 0,
    Connecting = 1,
    Paused = 2,
    Streaming = 3,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PwStreamEvents {
    pub version: u32,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
    pub state_changed: Option<
        unsafe extern "C" fn(*mut c_void, PwStreamState, PwStreamState, *const c_char),
    >,
    pub control_info: Option<unsafe extern "C" fn(*mut c_void, u32, *const c_void)>,
    pub io_changed: Option<unsafe extern "C" fn(*mut c_void, u32, *mut c_void, u32)>,
    pub param_changed: Option<unsafe extern "C" fn(*mut c_void, u32, *const SpaPod)>,
    pub add_buffer: Option<unsafe extern "C" fn(*mut c_void, *mut PwBuffer)>,
    pub remove_buffer: Option<unsafe extern "C" fn(*mut c_void, *mut PwBuffer)>,
    pub process: Option<unsafe extern "C" fn(*mut c_void)>,
    pub drained: Option<unsafe extern "C" fn(*mut c_void)>,
    pub command: Option<unsafe extern "C" fn(*mut c_void, *const c_void)>,
    pub trigger_done: Option<unsafe extern "C" fn(*mut c_void)>,
}

pub struct PwProperties;
pub struct PwProxy;

pub struct PwStream {
    pub(crate) loop_ptr: *mut PwLoop,
    pub(crate) events: PwStreamEvents,
    pub(crate) userdata: *mut c_void,
    pub(crate) state: PwStreamState,
    pub(crate) sample_rate: u32,
    pub(crate) channels: u32,
    pub(crate) format: u32,
    pub(crate) frame_size: u32,
    pub(crate) sock_fd: c_int,
    pub(crate) shm_fd: c_int,
    pub(crate) wake_efd: c_int,
    pub(crate) shm: *mut PwAaudioShm,
    pub(crate) pw_buf: PwBuffer,
    pub(crate) spa_buf: SpaBuffer,
    pub(crate) spa_data: SpaData,
    pub(crate) spa_chunk: SpaChunk,
    pub(crate) staging: [u8; PW_BUFFER_STAGING_BYTES],
    pub(crate) buffer_dequeued: bool,
}

unsafe fn send_cmd(stream: &PwStream, cmd: u8) {
    if stream.sock_fd >= 0 {
        let _ = unsafe { send(stream.sock_fd, (&cmd as *const u8).cast::<c_void>(), 1, MSG_NOSIGNAL) };
    }
}

unsafe fn connect_aaudio_sink(stream: &mut PwStream) -> c_int {
    let Ok(sock_path) = std::env::var(ENV_PW_ANDROID_SOCK_PATH) else {
        return 0;
    };
    if sock_path.is_empty() {
        return 0;
    }
    let Some((addr, addrlen)) = build_unix_sockaddr(&sock_path) else {
        return -ENAMETOOLONG;
    };

    let fd = unsafe { socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0) };
    if fd < 0 {
        return -unsafe { *__errno_location() };
    }

    let mut connected = false;
    for _ in 0..40 {
        if unsafe { connect(fd, &addr, addrlen) } == 0 {
            connected = true;
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    if !connected {
        let err = -unsafe { *__errno_location() };
        unsafe { close(fd) };
        return err;
    }

    let (shm_fd, wake_efd) = unsafe { recv_shm_fds(fd) };
    if shm_fd < 0 {
        // SAFETY: `fd` is a valid socket descriptor to close on error.
        unsafe { close(fd) };
        return -EIO;
    }

    // SAFETY: Mapping `size_of::<PwAaudioShm>()` bytes from `shm_fd` with `MAP_SHARED`.
    let mapped = unsafe {
        mmap(
            ptr::null_mut(),
            size_of::<PwAaudioShm>(),
            PROT_READ | PROT_WRITE,
            MAP_SHARED,
            shm_fd,
            0,
        )
    };
    if mapped == usize::MAX as *mut c_void {
        // SAFETY: Reading thread-local `errno` immediately after `mmap` failure.
        let err = -unsafe { *__errno_location() };
        // SAFETY: Closing valid file descriptors on error.
        unsafe {
            if wake_efd >= 0 {
                close(wake_efd);
            }
            close(shm_fd);
            close(fd);
        }
        return err;
    }

    stream.sock_fd = fd;
    stream.shm_fd = shm_fd;
    stream.wake_efd = wake_efd;
    stream.shm = mapped.cast::<PwAaudioShm>();
    0
}

pub(crate) unsafe fn shm_ring_free_bytes(shm: &PwAaudioShm) -> u32 {
    let r = unsafe { AtomicU32::from_ptr((&shm.read_idx as *const u32).cast_mut()) }
        .load(Ordering::Acquire);
    let w = unsafe { AtomicU32::from_ptr((&shm.write_idx as *const u32).cast_mut()) }
        .load(Ordering::Relaxed);
    let used = w.wrapping_sub(r);
    shm.ring_bytes.saturating_sub(used)
}

unsafe fn shm_ring_write(shm: &mut PwAaudioShm, src: &[u8]) {
    let free = unsafe { shm_ring_free_bytes(shm) } as usize;
    let len = src.len().min(free);
    if len == 0 {
        return;
    }
    let w = unsafe { AtomicU32::from_ptr(&mut shm.write_idx) }.load(Ordering::Relaxed);
    let mask = (shm.ring_bytes as usize) - 1;
    let pos = (w as usize) & mask;
    let first = (shm.ring_bytes as usize - pos).min(len);
    shm.data[pos..pos + first].copy_from_slice(&src[..first]);
    if len > first {
        shm.data[..len - first].copy_from_slice(&src[first..len]);
    }
    unsafe { AtomicU32::from_ptr(&mut shm.write_idx) }
        .store(w.wrapping_add(len as u32), Ordering::Release);
}

unsafe fn set_stream_state(stream: &mut PwStream, next: PwStreamState) {
    let old = stream.state;
    if old == next {
        return;
    }
    stream.state = next;
    if let Some(cb) = stream.events.state_changed {
        unsafe { cb(stream.userdata, old, next, ptr::null()) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_stream_new_simple(
    loop_ptr: *mut PwLoop,
    _name: *const c_char,
    _props: *mut PwProperties,
    events: *const PwStreamEvents,
    data: *mut c_void,
) -> *mut PwStream {
    let ev = if events.is_null() {
        unsafe { zeroed() }
    } else {
        unsafe { *events }
    };
    let mut stream = Box::new(PwStream {
        loop_ptr,
        events: ev,
        userdata: data,
        state: PwStreamState::Unconnected,
        sample_rate: 48000,
        channels: 2,
        format: PW_AAUDIO_FMT_F32_LE,
        frame_size: 8,
        sock_fd: -1,
        shm_fd: -1,
        wake_efd: -1,
        shm: ptr::null_mut(),
        pw_buf: unsafe { zeroed() },
        spa_buf: unsafe { zeroed() },
        spa_data: unsafe { zeroed() },
        spa_chunk: unsafe { zeroed() },
        staging: [0u8; PW_BUFFER_STAGING_BYTES],
        buffer_dequeued: false,
    });

    stream.spa_data.ty = 1;
    stream.spa_data.flags = 3;
    stream.spa_data.maxsize = PW_BUFFER_STAGING_BYTES as u32;
    stream.spa_data.data = stream.staging.as_mut_ptr().cast::<c_void>();
    stream.spa_data.chunk = &mut stream.spa_chunk;
    stream.spa_buf.n_datas = 1;
    stream.spa_buf.datas = &mut stream.spa_data;
    stream.pw_buf.buffer = &mut stream.spa_buf;

    let raw = Box::into_raw(stream);
    if let Some(lp) = unsafe { loop_ptr.as_ref() } {
        *lp.stream.lock().unwrap() = raw;
    }
    raw
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_stream_destroy(stream_ptr: *mut PwStream) {
    let Some(stream) = (unsafe { stream_ptr.as_mut() }) else {
        return;
    };
    if let Some(lp) = unsafe { stream.loop_ptr.as_ref() } {
        let mut guard = lp.stream.lock().unwrap();
        if *guard == stream_ptr {
            *guard = ptr::null_mut();
        }
    }
    if let Some(cb) = stream.events.destroy {
        unsafe { cb(stream.userdata) };
    }
    if stream.sock_fd >= 0 {
        unsafe {
            send_cmd(stream, PW_AAUDIO_CMD_QUIT);
            close(stream.sock_fd);
        }
    }
    if !stream.shm.is_null() {
        unsafe { munmap(stream.shm.cast::<c_void>(), size_of::<PwAaudioShm>()) };
    }
    if stream.wake_efd >= 0 {
        // SAFETY: `stream.wake_efd` is a valid open `eventfd` descriptor.
        unsafe { close(stream.wake_efd) };
    }
    if stream.shm_fd >= 0 {
        // SAFETY: `stream.shm_fd` is a valid open `memfd` descriptor.
        unsafe { close(stream.shm_fd) };
    }
    drop(unsafe { Box::from_raw(stream_ptr) });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_stream_connect(
    stream_ptr: *mut PwStream,
    _direction: c_int,
    _target_id: u32,
    flags: u32,
    params: *const *const SpaPod,
    n_params: u32,
) -> c_int {
    let Some(stream) = (unsafe { stream_ptr.as_mut() }) else {
        return -EINVAL;
    };
    if !params.is_null() {
        for i in 0..n_params as usize {
            let pod = unsafe { *params.add(i) };
            unsafe {
                parse_format_pod(
                    pod,
                    &mut stream.format,
                    &mut stream.sample_rate,
                    &mut stream.channels,
                )
            };
        }
    }
    let bps = if stream.format == PW_AAUDIO_FMT_S16_LE { 2 } else { 4 };
    stream.frame_size = stream.channels * bps;

    let res = unsafe { connect_aaudio_sink(stream) };
    if res < 0 {
        unsafe { set_stream_state(stream, PwStreamState::Error) };
        return res;
    }

    let start_active = (flags & PW_STREAM_FLAG_INACTIVE) == 0;
    if let Some(shm) = unsafe { stream.shm.as_mut() } {
        shm.sample_rate = stream.sample_rate;
        shm.channels = stream.channels;
        shm.format = stream.format;
        shm.frame_size = stream.frame_size;
        shm.volume_q16 = 0x10000;
        unsafe { AtomicU32::from_ptr(&mut shm.active) }
            .store(u32::from(start_active), Ordering::Release);
        unsafe { send_cmd(stream, PW_AAUDIO_CMD_CONFIG) };
    }

    unsafe {
        set_stream_state(stream, PwStreamState::Connecting);
        set_stream_state(stream, PwStreamState::Paused);
        if start_active {
            set_stream_state(stream, PwStreamState::Streaming);
            if let Some(lp) = stream.loop_ptr.as_ref() {
                lp.stream_cond.notify_all();
            }
        }
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_stream_get_state(
    stream_ptr: *mut PwStream,
    error: *mut *const c_char,
) -> PwStreamState {
    if !error.is_null() {
        unsafe { *error = ptr::null() };
    }
    unsafe { stream_ptr.as_ref() }
        .map(|s| s.state)
        .unwrap_or(PwStreamState::Error)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_stream_set_active(stream_ptr: *mut PwStream, active: bool) -> c_int {
    let Some(stream) = (unsafe { stream_ptr.as_mut() }) else {
        return -EINVAL;
    };
    if let Some(shm) = unsafe { stream.shm.as_mut() } {
        unsafe { AtomicU32::from_ptr(&mut shm.active) }
            .store(u32::from(active), Ordering::Release);
        unsafe { send_cmd(stream, PW_AAUDIO_CMD_ACTIVE) };
    }
    unsafe {
        set_stream_state(
            stream,
            if active {
                PwStreamState::Streaming
            } else {
                PwStreamState::Paused
            },
        );
        if active {
            if let Some(lp) = stream.loop_ptr.as_ref() {
                lp.stream_cond.notify_all();
            }
            if stream.wake_efd >= 0 {
                let one: u64 = 1;
                let _ = write(stream.wake_efd, (&one as *const u64).cast::<c_void>(), 8);
            }
        }
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_stream_set_control(
    stream_ptr: *mut PwStream,
    _id: u32,
    n_values: u32,
    values: *const f32,
) -> c_int {
    let Some(stream) = (unsafe { stream_ptr.as_mut() }) else {
        return -EINVAL;
    };
    if values.is_null() || n_values == 0 {
        return -EINVAL;
    }
    let vol = unsafe { *values }.clamp(0.0, 1.0);
    if let Some(shm) = unsafe { stream.shm.as_mut() } {
        let q16 = (vol * 65536.0 + 0.5) as u32;
        unsafe { AtomicU32::from_ptr(&mut shm.volume_q16) }.store(q16, Ordering::Release);
        unsafe { send_cmd(stream, PW_AAUDIO_CMD_VOLUME) };
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_stream_flush(stream_ptr: *mut PwStream, drain: bool) -> c_int {
    let Some(stream) = (unsafe { stream_ptr.as_mut() }) else {
        return -EINVAL;
    };
    if let Some(shm) = unsafe { stream.shm.as_mut() } {
        unsafe { AtomicU32::from_ptr(&mut shm.flush_seq) }.fetch_add(1, Ordering::Release);
        unsafe { send_cmd(stream, PW_AAUDIO_CMD_FLUSH) };
    }
    if drain && let Some(cb) = stream.events.drained {
        unsafe { cb(stream.userdata) };
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_stream_dequeue_buffer(stream_ptr: *mut PwStream) -> *mut PwBuffer {
    let Some(stream) = (unsafe { stream_ptr.as_mut() }) else {
        return ptr::null_mut();
    };
    if stream.buffer_dequeued {
        return ptr::null_mut();
    }
    stream.buffer_dequeued = true;
    stream.spa_chunk.offset = 0;
    stream.spa_chunk.size = 0;
    stream.spa_chunk.stride = stream.frame_size as i32;
    stream.spa_chunk.flags = 0;
    let max_frames = PW_BUFFER_STAGING_BYTES as u32 / stream.frame_size.max(1);
    stream.pw_buf.requested = u64::from(max_frames.min(960));
    stream.pw_buf.size = 0;
    &mut stream.pw_buf
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_stream_queue_buffer(
    stream_ptr: *mut PwStream,
    buffer: *mut PwBuffer,
) -> c_int {
    let Some(stream) = (unsafe { stream_ptr.as_mut() }) else {
        return -EINVAL;
    };
    if buffer.is_null() || !stream.buffer_dequeued {
        return -EINVAL;
    }
    stream.buffer_dequeued = false;
    let size = stream.spa_chunk.size as usize;
    let offset = stream.spa_chunk.offset as usize;
    if size > 0
        && offset + size <= PW_BUFFER_STAGING_BYTES
        && let Some(shm) = unsafe { stream.shm.as_mut() }
    {
        unsafe { shm_ring_write(shm, &stream.staging[offset..offset + size]) };
    }
    0
}
