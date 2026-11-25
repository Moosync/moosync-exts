use std::fs;
use std::path::Path;

const ENV_LD_PRELOAD: &str = "LD_PRELOAD";
const ENV_SSL_CERT_FILE: &str = "SSL_CERT_FILE";
const ENV_PIPEWIRE_CONFIG_DIR: &str = "PIPEWIRE_CONFIG_DIR";
const ANDROID_SYSTEM_CACERTS_DIR: &str = "/system/etc/security/cacerts";
const ANDROID_APEX_CACERTS_DIR: &str = "/apex/com.android.conscrypt/cacerts";
const CA_BUNDLE_FILENAME: &str = "cacert.pem";
const PIPEWIRE_CLIENT_CONF_FILENAME: &str = "client.conf";
const DEFAULT_PIPEWIRE_CLIENT_CONF: &[u8] = b"{}\n";

/// Builds a concatenated PEM bundle and `client.conf` inside `target_dir` from `cert_dir`.
pub(crate) fn populate_runtime_dir(target_dir: &Path, cert_dir: Option<&Path>) -> bool {
    let mut bundle_ready = false;
    if let Some(certs) = cert_dir
        && certs.is_dir()
    {
        let bundle_path = target_dir.join(CA_BUNDLE_FILENAME);
        let needs_write = match fs::metadata(&bundle_path) {
            Ok(meta) => meta.len() == 0,
            Err(_) => true,
        };
        if needs_write {
            let mut out = Vec::new();
            if let Ok(entries) = fs::read_dir(certs) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file()
                        && let Ok(bytes) = fs::read(&p)
                    {
                        out.extend_from_slice(&bytes);
                        if !bytes.ends_with(b"\n") {
                            out.push(b'\n');
                        }
                    }
                }
            }
            if !out.is_empty() {
                let _ = fs::write(&bundle_path, &out);
            }
        }
        bundle_ready = fs::metadata(&bundle_path).map(|m| m.len() > 0).unwrap_or(false);
    }

    let client_conf = target_dir.join(PIPEWIRE_CLIENT_CONF_FILENAME);
    if !client_conf.exists() {
        let _ = fs::write(&client_conf, DEFAULT_PIPEWIRE_CLIENT_CONF);
    }
    bundle_ready
}

/// Populates Android CA bundle and `client.conf` inside the glibc cache directory before `main()`.
fn ensure_android_runtime_files() {
    let Some(preload) = std::env::var_os(ENV_LD_PRELOAD) else {
        return;
    };
    let Some(glibc_dir) = Path::new(&preload).parent() else {
        return;
    };
    let cert_dir = if Path::new(ANDROID_SYSTEM_CACERTS_DIR).is_dir() {
        Some(Path::new(ANDROID_SYSTEM_CACERTS_DIR))
    } else if Path::new(ANDROID_APEX_CACERTS_DIR).is_dir() {
        Some(Path::new(ANDROID_APEX_CACERTS_DIR))
    } else {
        None
    };

    if populate_runtime_dir(glibc_dir, cert_dir) {
        let bundle_path = glibc_dir.join(CA_BUNDLE_FILENAME);
        // SAFETY: Runs in `.init_array` before `main()` and before any threads are spawned.
        unsafe { std::env::set_var(ENV_SSL_CERT_FILE, &bundle_path) };
    }
    // SAFETY: Runs in `.init_array` before `main()` and before any threads are spawned.
    unsafe { std::env::set_var(ENV_PIPEWIRE_CONFIG_DIR, glibc_dir) };
}

extern "C" fn init_android_runtime() {
    ensure_android_runtime_files();
}

#[used]
#[unsafe(link_section = ".init_array")]
static INIT_ANDROID_RUNTIME: extern "C" fn() = init_android_runtime;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pw_android_init_{}_{}", label, nanos));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_populate_runtime_dir_creates_bundle_and_client_conf() {
        let root = make_temp_dir("bundle");
        let certs = root.join("certs");
        let target = root.join("glibc");
        fs::create_dir_all(&certs).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(certs.join("a.0"), b"CERT_A").unwrap();
        fs::write(certs.join("b.0"), b"CERT_B\n").unwrap();

        let ready = populate_runtime_dir(&target, Some(&certs));

        assert!(ready);
        let bundle = fs::read_to_string(target.join(CA_BUNDLE_FILENAME)).unwrap();
        assert!(bundle.contains("CERT_A\n"));
        assert!(bundle.contains("CERT_B\n"));
        assert_eq!(
            fs::read(target.join(PIPEWIRE_CLIENT_CONF_FILENAME)).unwrap(),
            DEFAULT_PIPEWIRE_CLIENT_CONF
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_populate_runtime_dir_without_certs_creates_client_conf_only() {
        let root = make_temp_dir("nocerts");
        let missing = root.join("missing");

        let ready = populate_runtime_dir(&root, Some(&missing));

        assert!(!ready);
        assert!(!root.join(CA_BUNDLE_FILENAME).exists());
        assert_eq!(
            fs::read(root.join(PIPEWIRE_CLIENT_CONF_FILENAME)).unwrap(),
            DEFAULT_PIPEWIRE_CLIENT_CONF
        );
        let _ = fs::remove_dir_all(&root);
    }
}
