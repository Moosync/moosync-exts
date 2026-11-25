use std::collections::HashSet;

use object::LittleEndian;
use object::elf::{FileHeader64, PF_X, PT_GNU_RELRO, PT_LOAD, ProgramHeader64};
use object::read::elf::{FileHeader, ProgramHeader};

const PAGE_16K: u64 = 0x4000;

struct LoadSegment {
    ph_off: usize,
    flags: u32,
    offset: usize,
    vaddr: u64,
    filesz: usize,
    memsz: u64,
    align: u64,
}

fn collect_load_segments(
    phdrs: &[ProgramHeader64<LittleEndian>],
    e_phoff: usize,
    e_phentsize: usize,
) -> Option<(Vec<LoadSegment>, HashSet<u64>)> {
    let endian = LittleEndian;
    let mut loads = Vec::new();
    let mut needs_align = false;
    let mut exec_pages = HashSet::new();

    for (idx, ph) in phdrs.iter().enumerate() {
        if ph.p_type(endian) != PT_LOAD {
            continue;
        }
        let flags = ph.p_flags(endian);
        let offset = ph.p_offset(endian);
        let vaddr = ph.p_vaddr(endian);
        let filesz = ph.p_filesz(endian) as usize;
        let memsz = ph.p_memsz(endian);
        let align = ph.p_align(endian);

        if (vaddr % PAGE_16K) != (offset % PAGE_16K) || align < PAGE_16K {
            needs_align = true;
        }
        if (flags & PF_X) != 0 {
            let start_pg = vaddr / PAGE_16K;
            let end_pg = (vaddr + memsz.max(1) - 1) / PAGE_16K;
            exec_pages.extend(start_pg..=end_pg);
        }
        loads.push(LoadSegment {
            ph_off: e_phoff + idx * e_phentsize,
            flags,
            offset: offset as usize,
            vaddr,
            filesz,
            memsz,
            align,
        });
    }

    if !needs_align || loads.is_empty() {
        return None;
    }
    Some((loads, exec_pages))
}

fn write_aligned_segments(
    elf_bytes: &[u8],
    loads: &[LoadSegment],
    exec_pages: &HashSet<u64>,
) -> (Vec<u8>, Vec<(usize, usize, usize)>) {
    let mut out = elf_bytes[..loads[0].offset].to_vec();
    let mut orig_to_new = Vec::with_capacity(loads.len());

    for (idx, seg) in loads.iter().enumerate() {
        if idx > 0 {
            let prev_end = loads[idx - 1].offset + loads[idx - 1].filesz;
            if seg.offset > prev_end {
                out.extend_from_slice(&elf_bytes[prev_end..seg.offset]);
            }
        }
        let cur_off = out.len() as u64;
        let pad = ((seg.vaddr % PAGE_16K) + PAGE_16K - (cur_off % PAGE_16K)) % PAGE_16K;
        out.resize(out.len() + pad as usize, 0);
        let new_off = out.len();
        orig_to_new.push((seg.offset, seg.offset + seg.filesz, new_off - seg.offset));
        out.extend_from_slice(&elf_bytes[seg.offset..seg.offset + seg.filesz]);

        if (seg.flags & PF_X) == 0 {
            let start_pg = seg.vaddr / PAGE_16K;
            let end_pg = (seg.vaddr + seg.memsz.max(1) - 1) / PAGE_16K;
            if (start_pg..=end_pg).any(|pg| exec_pages.contains(&pg)) {
                out[seg.ph_off + 4..seg.ph_off + 8]
                    .copy_from_slice(&(seg.flags | PF_X).to_le_bytes());
            }
        }
        out[seg.ph_off + 8..seg.ph_off + 16].copy_from_slice(&(new_off as u64).to_le_bytes());
        if seg.align < PAGE_16K {
            out[seg.ph_off + 48..seg.ph_off + 56].copy_from_slice(&PAGE_16K.to_le_bytes());
        }
    }

    (out, orig_to_new)
}

fn patch_non_load_phdrs(
    out: &mut [u8],
    phdrs: &[ProgramHeader64<LittleEndian>],
    e_phoff: usize,
    e_phentsize: usize,
    orig_to_new: &[(usize, usize, usize)],
    exec_pages: &HashSet<u64>,
) {
    let endian = LittleEndian;
    for (idx, ph) in phdrs.iter().enumerate() {
        let ph_off = e_phoff + idx * e_phentsize;
        let p_type = ph.p_type(endian);
        let mut p_offset = ph.p_offset(endian);
        if p_type != PT_LOAD && p_offset != 0 {
            for &(s_start, s_end, delta) in orig_to_new {
                if (p_offset as usize) >= s_start && (p_offset as usize) < s_end {
                    p_offset += delta as u64;
                    out[ph_off + 8..ph_off + 16].copy_from_slice(&p_offset.to_le_bytes());
                    break;
                }
            }
        }
        if p_type == PT_GNU_RELRO {
            let p_vaddr = ph.p_vaddr(endian);
            if exec_pages.contains(&(p_vaddr / PAGE_16K)) {
                let new_vaddr = (p_vaddr + PAGE_16K - 1) & !(PAGE_16K - 1);
                let delta = new_vaddr - p_vaddr;
                let new_off = p_offset + delta;
                let new_paddr = ph.p_paddr(endian) + delta;
                let p_filesz = ph.p_filesz(endian);
                let p_memsz = ph.p_memsz(endian);
                if p_memsz > delta {
                    out[ph_off + 8..ph_off + 16].copy_from_slice(&new_off.to_le_bytes());
                    out[ph_off + 16..ph_off + 24].copy_from_slice(&new_vaddr.to_le_bytes());
                    out[ph_off + 24..ph_off + 32].copy_from_slice(&new_paddr.to_le_bytes());
                    out[ph_off + 32..ph_off + 40]
                        .copy_from_slice(&p_filesz.saturating_sub(delta).to_le_bytes());
                    out[ph_off + 40..ph_off + 48].copy_from_slice(&(p_memsz - delta).to_le_bytes());
                }
            }
        }
    }
}

/// Aligns ELF64 PT_LOAD segments to 16 KiB boundaries when needed for Android 15+ (16 KiB pages).
pub fn align_elf_16k_if_needed(elf_bytes: &[u8]) -> Option<Vec<u8>> {
    let endian = LittleEndian;
    let header = FileHeader64::<LittleEndian>::parse(elf_bytes).ok()?;
    let phdrs = header.program_headers(endian, elf_bytes).ok()?;
    let e_phoff = header.e_phoff.get(endian) as usize;
    let e_phentsize = header.e_phentsize.get(endian) as usize;

    let (loads, exec_pages) = collect_load_segments(phdrs, e_phoff, e_phentsize)?;
    let (mut out, orig_to_new) = write_aligned_segments(elf_bytes, &loads, &exec_pages);
    patch_non_load_phdrs(
        &mut out,
        phdrs,
        e_phoff,
        e_phentsize,
        &orig_to_new,
        &exec_pages,
    );

    out[40..48].fill(0);
    out[60..64].fill(0);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_u16(buf: &mut [u8], off: usize, val: u16) {
        buf[off..off + 2].copy_from_slice(&val.to_le_bytes());
    }

    fn write_u32(buf: &mut [u8], off: usize, val: u32) {
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    }

    fn write_u64(buf: &mut [u8], off: usize, val: u64) {
        buf[off..off + 8].copy_from_slice(&val.to_le_bytes());
    }

    fn build_elf64(loads: &[(u32, u64, u64, u64, u64, u64)], size: usize) -> Vec<u8> {
        let mut buf = vec![0u8; size];
        buf[0..4].copy_from_slice(b"\x7fELF");
        buf[4] = 2; // ELFCLASS64
        buf[5] = 1; // ELFDATA2LSB
        buf[6] = 1; // EV_CURRENT
        write_u16(&mut buf, 16, 3); // ET_DYN
        write_u16(&mut buf, 18, 62); // EM_X86_64
        write_u32(&mut buf, 20, 1);
        write_u64(&mut buf, 32, 64); // e_phoff
        write_u16(&mut buf, 52, 64); // e_ehsize
        write_u16(&mut buf, 54, 56); // e_phentsize
        write_u16(&mut buf, 56, loads.len() as u16); // e_phnum

        for (idx, &(flags, offset, vaddr, filesz, memsz, align)) in loads.iter().enumerate() {
            let off = 64 + idx * 56;
            write_u32(&mut buf, off, PT_LOAD);
            write_u32(&mut buf, off + 4, flags);
            write_u64(&mut buf, off + 8, offset);
            write_u64(&mut buf, off + 16, vaddr);
            write_u64(&mut buf, off + 24, vaddr);
            write_u64(&mut buf, off + 32, filesz);
            write_u64(&mut buf, off + 40, memsz);
            write_u64(&mut buf, off + 48, align);
        }
        buf
    }

    #[test]
    fn test_align_elf_16k_if_needed_non_elf_and_already_aligned() {
        assert!(align_elf_16k_if_needed(b"not-elf").is_none());

        let aligned = build_elf64(&[(PF_X | 4, 0x4000, 0x4000, 0x100, 0x100, PAGE_16K)], 0x5000);
        assert!(align_elf_16k_if_needed(&aligned).is_none());
    }

    #[test]
    fn test_align_elf_16k_if_needed_aligns_segments_and_propagates_pf_x() {
        let raw = build_elf64(
            &[
                (PF_X | 4, 0x0200, 0x1000, 0x0200, 0x0200, 0x1000),
                (4, 0x0400, 0x2000, 0x0100, 0x0100, 0x1000),
            ],
            0x1000,
        );

        let out = align_elf_16k_if_needed(&raw).expect("expected 16k alignment");
        let header = FileHeader64::<LittleEndian>::parse(&out[..]).unwrap();
        let phdrs = header.program_headers(LittleEndian, &out[..]).unwrap();
        assert_eq!(phdrs.len(), 2);
        for ph in phdrs {
            let vaddr = ph.p_vaddr(LittleEndian);
            let offset = ph.p_offset(LittleEndian);
            assert_eq!(vaddr % PAGE_16K, offset % PAGE_16K);
            assert!(ph.p_align(LittleEndian) >= PAGE_16K);
        }
        assert_ne!(phdrs[1].p_flags(LittleEndian) & PF_X, 0);
    }
}
