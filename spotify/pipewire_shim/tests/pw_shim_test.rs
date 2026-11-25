use std::ffi::{CStr, c_char, c_int, c_void};
use std::mem::{size_of, zeroed};
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use pipewire_shim_lib::shm::{
    AF_UNIX, Cmsghdr, Iovec, MAP_SHARED, Msghdr, PROT_READ, PROT_WRITE, PW_AAUDIO_CMD_CONFIG,
    PW_AAUDIO_CMD_QUIT, PW_AAUDIO_FMT_S16_LE, PW_AAUDIO_SHM_MAGIC, PW_AAUDIO_SHM_VERSION,
    PW_SHM_RING_BYTES, PwAaudioShm, SCM_RIGHTS, SOCK_STREAM, SOL_SOCKET, SockaddrUn,
    build_unix_sockaddr, cmsg_align, cmsg_space,
};
use pipewire_shim_lib::{
    PwBuffer, PwLoop, PwStream, PwStreamEvents, PwStreamState, SpaPod, SpaPodObjectBody,
    SpaPodProp, pw_context_connect, pw_context_destroy, pw_context_new, pw_core_disconnect,
    pw_deinit, pw_get_library_version, pw_init, pw_main_loop_destroy, pw_main_loop_get_loop,
    pw_main_loop_new, pw_main_loop_quit, pw_main_loop_run, pw_properties_new,
    pw_proxy_add_object_listener, pw_proxy_destroy, pw_stream_connect, pw_stream_dequeue_buffer,
    pw_stream_destroy, pw_stream_flush, pw_stream_get_state, pw_stream_new_simple,
    pw_stream_queue_buffer, pw_stream_set_active, pw_stream_set_control, pw_thread_loop_destroy,
    pw_thread_loop_get_loop, pw_thread_loop_lock, pw_thread_loop_new, pw_thread_loop_start,
    pw_thread_loop_stop, pw_thread_loop_unlock,
};

unsafe extern "C" {
    fn memfd_create(name: *const c_char, flags: u32) -> c_int;
    fn ftruncate(fd: c_int, length: isize) -> c_int;
    fn mmap(
        addr: *mut c_void,
        length: usize,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: isize,
    ) -> *mut c_void;
    fn munmap(addr: *mut c_void, length: usize) -> c_int;
    fn socket(domain: c_int, ty: c_int, protocol: c_int) -> c_int;
    fn bind(sockfd: c_int, addr: *const SockaddrUn, addrlen: u32) -> c_int;
    fn listen(sockfd: c_int, backlog: c_int) -> c_int;
    fn accept(sockfd: c_int, addr: *mut c_void, addrlen: *mut u32) -> c_int;
    fn sendmsg(sockfd: c_int, msg: *const Msghdr, flags: c_int) -> isize;
    fn recv(sockfd: c_int, buf: *mut c_void, len: usize, flags: c_int) -> isize;
    fn close(fd: c_int) -> c_int;
}

struct TestCtx {
    stream: *mut PwStream,
    last_state: PwStreamState,
    drained_calls: i32,
}

unsafe extern "C" fn on_state_changed(
    data: *mut c_void,
    _old: PwStreamState,
    state: PwStreamState,
    _error: *const c_char,
) {
    let ctx = unsafe { &mut *(data as *mut TestCtx) };
    ctx.last_state = state;
}

unsafe extern "C" fn on_process(data: *mut c_void) {
    let ctx = unsafe { &mut *(data as *mut TestCtx) };
    let buf: *mut PwBuffer = unsafe { pw_stream_dequeue_buffer(ctx.stream) };
    if buf.is_null() {
        return;
    }
    let spa_buf = unsafe { &*(*buf).buffer };
    let spa_data = unsafe { &*spa_buf.datas };
    let samples = spa_data.data as *mut i16;
    for i in 0..64 {
        unsafe { *samples.add(i) = (i as i16) + 100 };
    }
    let chunk = unsafe { &mut *spa_data.chunk };
    chunk.offset = 0;
    chunk.size = (64 * size_of::<i16>()) as u32;
    unsafe { pw_stream_queue_buffer(ctx.stream, buf) };
}

unsafe extern "C" fn on_drained(data: *mut c_void) {
    let ctx = unsafe { &mut *(data as *mut TestCtx) };
    ctx.drained_calls += 1;
}

#[test]
fn test_pipewire_shim_shm_ring_and_lifecycle() {
    let shm_fd = unsafe { memfd_create(c"rust_pw_shm".as_ptr(), 1) };
    assert!(shm_fd >= 0);
    assert_eq!(
        unsafe { ftruncate(shm_fd, size_of::<PwAaudioShm>() as isize) },
        0
    );
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
    assert_ne!(shm_ptr as usize, usize::MAX);
    unsafe {
        ptr::write_bytes(shm_ptr, 0, 1);
        (*shm_ptr).magic = PW_AAUDIO_SHM_MAGIC;
        (*shm_ptr).version = PW_AAUDIO_SHM_VERSION;
        (*shm_ptr).ring_bytes = PW_SHM_RING_BYTES as u32;
    }

    let sock_path = format!("/tmp/pw_rust_shim_{}.sock", std::process::id());
    let _ = std::fs::remove_file(&sock_path);

    let listen_fd = unsafe { socket(AF_UNIX, SOCK_STREAM, 0) };
    assert!(listen_fd >= 0);
    let (addr, addrlen) = build_unix_sockaddr(&sock_path).unwrap();
    assert_eq!(unsafe { bind(listen_fd, &addr, addrlen) }, 0);
    assert_eq!(unsafe { listen(listen_fd, 1) }, 0);

    let srv = thread::spawn(move || {
        let client = unsafe { accept(listen_fd, ptr::null_mut(), ptr::null_mut()) };
        assert!(client >= 0);

        let mut byte = b'S';
        let mut iov = Iovec {
            iov_base: (&mut byte as *mut u8).cast::<c_void>(),
            iov_len: 1,
        };
        let mut cmsg_buf = [0u8; cmsg_space(size_of::<c_int>())];
        let mut msg: Msghdr = unsafe { zeroed() };
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = cmsg_buf.as_mut_ptr().cast::<c_void>();
        msg.msg_controllen = cmsg_buf.len();
        let cmsg = unsafe { &mut *(cmsg_buf.as_mut_ptr() as *mut Cmsghdr) };
        cmsg.cmsg_level = SOL_SOCKET;
        cmsg.cmsg_type = SCM_RIGHTS;
        cmsg.cmsg_len = cmsg_align(size_of::<Cmsghdr>()) + size_of::<c_int>();
        unsafe {
            let data_ptr = cmsg_buf.as_mut_ptr().add(cmsg_align(size_of::<Cmsghdr>()));
            ptr::copy_nonoverlapping((&shm_fd as *const c_int).cast::<u8>(), data_ptr, size_of::<c_int>());
        }
        assert_eq!(unsafe { sendmsg(client, &msg, 0) }, 1);

        let mut cmds = Vec::new();
        let mut cmd = 0u8;
        while unsafe { recv(client, (&mut cmd as *mut u8).cast::<c_void>(), 1, 0) } == 1 {
            cmds.push(cmd);
            if cmd == PW_AAUDIO_CMD_QUIT {
                break;
            }
        }
        unsafe { close(client) };
        cmds
    });

    unsafe { std::env::set_var("PW_ANDROID_SOCK_PATH", &sock_path) };

    pw_init(ptr::null_mut(), ptr::null_mut());
    let ver = unsafe { CStr::from_ptr(pw_get_library_version()) }
        .to_str()
        .unwrap();
    assert_eq!(ver, "0.3.80");

    let tl = pw_thread_loop_new(c"test".as_ptr(), ptr::null());
    assert!(!tl.is_null());
    let loop_ptr: *mut PwLoop = unsafe { pw_thread_loop_get_loop(tl) };
    assert!(!loop_ptr.is_null());

    let mut ctx = TestCtx {
        stream: ptr::null_mut(),
        last_state: PwStreamState::Unconnected,
        drained_calls: 0,
    };
    let events = PwStreamEvents {
        version: 2,
        destroy: None,
        state_changed: Some(on_state_changed),
        control_info: None,
        io_changed: None,
        param_changed: None,
        add_buffer: None,
        remove_buffer: None,
        process: Some(on_process),
        drained: Some(on_drained),
        command: None,
        trigger_done: None,
    };

    let stream = unsafe {
        pw_stream_new_simple(
            loop_ptr,
            c"spotify".as_ptr(),
            ptr::null_mut(),
            &events,
            (&mut ctx as *mut TestCtx).cast::<c_void>(),
        )
    };
    assert!(!stream.is_null());
    ctx.stream = stream;

    #[repr(C)]
    struct FmtPod {
        pod: SpaPod,
        body: SpaPodObjectBody,
        p_fmt: SpaPodProp,
        v_fmt: u32,
        pad1: u32,
        p_rate: SpaPodProp,
        v_rate: u32,
        pad2: u32,
        p_ch: SpaPodProp,
        v_ch: u32,
        pad3: u32,
    }
    let fmt_pod = FmtPod {
        pod: SpaPod {
            size: (size_of::<FmtPod>() - size_of::<SpaPod>()) as u32,
            ty: 15,
        },
        body: SpaPodObjectBody { ty: 0, id: 0 },
        p_fmt: SpaPodProp {
            key: 0x10001,
            flags: 0,
            value: SpaPod { size: 4, ty: 3 },
        },
        v_fmt: 0x103,
        pad1: 0,
        p_rate: SpaPodProp {
            key: 0x10003,
            flags: 0,
            value: SpaPod { size: 4, ty: 4 },
        },
        v_rate: 44100,
        pad2: 0,
        p_ch: SpaPodProp {
            key: 0x10004,
            flags: 0,
            value: SpaPod { size: 4, ty: 4 },
        },
        v_ch: 2,
        pad3: 0,
    };
    let params = [&fmt_pod.pod as *const SpaPod];

    assert_eq!(
        unsafe { pw_stream_connect(stream, 1, u32::MAX, 0, params.as_ptr(), 1) },
        0
    );
    assert_eq!(ctx.last_state, PwStreamState::Streaming);
    unsafe {
        assert_eq!((*shm_ptr).sample_rate, 44100);
        assert_eq!((*shm_ptr).channels, 2);
        assert_eq!((*shm_ptr).format, PW_AAUDIO_FMT_S16_LE);
        assert_eq!((*shm_ptr).frame_size, 4);
    }

    assert_eq!(unsafe { pw_thread_loop_start(tl) }, 0);
    for _ in 0..50 {
        let w = unsafe { AtomicU32::from_ptr(&mut (*shm_ptr).write_idx) }.load(Ordering::Acquire);
        if w >= (64 * size_of::<i16>()) as u32 {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    unsafe {
        let w = AtomicU32::from_ptr(&mut (*shm_ptr).write_idx).load(Ordering::Acquire);
        assert!(w >= (64 * size_of::<i16>()) as u32);
        let ring_samples = (*shm_ptr).data.as_ptr() as *const i16;
        assert_eq!(*ring_samples, 100);
        assert_eq!(*ring_samples.add(63), 163);
    }

    let vol = 0.5f32;
    assert_eq!(unsafe { pw_stream_set_control(stream, 0, 1, &vol) }, 0);
    assert_eq!(unsafe { (*shm_ptr).volume_q16 }, 32768);

    assert_eq!(unsafe { pw_stream_set_active(stream, false) }, 0);
    assert_eq!(
        unsafe { pw_stream_get_state(stream, ptr::null_mut()) },
        PwStreamState::Paused
    );

    assert_eq!(unsafe { pw_stream_flush(stream, true) }, 0);
    assert_eq!(ctx.drained_calls, 1);

    unsafe {
        pw_thread_loop_stop(tl);
        pw_stream_destroy(stream);
        pw_thread_loop_destroy(tl);
    }
    pw_deinit();

    let cmds = srv.join().unwrap();
    assert_eq!(cmds[0], PW_AAUDIO_CMD_CONFIG);

    unsafe {
        close(listen_fd);
        munmap(shm_ptr.cast::<c_void>(), size_of::<PwAaudioShm>());
        close(shm_fd);
    }
    let _ = std::fs::remove_file(&sock_path);
}

#[test]
fn test_pipewire_main_loop_context_core_and_recursive_lock() {
    let ml = pw_main_loop_new(ptr::null());
    assert!(!ml.is_null());
    let lp = unsafe { pw_main_loop_get_loop(ml) };
    assert!(!lp.is_null());

    let props = pw_properties_new(c"test".as_ptr());
    let ctx = pw_context_new(lp, props, 0);
    let core = pw_context_connect(ctx, ptr::null_mut(), 0);
    assert!(!core.is_null());
    pw_proxy_add_object_listener(ptr::null_mut(), ptr::null_mut(), ptr::null(), ptr::null_mut());
    unsafe {
        pw_proxy_destroy(ptr::null_mut());
        assert_eq!(pw_core_disconnect(core), 0);
        pw_context_destroy(ctx);
        drop(Box::from_raw(props));
    }

    let ml_addr = ml as usize;
    let quitter = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        assert_eq!(unsafe { pw_main_loop_quit(ml_addr as *mut _) }, 0);
    });
    assert_eq!(unsafe { pw_main_loop_run(ml) }, 0);
    quitter.join().unwrap();
    unsafe { pw_main_loop_destroy(ml) };

    let tl = pw_thread_loop_new(c"lock_test".as_ptr(), ptr::null());
    unsafe {
        pw_thread_loop_lock(tl);
        pw_thread_loop_lock(tl);
        pw_thread_loop_unlock(tl);
        pw_thread_loop_unlock(tl);
        pw_thread_loop_destroy(tl);
    }
}
