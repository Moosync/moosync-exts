pub fn get_glibc_tar_gz(arch: &str) -> Option<&'static [u8]> {
    match arch {
        "x86_64" => Some(include_bytes!(env!("GLIBC_X86_64_TAR_GZ"))),
        _ => None,
    }
}

pub fn get_aaudio_sink_bin(arch: &str) -> Option<&'static [u8]> {
    match arch {
        "x86_64" => Some(include_bytes!(env!("AAUDIO_SINK_X86_64"))),
        _ => None,
    }
}
