use std::ffi::c_void;
use std::mem::size_of;

use crate::shm::{PW_AAUDIO_FMT_F32_LE, PW_AAUDIO_FMT_S16_LE};

pub const SPA_TYPE_ID: u32 = 3;
pub const SPA_TYPE_INT: u32 = 4;
pub const SPA_TYPE_OBJECT: u32 = 15;
pub const SPA_TYPE_CHOICE: u32 = 19;

pub const SPA_FORMAT_AUDIO_FORMAT: u32 = 0x10001;
pub const SPA_FORMAT_AUDIO_RATE: u32 = 0x10003;
pub const SPA_FORMAT_AUDIO_CHANNELS: u32 = 0x10004;

pub const SPA_AUDIO_FORMAT_S16_LE: u32 = 0x103;
pub const SPA_AUDIO_FORMAT_F32_LE: u32 = 0x11b;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SpaPod {
    pub size: u32,
    pub ty: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SpaPodObjectBody {
    pub ty: u32,
    pub id: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SpaPodProp {
    pub key: u32,
    pub flags: u32,
    pub value: SpaPod,
}

#[repr(C)]
pub struct SpaChunk {
    pub offset: u32,
    pub size: u32,
    pub stride: i32,
    pub flags: i32,
}

#[repr(C)]
pub struct SpaData {
    pub ty: u32,
    pub flags: u32,
    pub fd: i64,
    pub mapoffset: u32,
    pub maxsize: u32,
    pub data: *mut c_void,
    pub chunk: *mut SpaChunk,
}

#[repr(C)]
pub struct SpaBuffer {
    pub n_metas: u32,
    pub n_datas: u32,
    pub metas: *mut c_void,
    pub datas: *mut SpaData,
}

pub const fn spa_align8(n: usize) -> usize {
    (n + 7) & !7
}

pub unsafe fn read_pod_u32(pod: *const SpaPod) -> u32 {
    if pod.is_null() {
        return 0;
    }
    // SAFETY: Caller guarantees `pod` points to a valid `SpaPod` header and its body.
    let p = unsafe { *pod };
    if (p.ty == SPA_TYPE_ID || p.ty == SPA_TYPE_INT) && p.size >= 4 {
        // SAFETY: Pod payload begins immediately after the `SpaPod` header.
        let val_ptr = unsafe { pod.add(1) as *const u32 };
        return unsafe { *val_ptr };
    }
    if p.ty == SPA_TYPE_CHOICE && p.size as usize >= 8 + size_of::<SpaPod>() {
        // SAFETY: Choice body starts 8 bytes into the pod payload with a nested `SpaPod`.
        let body = unsafe { (pod.add(1) as *const u8).add(8) as *const SpaPod };
        let child = unsafe { *body };
        if (child.ty == SPA_TYPE_ID || child.ty == SPA_TYPE_INT) && child.size >= 4 {
            let val_ptr = unsafe { body.add(1) as *const u32 };
            return unsafe { *val_ptr };
        }
    }
    0
}

pub unsafe fn parse_format_pod(
    pod: *const SpaPod,
    out_fmt: &mut u32,
    out_rate: &mut u32,
    out_channels: &mut u32,
) {
    if pod.is_null() {
        return;
    }
    // SAFETY: Caller guarantees `pod` points to a valid `SpaPod`.
    let p = unsafe { *pod };
    if p.ty != SPA_TYPE_OBJECT || (p.size as usize) < size_of::<SpaPodObjectBody>() {
        return;
    }
    // SAFETY: Object body and properties lie within `p.size` bytes after `pod`.
    let base = unsafe { pod.add(1) as *const u8 };
    let end = unsafe { base.add(p.size as usize) };
    let mut cur = unsafe { base.add(size_of::<SpaPodObjectBody>()) };

    while unsafe { cur.add(size_of::<SpaPodProp>()) } <= end {
        let prop = unsafe { &*(cur as *const SpaPodProp) };
        let prop_total = size_of::<SpaPodProp>() + spa_align8(prop.value.size as usize);
        if unsafe { cur.add(prop_total) } > end {
            break;
        }
        let val_pod = &prop.value as *const SpaPod;
        if prop.key == SPA_FORMAT_AUDIO_FORMAT {
            let fmt = unsafe { read_pod_u32(val_pod) };
            if fmt == SPA_AUDIO_FORMAT_S16_LE {
                *out_fmt = PW_AAUDIO_FMT_S16_LE;
            } else if fmt == SPA_AUDIO_FORMAT_F32_LE {
                *out_fmt = PW_AAUDIO_FMT_F32_LE;
            }
        } else if prop.key == SPA_FORMAT_AUDIO_RATE {
            let rate = unsafe { read_pod_u32(val_pod) };
            if (8000..=384000).contains(&rate) {
                *out_rate = rate;
            }
        } else if prop.key == SPA_FORMAT_AUDIO_CHANNELS {
            let ch = unsafe { read_pod_u32(val_pod) };
            if (1..=8).contains(&ch) {
                *out_channels = ch;
            }
        }
        cur = unsafe { cur.add(prop_total) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_read_pod_u32_handles_null_id_int_and_choice() {
        assert_eq!(unsafe { read_pod_u32(ptr::null()) }, 0);

        #[repr(C)]
        struct IntPod {
            pod: SpaPod,
            val: u32,
        }
        let int_pod = IntPod {
            pod: SpaPod {
                size: 4,
                ty: SPA_TYPE_INT,
            },
            val: 44100,
        };
        assert_eq!(unsafe { read_pod_u32(&int_pod.pod) }, 44100);

        #[repr(C)]
        struct ChoicePod {
            pod: SpaPod,
            choice_type: u32,
            flags: u32,
            child: SpaPod,
            val: u32,
        }
        let choice_pod = ChoicePod {
            pod: SpaPod {
                size: (8 + size_of::<SpaPod>() + 4) as u32,
                ty: SPA_TYPE_CHOICE,
            },
            choice_type: 0,
            flags: 0,
            child: SpaPod {
                size: 4,
                ty: SPA_TYPE_ID,
            },
            val: SPA_AUDIO_FORMAT_S16_LE,
        };
        assert_eq!(
            unsafe { read_pod_u32(&choice_pod.pod) },
            SPA_AUDIO_FORMAT_S16_LE
        );
    }

    #[test]
    fn test_parse_format_pod_extracts_valid_fields_and_ignores_out_of_range() {
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
        let pod = FmtPod {
            pod: SpaPod {
                size: (size_of::<FmtPod>() - size_of::<SpaPod>()) as u32,
                ty: SPA_TYPE_OBJECT,
            },
            body: SpaPodObjectBody { ty: 0, id: 0 },
            p_fmt: SpaPodProp {
                key: SPA_FORMAT_AUDIO_FORMAT,
                flags: 0,
                value: SpaPod {
                    size: 4,
                    ty: SPA_TYPE_ID,
                },
            },
            v_fmt: SPA_AUDIO_FORMAT_S16_LE,
            pad1: 0,
            p_rate: SpaPodProp {
                key: SPA_FORMAT_AUDIO_RATE,
                flags: 0,
                value: SpaPod {
                    size: 4,
                    ty: SPA_TYPE_INT,
                },
            },
            v_rate: 44100,
            pad2: 0,
            p_ch: SpaPodProp {
                key: SPA_FORMAT_AUDIO_CHANNELS,
                flags: 0,
                value: SpaPod {
                    size: 4,
                    ty: SPA_TYPE_INT,
                },
            },
            v_ch: 2,
            pad3: 0,
        };

        let mut fmt = 0;
        let mut rate = 0;
        let mut channels = 0;
        unsafe { parse_format_pod(&pod.pod, &mut fmt, &mut rate, &mut channels) };
        assert_eq!(fmt, PW_AAUDIO_FMT_S16_LE);
        assert_eq!(rate, 44100);
        assert_eq!(channels, 2);

        let bad_pod = FmtPod {
            v_rate: 123, // below 8000
            v_ch: 99,    // above 8
            ..pod
        };
        let mut rate2 = 48000;
        let mut ch2 = 2;
        unsafe { parse_format_pod(&bad_pod.pod, &mut fmt, &mut rate2, &mut ch2) };
        assert_eq!(rate2, 48000);
        assert_eq!(ch2, 2);
    }
}
