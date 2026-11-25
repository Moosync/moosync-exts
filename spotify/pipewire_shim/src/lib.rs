#![allow(clippy::missing_safety_doc)]

mod android_init;
pub mod dns;
pub mod event_loop;
mod ffi;
pub mod shm;
pub mod spa;
pub mod stream;

use std::ffi::{CStr, c_char, c_int, c_void};

pub use event_loop::{
    PwLoop, PwMainLoop, PwThreadLoop, pw_main_loop_destroy, pw_main_loop_get_loop,
    pw_main_loop_new, pw_main_loop_quit, pw_main_loop_run, pw_thread_loop_destroy,
    pw_thread_loop_get_loop, pw_thread_loop_lock, pw_thread_loop_new, pw_thread_loop_start,
    pw_thread_loop_stop, pw_thread_loop_unlock,
};
pub use spa::{SpaBuffer, SpaChunk, SpaData, SpaPod, SpaPodObjectBody, SpaPodProp};
pub use stream::{
    PwBuffer, PwProperties, PwProxy, PwStream, PwStreamEvents, PwStreamState,
    pw_stream_connect, pw_stream_dequeue_buffer, pw_stream_destroy, pw_stream_flush,
    pw_stream_get_state, pw_stream_new_simple, pw_stream_queue_buffer, pw_stream_set_active,
    pw_stream_set_control,
};

pub struct PwContext {
    _loop: *mut PwLoop,
}

pub struct PwCore {
    _context: *mut PwContext,
}

#[unsafe(no_mangle)]
pub extern "C" fn pw_init(_argc: *mut c_int, _argv: *mut *mut *mut c_char) {}

#[unsafe(no_mangle)]
pub extern "C" fn pw_deinit() {}

#[unsafe(no_mangle)]
pub extern "C" fn pw_get_library_version() -> *const c_char {
    c"0.3.80".as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn pw_properties_new(_key: *const c_char) -> *mut PwProperties {
    Box::into_raw(Box::new(PwProperties))
}

#[unsafe(no_mangle)]
pub extern "C" fn pw_context_new(
    main_loop: *mut PwLoop,
    _props: *mut PwProperties,
    _user_data_size: usize,
) -> *mut PwContext {
    Box::into_raw(Box::new(PwContext { _loop: main_loop }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_context_destroy(context: *mut PwContext) {
    if !context.is_null() {
        drop(unsafe { Box::from_raw(context) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pw_context_connect(
    context: *mut PwContext,
    _properties: *mut PwProperties,
    _user_data_size: usize,
) -> *mut PwCore {
    Box::into_raw(Box::new(PwCore { _context: context }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_core_disconnect(core: *mut PwCore) -> c_int {
    if !core.is_null() {
        drop(unsafe { Box::from_raw(core) });
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn pw_proxy_add_object_listener(
    _proxy: *mut PwProxy,
    _listener: *mut c_void,
    _funcs: *const c_void,
    _data: *mut c_void,
) {
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn pw_proxy_destroy(proxy: *mut PwProxy) {
    if !proxy.is_null() {
        drop(unsafe { Box::from_raw(proxy) });
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pw_shim_force_keep_symbols() -> *const c_char {
    CStr::from_bytes_with_nul(b"pw_shim\0").unwrap().as_ptr()
}
