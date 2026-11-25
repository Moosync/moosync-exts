use std::ffi::{c_char, c_int, c_void};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use crate::ffi::{EINVAL, POLLIN, PollFd, close, eventfd, poll, read, write};
use crate::shm::{EFD_CLOEXEC, EFD_NONBLOCK};
use crate::stream::{PwStream, PwStreamState, shm_ring_free_bytes};

pub struct PwLoop {
    pub(crate) stream: Mutex<*mut PwStream>,
    pub(crate) stream_cond: Condvar,
}

unsafe impl Send for PwLoop {}
unsafe impl Sync for PwLoop {}

struct RecursiveLockState {
    owner: Option<thread::ThreadId>,
    depth: usize,
}

pub struct PwThreadLoop {
    loop_obj: Box<PwLoop>,
    lock_state: Mutex<RecursiveLockState>,
    lock_cond: Condvar,
    stop_requested: Arc<AtomicBool>,
    stop_efd: c_int,
    thread: Option<JoinHandle<()>>,
}

pub struct PwMainLoop {
    loop_obj: Box<PwLoop>,
    quit_lock: Mutex<bool>,
    quit_cond: Condvar,
}

/// Allocates a new PipeWire threaded event loop with a non-blocking stop `eventfd`.
#[unsafe(no_mangle)]
pub extern "C" fn pw_thread_loop_new(
    _name: *const c_char,
    _props: *const c_void,
) -> *mut PwThreadLoop {
    // SAFETY: `eventfd` syscall with `EFD_NONBLOCK | EFD_CLOEXEC`.
    let stop_efd = unsafe { eventfd(0, EFD_NONBLOCK | EFD_CLOEXEC) };
    let tl = Box::new(PwThreadLoop {
        loop_obj: Box::new(PwLoop {
            stream: Mutex::new(ptr::null_mut()),
            stream_cond: Condvar::new(),
        }),
        lock_state: Mutex::new(RecursiveLockState {
            owner: None,
            depth: 0,
        }),
        lock_cond: Condvar::new(),
        stop_requested: Arc::new(AtomicBool::new(false)),
        stop_efd,
        thread: None,
    });
    Box::into_raw(tl)
}

/// Stops and frees a `PwThreadLoop` created by `pw_thread_loop_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_thread_loop_destroy(loop_ptr: *mut PwThreadLoop) {
    if loop_ptr.is_null() {
        return;
    }
    // SAFETY: `loop_ptr` is non-null and owned by caller; stopping before freeing.
    unsafe { pw_thread_loop_stop(loop_ptr) };
    // SAFETY: `loop_ptr` was allocated via `Box::into_raw` in `pw_thread_loop_new`.
    let tl = unsafe { Box::from_raw(loop_ptr) };
    if tl.stop_efd >= 0 {
        // SAFETY: `tl.stop_efd` is a valid open `eventfd` descriptor.
        unsafe { close(tl.stop_efd) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_thread_loop_get_loop(loop_ptr: *mut PwThreadLoop) -> *mut PwLoop {
    if loop_ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe { (&mut *loop_ptr).loop_obj.as_mut() as *mut PwLoop }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_thread_loop_lock(loop_ptr: *mut PwThreadLoop) {
    if let Some(tl) = unsafe { loop_ptr.as_ref() } {
        let me = thread::current().id();
        let mut state = tl.lock_state.lock().unwrap();
        while state.owner.is_some() && state.owner != Some(me) {
            state = tl.lock_cond.wait(state).unwrap();
        }
        state.owner = Some(me);
        state.depth += 1;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_thread_loop_unlock(loop_ptr: *mut PwThreadLoop) {
    if let Some(tl) = unsafe { loop_ptr.as_ref() } {
        let me = thread::current().id();
        let mut state = tl.lock_state.lock().unwrap();
        if state.owner == Some(me) && state.depth > 0 {
            state.depth -= 1;
            if state.depth == 0 {
                state.owner = None;
                tl.lock_cond.notify_one();
            }
        }
    }
}

/// Starts the background audio pump thread, blocking on AAudio's `wake_efd` via `poll()` without timer sleeps.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_thread_loop_start(loop_ptr: *mut PwThreadLoop) -> c_int {
    // SAFETY: Caller guarantees `loop_ptr` is valid for the duration of `pw_thread_loop_start`.
    let Some(tl) = (unsafe { loop_ptr.as_mut() }) else {
        return -EINVAL;
    };
    if tl.thread.is_some() {
        return 0;
    }
    tl.stop_requested.store(false, Ordering::SeqCst);
    let stop = Arc::clone(&tl.stop_requested);
    let stop_efd = tl.stop_efd;
    let loop_addr = (tl.loop_obj.as_ref() as *const PwLoop) as usize;
    let handle = thread::spawn(move || {
        // SAFETY: `PwLoop` lives inside `PwThreadLoop` until `pw_thread_loop_stop` joins this thread.
        let loop_ref = unsafe { &*(loop_addr as *const PwLoop) };
        while !stop.load(Ordering::SeqCst) {
            let (stream_ptr, wake_efd, is_streaming) = {
                let mut guard = loop_ref.stream.lock().unwrap();
                loop {
                    if stop.load(Ordering::SeqCst) {
                        break (ptr::null_mut(), -1, false);
                    }
                    let ptr = *guard;
                    // SAFETY: When non-null, `ptr` is cleared under `loop_ref.stream` lock before `pw_stream_destroy` frees it.
                    if let Some(stream) = unsafe { ptr.as_ref() }
                        && stream.state == PwStreamState::Streaming
                    {
                        break (ptr, stream.wake_efd, true);
                    }
                    guard = loop_ref.stream_cond.wait(guard).unwrap();
                }
            };
            if stop.load(Ordering::SeqCst) || !is_streaming {
                break;
            }
            // SAFETY: `stream_ptr` is valid while `pw_stream_destroy` hasn't run.
            if let Some(stream) = unsafe { stream_ptr.as_mut() }
                && let Some(process_cb) = stream.events.process
            {
                let period = 960 * stream.frame_size.max(1);
                for _ in 0..4 {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    // SAFETY: `stream.shm` is a valid mapped `PwAaudioShm` for the stream's lifetime.
                    let can_push = if let Some(shm) = unsafe { stream.shm.as_ref() } {
                        // SAFETY: `shm` is valid and aligned.
                        unsafe { shm_ring_free_bytes(shm) >= period }
                    } else {
                        true
                    };
                    if !can_push {
                        break;
                    }
                    // SAFETY: `process_cb` and `stream.userdata` are provided by PipeWire caller.
                    unsafe { process_cb(stream.userdata) };
                }
            }
            if wake_efd >= 0 && stop_efd >= 0 {
                let mut fds = [
                    PollFd {
                        fd: wake_efd,
                        events: POLLIN,
                        revents: 0,
                    },
                    PollFd {
                        fd: stop_efd,
                        events: POLLIN,
                        revents: 0,
                    },
                ];
                // SAFETY: `fds` is a valid 2-element `PollFd` array; blocks until AAudio drains or loop stops.
                let _ = unsafe { poll(fds.as_mut_ptr(), 2, -1) };
                if (fds[0].revents & POLLIN) != 0 {
                    let mut counter: u64 = 0;
                    // SAFETY: `wake_efd` is readable (`EFD_NONBLOCK`) and `&mut counter` is 8 bytes.
                    let _ = unsafe { read(wake_efd, (&mut counter as *mut u64).cast::<c_void>(), 8) };
                }
            }
        }
    });
    tl.thread = Some(handle);
    0
}

/// Signals the background audio pump thread to stop and joins it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_thread_loop_stop(loop_ptr: *mut PwThreadLoop) {
    // SAFETY: Caller guarantees `loop_ptr` is valid or null.
    let Some(tl) = (unsafe { loop_ptr.as_mut() }) else {
        return;
    };
    tl.stop_requested.store(true, Ordering::SeqCst);
    tl.loop_obj.stream_cond.notify_all();
    if tl.stop_efd >= 0 {
        let one: u64 = 1;
        // SAFETY: `tl.stop_efd` is a valid `eventfd` descriptor and `&one` is 8 bytes.
        let _ = unsafe { write(tl.stop_efd, (&one as *const u64).cast::<c_void>(), 8) };
    }
    if let Some(handle) = tl.thread.take() {
        let _ = handle.join();
    }
}

/// Allocates a new PipeWire main event loop.
#[unsafe(no_mangle)]
pub extern "C" fn pw_main_loop_new(_props: *const c_void) -> *mut PwMainLoop {
    Box::into_raw(Box::new(PwMainLoop {
        loop_obj: Box::new(PwLoop {
            stream: Mutex::new(ptr::null_mut()),
            stream_cond: Condvar::new(),
        }),
        quit_lock: Mutex::new(false),
        quit_cond: Condvar::new(),
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_main_loop_destroy(loop_ptr: *mut PwMainLoop) {
    if !loop_ptr.is_null() {
        drop(unsafe { Box::from_raw(loop_ptr) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_main_loop_get_loop(loop_ptr: *mut PwMainLoop) -> *mut PwLoop {
    if loop_ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe { (&mut *loop_ptr).loop_obj.as_mut() as *mut PwLoop }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_main_loop_run(loop_ptr: *mut PwMainLoop) -> c_int {
    let Some(ml) = (unsafe { loop_ptr.as_ref() }) else {
        return -EINVAL;
    };
    let mut quit = ml.quit_lock.lock().unwrap();
    while !*quit {
        quit = ml.quit_cond.wait(quit).unwrap();
    }
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_main_loop_quit(loop_ptr: *mut PwMainLoop) -> c_int {
    let Some(ml) = (unsafe { loop_ptr.as_ref() }) else {
        return -EINVAL;
    };
    let mut quit = ml.quit_lock.lock().unwrap();
    *quit = true;
    ml.quit_cond.notify_all();
    0
}
