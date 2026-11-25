use std::ffi::{c_char, c_void};
use std::mem::size_of;
use std::ptr;
use std::thread;
use std::time::Duration;

use pipewire_shim_lib::spa::{
    SPA_AUDIO_FORMAT_F32_LE, SPA_FORMAT_AUDIO_CHANNELS, SPA_FORMAT_AUDIO_FORMAT,
    SPA_FORMAT_AUDIO_RATE, SPA_TYPE_ID, SPA_TYPE_INT, SPA_TYPE_OBJECT,
};
use pipewire_shim_lib::{
    PwBuffer, PwStream, PwStreamEvents, PwStreamState, SpaPod, SpaPodObjectBody, SpaPodProp,
    pw_deinit, pw_init, pw_stream_connect, pw_stream_dequeue_buffer, pw_stream_destroy,
    pw_stream_new_simple, pw_stream_queue_buffer, pw_thread_loop_destroy,
    pw_thread_loop_get_loop, pw_thread_loop_new, pw_thread_loop_start, pw_thread_loop_stop,
};

struct SineCtx {
    stream: *mut PwStream,
    phase: f32,
    frames_sent: u64,
}

unsafe extern "C" fn on_state(
    _data: *mut c_void,
    _old: PwStreamState,
    state: PwStreamState,
    _err: *const c_char,
) {
    eprintln!("pw_sine_poc: state -> {:?}", state);
}

unsafe extern "C" fn on_process(data: *mut c_void) {
    let ctx = unsafe { &mut *(data as *mut SineCtx) };
    let buf: *mut PwBuffer = unsafe { pw_stream_dequeue_buffer(ctx.stream) };
    if buf.is_null() {
        return;
    }
    let spa_buf = unsafe { &*(*buf).buffer };
    let spa_data = unsafe { &*spa_buf.datas };
    let frames = 480usize;
    let out = unsafe { std::slice::from_raw_parts_mut(spa_data.data.cast::<f32>(), frames * 2) };
    for i in 0..frames {
        let s = (ctx.phase * 2.0 * std::f32::consts::PI).sin() * 0.2;
        out[i * 2] = s;
        out[i * 2 + 1] = s;
        ctx.phase = (ctx.phase + 440.0 / 48000.0) % 1.0;
    }
    let chunk = unsafe { &mut *spa_data.chunk };
    chunk.offset = 0;
    chunk.size = (frames * 2 * size_of::<f32>()) as u32;
    unsafe { pw_stream_queue_buffer(ctx.stream, buf) };
    ctx.frames_sent += frames as u64;
}

fn main() {
    pw_init(ptr::null_mut(), ptr::null_mut());
    let tl = pw_thread_loop_new(c"poc".as_ptr(), ptr::null());
    let loop_ptr = unsafe { pw_thread_loop_get_loop(tl) };
    let mut ctx = SineCtx {
        stream: ptr::null_mut(),
        phase: 0.0,
        frames_sent: 0,
    };
    let events = PwStreamEvents {
        version: 2,
        destroy: None,
        state_changed: Some(on_state),
        control_info: None,
        io_changed: None,
        param_changed: None,
        add_buffer: None,
        remove_buffer: None,
        process: Some(on_process),
        drained: None,
        command: None,
        trigger_done: None,
    };
    let stream = unsafe {
        pw_stream_new_simple(
            loop_ptr,
            c"poc".as_ptr(),
            ptr::null_mut(),
            &events,
            (&mut ctx as *mut SineCtx).cast::<c_void>(),
        )
    };
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
            ty: SPA_TYPE_OBJECT,
        },
        body: SpaPodObjectBody { ty: 0, id: 0 },
        p_fmt: SpaPodProp {
            key: SPA_FORMAT_AUDIO_FORMAT,
            flags: 0,
            value: SpaPod { size: 4, ty: SPA_TYPE_ID },
        },
        v_fmt: SPA_AUDIO_FORMAT_F32_LE,
        pad1: 0,
        p_rate: SpaPodProp {
            key: SPA_FORMAT_AUDIO_RATE,
            flags: 0,
            value: SpaPod { size: 4, ty: SPA_TYPE_INT },
        },
        v_rate: 48000,
        pad2: 0,
        p_ch: SpaPodProp {
            key: SPA_FORMAT_AUDIO_CHANNELS,
            flags: 0,
            value: SpaPod { size: 4, ty: SPA_TYPE_INT },
        },
        v_ch: 2,
        pad3: 0,
    };
    let params = [&fmt_pod.pod as *const SpaPod];
    let res = unsafe { pw_stream_connect(stream, 1, u32::MAX, 0, params.as_ptr(), 1) };
    assert_eq!(res, 0, "pw_stream_connect failed: {}", res);
    assert_eq!(unsafe { pw_thread_loop_start(tl) }, 0);
    thread::sleep(Duration::from_millis(400));
    eprintln!("pw_sine_poc: frames_sent={}", ctx.frames_sent);
    unsafe {
        pw_thread_loop_stop(tl);
        pw_stream_destroy(stream);
        pw_thread_loop_destroy(tl);
    }
    pw_deinit();
}
