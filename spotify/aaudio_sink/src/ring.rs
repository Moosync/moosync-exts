use std::ffi::{c_int, c_void};
use std::mem::{size_of, zeroed};
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use pw_aaudio_shm::{
    Cmsghdr, Iovec, Msghdr, PW_AAUDIO_FMT_F32_LE, PW_AAUDIO_FMT_S16_LE, PwAaudioShm, SCM_RIGHTS,
    SOL_SOCKET, cmsg_align, cmsg_space,
};

unsafe extern "C" {
    fn sendmsg(sockfd: c_int, msg: *const Msghdr, flags: c_int) -> isize;
}

pub fn apply_volume_s16(buf: &mut [u8], vol_q16: u32) {
    if vol_q16 >= 0x10000 {
        return;
    }
    for chunk in buf.chunks_exact_mut(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        let scaled = (((sample as i32) * (vol_q16 as i32)) >> 16) as i16;
        chunk.copy_from_slice(&scaled.to_le_bytes());
    }
}

pub fn apply_volume_f32(buf: &mut [u8], vol_q16: u32) {
    if vol_q16 >= 0x10000 {
        return;
    }
    let gain = vol_q16 as f32 / 65536.0;
    for chunk in buf.chunks_exact_mut(4) {
        let sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        chunk.copy_from_slice(&(sample * gain).to_le_bytes());
    }
}

/// Drains up to `out.len()` bytes from `shm` ring buffer into `out`, zero-filling any remainder.
/// Returns `true` when audio was consumed (so the caller should signal `wake_efd`).
pub fn consume_ring_to_buf(
    shm: &mut PwAaudioShm,
    out: &mut [u8],
    last_flush_seq: &mut u32,
) -> bool {
    // SAFETY: `shm.flush_seq` is aligned `u32` inside `PwAaudioShm`.
    let flush_seq = unsafe { AtomicU32::from_ptr(&mut shm.flush_seq) }.load(Ordering::Acquire);
    if flush_seq != *last_flush_seq {
        // SAFETY: `shm.write_idx` / `read_idx` are aligned `u32` SPSC indices inside `PwAaudioShm`.
        let w = unsafe { AtomicU32::from_ptr(&mut shm.write_idx) }.load(Ordering::Acquire);
        // SAFETY: Advancing `read_idx` to `write_idx` discards buffered audio on flush.
        unsafe { AtomicU32::from_ptr(&mut shm.read_idx) }.store(w, Ordering::Release);
        *last_flush_seq = flush_seq;
        out.fill(0);
        return false;
    }

    // SAFETY: `shm.active` is an aligned `u32` atomic flag inside `PwAaudioShm`.
    if unsafe { AtomicU32::from_ptr(&mut shm.active) }.load(Ordering::Acquire) == 0 {
        out.fill(0);
        return false;
    }

    // SAFETY: `shm.write_idx` and `shm.read_idx` are aligned `u32` SPSC ring indices inside `PwAaudioShm`.
    let w = unsafe { AtomicU32::from_ptr(&mut shm.write_idx) }.load(Ordering::Acquire);
    // SAFETY: Consumer-owned `read_idx` read with relaxed ordering.
    let r = unsafe { AtomicU32::from_ptr(&mut shm.read_idx) }.load(Ordering::Relaxed);
    let avail = (w.wrapping_sub(r) as usize).min(shm.ring_bytes as usize);
    let to_read = out.len().min(avail);

    if to_read > 0 {
        let mask = (shm.ring_bytes as usize) - 1;
        let pos = (r as usize) & mask;
        let first = (shm.ring_bytes as usize - pos).min(to_read);
        out[..first].copy_from_slice(&shm.data[pos..pos + first]);
        if to_read > first {
            out[first..to_read].copy_from_slice(&shm.data[..to_read - first]);
        }
        // SAFETY: Consumer advances `read_idx` after reading from `shm.data`.
        unsafe { AtomicU32::from_ptr(&mut shm.read_idx) }
            .store(r.wrapping_add(to_read as u32), Ordering::Release);

        // SAFETY: `shm.volume_q16` is an aligned `u32` inside `PwAaudioShm`.
        let vol_q16 = unsafe { AtomicU32::from_ptr(&mut shm.volume_q16) }.load(Ordering::Relaxed);
        if shm.format == PW_AAUDIO_FMT_S16_LE {
            apply_volume_s16(&mut out[..to_read], vol_q16);
        } else if shm.format == PW_AAUDIO_FMT_F32_LE {
            apply_volume_f32(&mut out[..to_read], vol_q16);
        }
    }
    if to_read < out.len() {
        out[to_read..].fill(0);
    }
    true
}

pub fn send_shm_fds(client_fd: c_int, shm_fd: c_int, wake_efd: c_int) -> bool {
    let mut byte = b'S';
    let mut iov = Iovec {
        iov_base: (&mut byte as *mut u8).cast::<c_void>(),
        iov_len: 1,
    };
    let fds = [shm_fd, wake_efd];
    let mut cmsg_buf = [0u8; cmsg_space(size_of::<[c_int; 2]>())];
    // SAFETY: `Msghdr` is a plain C POD struct of zeros before field initialization.
    let mut msg: Msghdr = unsafe { zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = cmsg_buf.as_mut_ptr().cast::<c_void>();
    msg.msg_controllen = cmsg_buf.len();

    // SAFETY: `cmsg_buf` is sized via `cmsg_space(size_of::<[c_int; 2]>())` and aligned for `Cmsghdr`.
    let cmsg = unsafe { &mut *(cmsg_buf.as_mut_ptr() as *mut Cmsghdr) };
    cmsg.cmsg_level = SOL_SOCKET;
    cmsg.cmsg_type = SCM_RIGHTS;
    cmsg.cmsg_len = cmsg_align(size_of::<Cmsghdr>()) + size_of::<[c_int; 2]>();
    // SAFETY: `data_ptr` lies within `cmsg_buf` and `client_fd`/`msg` are valid for `sendmsg`.
    unsafe {
        let data_ptr = cmsg_buf.as_mut_ptr().add(cmsg_align(size_of::<Cmsghdr>()));
        ptr::copy_nonoverlapping(
            fds.as_ptr().cast::<u8>(),
            data_ptr,
            size_of::<[c_int; 2]>(),
        );
        sendmsg(client_fd, &msg, 0) == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pw_aaudio_shm::{PW_AAUDIO_SHM_MAGIC, PW_AAUDIO_SHM_VERSION, PW_SHM_RING_BYTES};

    fn new_test_shm() -> Box<PwAaudioShm> {
        let mut shm = Box::new(unsafe { zeroed::<PwAaudioShm>() });
        shm.magic = PW_AAUDIO_SHM_MAGIC;
        shm.version = PW_AAUDIO_SHM_VERSION;
        shm.ring_bytes = PW_SHM_RING_BYTES as u32;
        shm.format = PW_AAUDIO_FMT_S16_LE;
        shm.volume_q16 = 0x10000;
        shm.active = 1;
        shm
    }

    #[test]
    fn test_apply_volume_s16_and_f32() {
        let mut s16 = [0u8; 4];
        s16[..2].copy_from_slice(&1000i16.to_le_bytes());
        s16[2..].copy_from_slice(&(-2000i16).to_le_bytes());
        apply_volume_s16(&mut s16, 0x8000); // 0.5x
        assert_eq!(i16::from_le_bytes([s16[0], s16[1]]), 500);
        assert_eq!(i16::from_le_bytes([s16[2], s16[3]]), -1000);

        let mut f32_buf = [0u8; 4];
        f32_buf.copy_from_slice(&1.0f32.to_le_bytes());
        apply_volume_f32(&mut f32_buf, 0x4000); // 0.25x
        assert!((f32::from_le_bytes(f32_buf) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_consume_ring_to_buf_wrap_around_and_flush() {
        let mut shm = new_test_shm();
        let ring_len = shm.ring_bytes as usize;
        shm.read_idx = (ring_len - 2) as u32;
        shm.write_idx = (ring_len + 2) as u32;
        shm.data[ring_len - 2] = 0x10;
        shm.data[ring_len - 1] = 0x00;
        shm.data[0] = 0x20;
        shm.data[1] = 0x00;

        let mut out = [0xFFu8; 8];
        let mut flush_seq = 0;
        assert!(consume_ring_to_buf(&mut shm, &mut out, &mut flush_seq));
        assert_eq!(&out[..4], &[0x10, 0x00, 0x20, 0x00]);
        assert_eq!(&out[4..], &[0, 0, 0, 0]);
        assert_eq!(shm.read_idx, shm.write_idx);

        shm.write_idx = shm.write_idx.wrapping_add(4);
        shm.flush_seq = 1;
        out.fill(0xAA);
        assert!(!consume_ring_to_buf(&mut shm, &mut out, &mut flush_seq));
        assert_eq!(out, [0u8; 8]);
        assert_eq!(shm.read_idx, shm.write_idx);
        assert_eq!(flush_seq, 1);
    }

    #[test]
    fn test_consume_ring_to_buf_inactive_outputs_silence() {
        let mut shm = new_test_shm();
        shm.active = 0;
        shm.write_idx = 4;
        let mut out = [0xFFu8; 4];
        let mut flush_seq = 0;

        assert!(!consume_ring_to_buf(&mut shm, &mut out, &mut flush_seq));
        assert_eq!(out, [0u8; 4]);
        assert_eq!(shm.read_idx, 0);
    }
}
