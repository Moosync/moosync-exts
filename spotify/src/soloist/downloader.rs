use std::fs;
use std::io::Cursor;
use std::path::Path;

use moosync_edk::{MoosyncError, MoosyncResult};

// Platform tags sourced from Rust std documentation:
// https://doc.rust-lang.org/std/env/consts/constant.OS.html
// https://doc.rust-lang.org/std/env/consts/constant.ARCH.html
// Soloist download archives documentation:
// https://developer.spotify.com/documentation/soloist/reference/downloads-and-updates
pub fn get_soloist_download_url(os: &str, arch: &str) -> MoosyncResult<String> {
    let archive_name = match (os, arch) {
        ("linux", "x86_64") => "soloist_release_x86_64.tar.gz",
        ("linux", "aarch64") => "soloist_release_arm64.tar.gz",
        ("darwin" | "macos", "x86_64") => "soloist_release_osx_x86_64.tar.gz",
        ("darwin" | "macos", "aarch64") => "soloist_release_osx_arm64.tar.gz",
        ("windows", "x86_64") => "soloist_release_windows_x86_64.zip",
        _ => {
            return Err(MoosyncError::String(format!(
                "Unsupported platform for Spotify Soloist: os={}, arch={}",
                os, arch
            )))
        }
    };
    Ok(format!(
        "https://soloist-builds.spotifycdn.com/{}",
        archive_name
    ))
}

fn extract_zip_binary(archive_bytes: &[u8], dest_path: &Path) -> MoosyncResult<()> {
    let cursor = Cursor::new(archive_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| MoosyncError::String(format!("Failed to read zip archive: {}", e)))?;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| MoosyncError::String(format!("Failed to read zip entry: {}", e)))?;
        let name = file.name();
        if name == "soloist"
            || name == "soloist.exe"
            || name.ends_with("/soloist")
            || name.ends_with("/soloist.exe")
        {
            let mut out = fs::File::create(dest_path).map_err(|e| {
                MoosyncError::String(format!("Failed to create soloist binary file: {}", e))
            })?;
            std::io::copy(&mut file, &mut out).map_err(|e| {
                MoosyncError::String(format!("Failed to write soloist binary file: {}", e))
            })?;
            return Ok(());
        }
    }
    Err(MoosyncError::String(
        "soloist binary not found in zip archive".into(),
    ))
}

fn extract_tar_gz_binary(archive_bytes: &[u8], dest_path: &Path) -> MoosyncResult<()> {
    let cursor = Cursor::new(archive_bytes);
    let gz = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(gz);
    let entries = archive
        .entries()
        .map_err(|e| MoosyncError::String(format!("Failed to read tar.gz entries: {}", e)))?;

    for entry in entries {
        let mut entry = entry
            .map_err(|e| MoosyncError::String(format!("Failed to read tar.gz entry: {}", e)))?;
        let path = entry
            .path()
            .map_err(|e| MoosyncError::String(format!("Failed to read tar.gz path: {}", e)))?;
        if path.file_name().and_then(|n| n.to_str()) == Some("soloist") {
            let mut out = fs::File::create(dest_path).map_err(|e| {
                MoosyncError::String(format!("Failed to create soloist binary file: {}", e))
            })?;
            std::io::copy(&mut entry, &mut out).map_err(|e| {
                MoosyncError::String(format!("Failed to write soloist binary file: {}", e))
            })?;
            return Ok(());
        }
    }

    Err(MoosyncError::String(
        "soloist binary not found in tar.gz archive".into(),
    ))
}

pub fn extract_soloist_binary(
    archive_bytes: &[u8],
    is_zip: bool,
    dest_path: &Path,
) -> MoosyncResult<()> {
    if is_zip {
        extract_zip_binary(archive_bytes, dest_path)
    } else {
        extract_tar_gz_binary(archive_bytes, dest_path)
    }
}
