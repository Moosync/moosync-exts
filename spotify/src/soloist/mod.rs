pub mod elf_align;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use futures::executor::block_on;
use moosync_edk::{
    api::{duration_to_proto, get_platform, notify_player_event, spawn_process},
    info,
    types::{
        player_event, preference_value, ChildProcess, Duration as ProtoDuration, LoadingEvent,
        NotifyPlayerEventRequest, PlayerEvent, PreferenceItem, PreferenceType, PreferenceValue,
        Song, StatusDot, StatusDotColor, StatusDotList, WebSocketClient, WebSocketMessage,
    },
    warn, MoosyncError, MoosyncResult,
};

mod client;
mod downloader;

pub use client::{SoloistPosition, SoloistWsCommand, SoloistWsEvent, SoloistWsResponse};
pub use downloader::{align_elf_16k_if_needed, extract_soloist_binary, get_soloist_download_url};

pub const SOLOIST_BIN_PATH: &str = "/cache/bin/soloist";
pub const SOLOIST_ANDROID_X86_64_BIN_PATH: &str = "/cache/bin/soloist_android_x86_64";
pub const AAUDIO_SINK_BIN_PATH: &str = "/cache/bin/aaudio_sink";
pub const SOLOIST_GLIBC_DIR: &str = "/cache/glibc";
pub const PW_ANDROID_SOCK_PATH: &str = "@moosync_pw_aaudio";
pub const SOLOIST_DATA_DIR: &str = "/cache/soloist_data";
pub const SOLOIST_CACHE_DIR: &str = "/cache/soloist_data/cache";
pub const DEFAULT_WS_PORT: u16 = 9090;
pub const SOLOIST_DEVICE_NAME: &str = "Moosync";
pub const EXPIRATION_EXIT_CODE: i32 = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoloistStatus {
    Inactive(String),
    Discovery(String),
    Connected(String),
    Error(String),
}

#[derive(Clone)]
struct SoloistConnectContext {
    ws: Arc<Mutex<Option<WebSocketClient>>>,
    is_active: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    status: Arc<Mutex<SoloistStatus>>,
    is_paired: Arc<Mutex<bool>>,
    port: u16,
}

impl SoloistConnectContext {
    fn on_output(&self, data: &[u8]) {
        if self.connected.load(Ordering::SeqCst) {
            return;
        }
        let text = String::from_utf8_lossy(data);
        if text.contains("websocket server listening on")
            && connect_socket_once(&self.ws, &self.is_active, self.port).is_ok()
        {
            self.connected.store(true, Ordering::SeqCst);
            *self.is_paired.lock().unwrap() = true;
            *self.status.lock().unwrap() = SoloistStatus::Connected("Paired and ready".into());
        }
    }

    fn on_exit(&self, code: i32) {
        *self.status.lock().unwrap() =
            SoloistStatus::Error(format!("Spotify Soloist exited (code: {})", code));
    }
}

pub struct SoloistPlayer {
    pairing_process: Mutex<Option<ChildProcess>>,
    process: Mutex<Option<ChildProcess>>,
    aaudio_process: Mutex<Option<ChildProcess>>,
    ws: Arc<Mutex<Option<WebSocketClient>>>,
    port: Mutex<u16>,
    is_paired: Arc<Mutex<bool>>,
    is_active: Arc<AtomicBool>,
    status: Arc<Mutex<SoloistStatus>>,
    pending_uri: Mutex<Option<String>>,
}

impl SoloistPlayer {
    pub fn new() -> Self {
        let is_paired = Self::has_stored_session();
        let initial_status = if is_paired {
            SoloistStatus::Connected("Paired and ready".into())
        } else {
            SoloistStatus::Inactive("Not running".into())
        };
        Self {
            pairing_process: Mutex::new(None),
            process: Mutex::new(None),
            aaudio_process: Mutex::new(None),
            ws: Arc::new(Mutex::new(None)),
            port: Mutex::new(DEFAULT_WS_PORT),
            is_paired: Arc::new(Mutex::new(is_paired)),
            is_active: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new(initial_status)),
            pending_uri: Mutex::new(None),
        }
    }

    pub fn set_status(&self, status: SoloistStatus) {
        *self.status.lock().unwrap() = status;
    }

    pub fn status_preference_item(&self) -> PreferenceItem {
        let status = self.status.lock().unwrap().clone();
        let (title, subtitle, color) = match status {
            SoloistStatus::Inactive(msg) => ("Inactive", msg, StatusDotColor::Off),
            SoloistStatus::Discovery(msg) => ("Discovery", msg, StatusDotColor::Warn),
            SoloistStatus::Connected(msg) => ("Connected", msg, StatusDotColor::Info),
            SoloistStatus::Error(msg) => ("Error", msg, StatusDotColor::Error),
        };
        PreferenceItem {
            id: "soloist_status".into(),
            pref_type: PreferenceType::StatusGroup as i32,
            title: "Soloist Status".into(),
            subtitle: "Current state of the Spotify Soloist playback daemon".into(),
            value: Some(PreferenceValue {
                value: Some(preference_value::Value::StatusList(StatusDotList {
                    values: vec![StatusDot {
                        title: title.into(),
                        subtitle,
                        color: color as i32,
                    }],
                })),
            }),
            ..Default::default()
        }
    }

    pub fn is_paired(&self) -> bool {
        let mut paired = self.is_paired.lock().unwrap();
        if !*paired && Self::has_stored_session() {
            *paired = true;
        }
        *paired
    }

    fn has_stored_session() -> bool {
        let users_dir = Path::new(SOLOIST_DATA_DIR).join("settings").join("Users");
        if let Ok(user_entries) = fs::read_dir(&users_dir)
            && user_entries
                .flatten()
                .any(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        {
            return true;
        }

        let Ok(entries) = fs::read_dir(SOLOIST_DATA_DIR) else {
            return false;
        };
        entries.flatten().any(|entry| {
            if !entry.path().is_file() {
                return false;
            }
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            !name_str.starts_with('.')
                && !name_str.ends_with(".pid")
                && !name_str.ends_with(".addr")
                && !name_str.ends_with(".port")
        })
    }

    pub fn set_port(&self, port: u16) {
        let mut p = self.port.lock().unwrap();
        *p = port;
    }

    pub fn get_port(&self) -> u16 {
        *self.port.lock().unwrap()
    }

    fn soloist_bin_path() -> &'static str {
        if let Ok(platform) = get_platform() {
            if platform.os == "android" && platform.arch == "x86_64" {
                return SOLOIST_ANDROID_X86_64_BIN_PATH;
            }
        }
        SOLOIST_BIN_PATH
    }

    fn extract_glibc_archive(arch: &str) -> MoosyncResult<()> {
        let glibc_dir = Path::new(SOLOIST_GLIBC_DIR);
        if glibc_dir.join("libpipewire-0.3.so.0").exists() && glibc_dir.join("libc.so.6").exists() {
            return Ok(());
        }
        let archive_bytes = glibc_bundle::get_glibc_tar_gz(arch).ok_or_else(|| {
            MoosyncError::String(format!("Bundled glibc not yet available for arch: {}", arch))
        })?;
        fs::create_dir_all(glibc_dir).map_err(|e| {
            MoosyncError::String(format!("Failed to create glibc directory: {}", e))
        })?;
        let gz = flate2::read::GzDecoder::new(std::io::Cursor::new(archive_bytes));
        let mut archive = tar::Archive::new(gz);
        let entries = archive.entries().map_err(|e| {
            MoosyncError::String(format!("Failed to read bundled glibc entries: {}", e))
        })?;
        for entry_res in entries {
            let mut entry = entry_res.map_err(|e| {
                MoosyncError::String(format!("Failed to read bundled glibc entry: {}", e))
            })?;
            if !entry.header().entry_type().is_file() {
                continue;
            }
            let entry_path = entry.path().map_err(|e| {
                MoosyncError::String(format!("Failed to read bundled glibc entry path: {}", e))
            })?;
            let Some(file_name) = entry_path.file_name() else {
                continue;
            };
            let dest = glibc_dir.join(file_name);
            let mut out_file = fs::File::create(&dest).map_err(|e| {
                MoosyncError::String(format!("Failed to create {}: {}", dest.display(), e))
            })?;
            std::io::copy(&mut entry, &mut out_file).map_err(|e| {
                MoosyncError::String(format!("Failed to write {}: {}", dest.display(), e))
            })?;
        }
        Ok(())
    }

    fn extract_aaudio_sink(arch: &str) -> MoosyncResult<()> {
        let sink_path = Path::new(AAUDIO_SINK_BIN_PATH);
        if sink_path.exists() {
            return Ok(());
        }
        let Some(sink_bytes) = glibc_bundle::get_aaudio_sink_bin(arch) else {
            return Ok(());
        };
        if let Some(parent) = sink_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                MoosyncError::String(format!("Failed to create aaudio_sink directory: {}", e))
            })?;
        }
        fs::write(sink_path, sink_bytes).map_err(|e| {
            MoosyncError::String(format!("Failed to write aaudio_sink binary: {}", e))
        })
    }

    pub fn ensure_glibc_extracted(&self, arch: &str) -> MoosyncResult<()> {
        Self::extract_glibc_archive(arch)?;
        Self::extract_aaudio_sink(arch)
    }

    pub fn ensure_binary_downloaded(&self, is_premium: bool) -> MoosyncResult<()> {
        if !is_premium {
            return Err(MoosyncError::String(
                "Spotify Premium is required to download and use Spotify Soloist".into(),
            ));
        }

        let platform = get_platform()?;
        if platform.os == "android" {
            self.ensure_glibc_extracted(&platform.arch)?;
        }
        let bin_path = Self::soloist_bin_path();
        if Path::new(bin_path).exists() {
            return Ok(());
        }

        info!("Downloading Spotify Soloist binary");
        let target_url = get_soloist_download_url(&platform.os, &platform.arch)?;

        let bytes = block_on(async {
            let resp = reqwest::Client::new()
                .get(&target_url)
                .send()
                .await
                .map_err(|e| {
                    MoosyncError::String(format!("Failed to fetch soloist binary: {}", e))
                })?;
            if !resp.status().is_success() {
                return Err(MoosyncError::String(format!(
                    "Failed to fetch soloist binary: HTTP {}",
                    resp.status()
                )));
            }
            resp.bytes()
                .await
                .map_err(|e| MoosyncError::String(format!("Failed to read soloist bytes: {}", e)))
        })?;

        if let Some(parent) = Path::new(bin_path).parent() {
            fs::create_dir_all(parent).map_err(|e| {
                MoosyncError::String(format!("Failed to create bin directory: {}", e))
            })?;
        }

        let is_zip = target_url.ends_with(".zip");
        extract_soloist_binary(&bytes, is_zip, Path::new(bin_path))?;
        if platform.os == "android" {
            if let Ok(raw) = fs::read(bin_path) {
                if let Some(aligned) = align_elf_16k_if_needed(&raw) {
                    let _ = fs::write(bin_path, aligned);
                }
            }
        }

        info!("Spotify Soloist binary downloaded successfully");
        Ok(())
    }

    fn ensure_aaudio_sink_started(&self) -> MoosyncResult<()> {
        let mut sink_guard = self.aaudio_process.lock().unwrap();
        if let Some(proc) = sink_guard.as_ref() {
            if proc.get_status().map(|(running, _)| running).unwrap_or(false) {
                return Ok(());
            }
            *sink_guard = None;
        }
        let args = ["--socket", PW_ANDROID_SOCK_PATH];
        let env = HashMap::new();
        info!("Spawning AAudio sink for Spotify Soloist");
        let child = spawn_process(AAUDIO_SINK_BIN_PATH, &args, &env)?;
        *sink_guard = Some(child);
        Ok(())
    }

    fn build_soloist_env(api_key: Option<&str>) -> MoosyncResult<HashMap<String, String>> {
        let mut env = HashMap::new();
        if let Some(key) = api_key {
            env.insert("SPOTIFY_SOLOIST_API_KEY".to_string(), key.to_string());
        }
        if let Ok(platform) = get_platform() {
            if platform.os == "android" {
                env.insert("LD_LIBRARY_PATH".to_string(), SOLOIST_GLIBC_DIR.to_string());
                env.insert(
                    "LD_PRELOAD".to_string(),
                    "/cache/glibc/libpipewire-0.3.so.0".to_string(),
                );
                env.insert(
                    "GLIBC_TUNABLES".to_string(),
                    "glibc.pthread.rseq=0".to_string(),
                );
                env.insert(
                    "PW_ANDROID_SOCK_PATH".to_string(),
                    PW_ANDROID_SOCK_PATH.to_string(),
                );
                env.insert(
                    "SSL_CERT_DIR".to_string(),
                    "/system/etc/security/cacerts".to_string(),
                );
            }
        }
        Ok(env)
    }

    pub fn start_pairing_mode(&self, is_premium: bool, api_key: &str) -> MoosyncResult<()> {
        if !is_premium {
            return Err(MoosyncError::String(
                "Spotify Premium is required to pair Spotify Soloist".into(),
            ));
        }

        if self.is_paired() {
            self.set_status(SoloistStatus::Connected("Paired and ready".into()));
            return Ok(());
        }

        let mut pairing_guard = self.pairing_process.lock().unwrap();
        if let Some(proc) = pairing_guard.as_ref() {
            if proc.get_status().map(|(running, _)| running).unwrap_or(false) {
                self.set_status(SoloistStatus::Discovery(
                    "Advertising 'Moosync' via Spotify Connect".into(),
                ));
                return Ok(());
            }
            *pairing_guard = None;
        }

        if let Err(e) = self.ensure_binary_downloaded(is_premium) {
            self.set_status(SoloistStatus::Error(e.to_string()));
            return Err(e);
        }

        fs::create_dir_all(SOLOIST_DATA_DIR).map_err(|e| {
            MoosyncError::String(format!("Failed to create soloist data dir: {}", e))
        })?;

        let is_android = get_platform()
            .map(|p| p.os == "android")
            .unwrap_or(false);
        let mut args = vec![
            "--pair",
            "--device-name",
            SOLOIST_DEVICE_NAME,
            "--data-dir",
            SOLOIST_DATA_DIR,
        ];
        if is_android {
            args.push("--cache-dir");
            args.push(SOLOIST_CACHE_DIR);
        }
        if !api_key.is_empty() {
            args.push("--api-key");
            args.push(api_key);
        }

        let env = Self::build_soloist_env(None)?;
        info!("Spawning Spotify Soloist in pairing mode");
        let child = match spawn_process(Self::soloist_bin_path(), &args, &env) {
            Ok(c) => c,
            Err(e) => {
                self.set_status(SoloistStatus::Error(e.to_string()));
                return Err(e);
            }
        };

        *pairing_guard = Some(child);
        self.set_status(SoloistStatus::Discovery(
            "Advertising 'Moosync' via Spotify Connect".into(),
        ));
        Ok(())
    }

    pub fn poll_pairing_status(&self) -> MoosyncResult<bool> {
        if self.is_paired() {
            self.set_status(SoloistStatus::Connected("Paired and ready".into()));
            return Ok(true);
        }

        let mut pairing_guard = self.pairing_process.lock().unwrap();
        if let Some(proc) = pairing_guard.as_ref() {
            let (running, exit_code) = proc.get_status()?;
            if !running {
                if exit_code == Some(0) {
                    info!("Soloist pairing completed successfully");
                    let mut paired = self.is_paired.lock().unwrap();
                    *paired = true;
                    *pairing_guard = None;
                    self.set_status(SoloistStatus::Connected("Paired and ready".into()));
                    return Ok(true);
                }
                if exit_code == Some(EXPIRATION_EXIT_CODE) {
                    handle_expiration_cleanup();
                    *pairing_guard = None;
                    let msg = "Spotify Soloist build expired (exit code 10).".to_string();
                    self.set_status(SoloistStatus::Error(msg.clone()));
                    return Err(MoosyncError::String(msg));
                }
                warn!("Soloist pairing process exited with code: {:?}", exit_code);
                *pairing_guard = None;
                self.set_status(SoloistStatus::Error(format!(
                    "Pairing exited with code {:?}",
                    exit_code
                )));
                return Ok(false);
            }
            self.set_status(SoloistStatus::Discovery(
                "Advertising 'Moosync' via Spotify Connect".into(),
            ));
            return Ok(false);
        }

        Ok(false)
    }

    pub fn start_soloist(&self, is_premium: bool, api_key: &str) -> MoosyncResult<()> {
        if !is_premium {
            return Err(MoosyncError::String(
                "Spotify Premium is required for Spotify Soloist".into(),
            ));
        }

        if api_key.is_empty() {
            self.set_status(SoloistStatus::Inactive(
                "Soloist API Key not configured".into(),
            ));
            return Err(MoosyncError::String(
                "Soloist API Key is required for Spotify Soloist".into(),
            ));
        }

        self.ensure_binary_downloaded(is_premium)?;

        let mut proc_guard = self.process.lock().unwrap();
        if let Some(proc) = proc_guard.as_ref() {
            let (running, exit_code) = proc.get_status().unwrap_or((false, None));
            if running {
                return Ok(());
            }
            if let Some(code) = exit_code {
                if code == EXPIRATION_EXIT_CODE {
                    handle_expiration_cleanup();
                    return Err(MoosyncError::String(
                        "Spotify Soloist build expired (exit code 10). Deleted old binary; please restart.".into(),
                    ));
                }
            }
        }

        let port = self.get_port();
        fs::create_dir_all(SOLOIST_DATA_DIR).map_err(|e| {
            MoosyncError::String(format!("Failed to create soloist data dir: {}", e))
        })?;

        let is_android = get_platform()
            .map(|p| p.os == "android")
            .unwrap_or(false);
        let ws_arg = format!("127.0.0.1:{}", port);
        let args = vec![
            "--device-name",
            SOLOIST_DEVICE_NAME,
            "--ws",
            &ws_arg,
            "--data-dir",
            SOLOIST_DATA_DIR,
            "--cache-dir",
            SOLOIST_CACHE_DIR,
            "--api-key",
            api_key,
        ];

        if is_android {
            self.ensure_aaudio_sink_started()?;
        }

        let env = Self::build_soloist_env(Some(api_key))?;

        info!("Spawning Spotify Soloist on ws port {}", port);
        let child = spawn_process(Self::soloist_bin_path(), &args, &env)?;

        let (running, exit_code) = child.get_status().unwrap_or((true, None));
        if !running {
            if exit_code == Some(EXPIRATION_EXIT_CODE) {
                handle_expiration_cleanup();
                return Err(MoosyncError::String(
                    "Spotify Soloist build expired (exit code 10).".into(),
                ));
            }
            let err_msg = format!(
                "Spotify Soloist exited immediately (exit code: {:?})",
                exit_code
            );
            self.set_status(SoloistStatus::Error(err_msg.clone()));
            return Err(MoosyncError::String(err_msg));
        }

        let ctx = SoloistConnectContext {
            ws: Arc::clone(&self.ws),
            is_active: Arc::clone(&self.is_active),
            connected: Arc::new(AtomicBool::new(false)),
            status: Arc::clone(&self.status),
            is_paired: Arc::clone(&self.is_paired),
            port,
        };

        let stderr_ctx = ctx.clone();
        child.on_stderr(move |data| stderr_ctx.on_output(data));

        let stdout_ctx = ctx.clone();
        child.on_stdout(move |data| stdout_ctx.on_output(data));

        let exit_ctx = ctx.clone();
        child.on_exit(move |code| exit_ctx.on_exit(code));

        if connect_socket_once(&self.ws, &self.is_active, port).is_ok() {
            ctx.connected.store(true, Ordering::SeqCst);
            let mut paired = self.is_paired.lock().unwrap();
            *paired = true;
            self.set_status(SoloistStatus::Connected("Paired and ready".into()));
        } else {
            self.set_status(SoloistStatus::Connected("Waiting for Soloist...".into()));
        }
        *proc_guard = Some(child);
        Ok(())
    }

    pub fn connect_and_validate_socket(
        &self,
        port: u16,
        _child: Option<&ChildProcess>,
    ) -> MoosyncResult<()> {
        connect_socket_once(&self.ws, &self.is_active, port)
    }

    pub fn load_track(&self, src: &str, song: Option<&Song>) -> MoosyncResult<()> {
        let uri = resolve_spotify_uri(src, song);
        *self.pending_uri.lock().unwrap() = Some(uri);

        let _ = notify_player_event(NotifyPlayerEventRequest {
            event: Some(PlayerEvent {
                event: Some(player_event::Event::Loading(LoadingEvent {})),
            }),
        });

        Ok(())
    }

    pub fn play(&self) -> MoosyncResult<()> {
        if !self.is_active.load(Ordering::SeqCst) {
            self.send_command(&SoloistWsCommand::new("activate"))?;
            self.is_active.store(true, Ordering::SeqCst);
        }

        let uri = self.pending_uri.lock().unwrap().take();
        self.send_command(&SoloistWsCommand {
            uri: uri.as_deref(),
            ..SoloistWsCommand::new("play")
        })?;

        emit_player_event(player_event::Event::Play(true));

        Ok(())
    }

    pub fn pause(&self) -> MoosyncResult<()> {
        self.send_command(&SoloistWsCommand::new("pause"))?;

        let _ = notify_player_event(NotifyPlayerEventRequest {
            event: Some(PlayerEvent {
                event: Some(player_event::Event::Pause(true)),
            }),
        });

        Ok(())
    }

    pub fn stop(&self) -> MoosyncResult<()> {
        *self.pending_uri.lock().unwrap() = None;
        self.send_command(&SoloistWsCommand::new("pause"))?;

        let _ = notify_player_event(NotifyPlayerEventRequest {
            event: Some(PlayerEvent {
                event: Some(player_event::Event::Ended(true)),
            }),
        });

        Ok(())
    }

    pub fn seek(&self, position: Option<ProtoDuration>) -> MoosyncResult<()> {
        let ms = position
            .map(|d| (d.seconds as u64 * 1000) + (d.nanos as u64 / 1_000_000))
            .unwrap_or(0);

        self.send_command(&SoloistWsCommand {
            position_ms: Some(ms),
            ..SoloistWsCommand::new("seek")
        })?;

        let _ = notify_player_event(NotifyPlayerEventRequest {
            event: Some(PlayerEvent {
                event: Some(player_event::Event::TimeUpdate(duration_to_proto(
                    Duration::from_millis(ms),
                ))),
            }),
        });

        Ok(())
    }

    pub fn set_volume(&self, volume: u32) -> MoosyncResult<()> {
        self.send_command(&SoloistWsCommand {
            volume: Some(volume),
            ..SoloistWsCommand::new("set_volume")
        })?;
        Ok(())
    }

    fn send_command(&self, cmd: &SoloistWsCommand) -> MoosyncResult<()> {
        let ws_guard = self.ws.lock().unwrap();
        let client = ws_guard
            .as_ref()
            .ok_or_else(|| MoosyncError::String("Soloist WebSocket not connected".into()))?;

        let payload = serde_json::to_string(cmd)
            .map_err(|e| MoosyncError::String(format!("Serialization error: {}", e)))?;

        client.send_text(payload)?;
        Ok(())
    }
}

pub fn resolve_spotify_uri(src: &str, song: Option<&Song>) -> String {
    if src.starts_with("spotify:track:") {
        return src.to_string();
    }
    if let Some(inner) = song.and_then(|s| s.song.as_ref()) {
        if let Some(id) = inner.id.as_deref() {
            if let Some(raw_id) = id.strip_prefix("app.moosync.spotify:") {
                return format!("spotify:track:{}", raw_id);
            }
        }
    }
    src.to_string()
}

pub fn handle_expiration_cleanup() {
    warn!("Cleaning up expired Spotify Soloist binary");
    let _ = fs::remove_file(SOLOIST_BIN_PATH);
}

fn emit_player_event(event: player_event::Event) {
    let _ = notify_player_event(NotifyPlayerEventRequest {
        event: Some(PlayerEvent { event: Some(event) }),
    });
}

fn emit_position_update(pos: &SoloistPosition) {
    emit_player_event(player_event::Event::TimeUpdate(duration_to_proto(
        Duration::from_millis(pos.position_ms),
    )));
}

fn is_valid_device_name(device_name: Option<&str>) -> bool {
    match device_name {
        Some(name) if name != SOLOIST_DEVICE_NAME => {
            warn!(
                "Soloist WebSocket device mismatch: expected '{}', got '{}'",
                SOLOIST_DEVICE_NAME, name
            );
            false
        }
        _ => true,
    }
}

fn apply_active_state(active: Option<bool>, is_active: &Arc<AtomicBool>) -> bool {
    match active {
        Some(false) => {
            is_active.store(false, Ordering::SeqCst);
            emit_player_event(player_event::Event::Pause(true));
            false
        }
        Some(true) => {
            is_active.store(true, Ordering::SeqCst);
            true
        }
        None => is_active.load(Ordering::SeqCst),
    }
}

fn apply_playback_status(status: Option<&str>, is_active: bool) {
    match status {
        Some("paused") => emit_player_event(player_event::Event::Pause(true)),
        Some("buffering") => emit_player_event(player_event::Event::Loading(LoadingEvent {})),
        Some("playing") if is_active => emit_player_event(player_event::Event::Play(true)),
        _ => {}
    }
}

fn handle_ws_message(
    text: &str,
    client: &WebSocketClient,
    status_cmd: &str,
    is_active: &Arc<AtomicBool>,
) {
    let Ok(resp) = serde_json::from_str::<SoloistWsResponse>(text) else {
        return;
    };

    match resp {
        SoloistWsResponse::Event(SoloistWsEvent::AuthState {
            logged_in,
            device_name,
            is_active: active,
        }) => {
            if !is_valid_device_name(device_name.as_deref()) {
                return;
            }
            if !logged_in {
                let _ = client.send_text(status_cmd);
                return;
            }
            apply_active_state(active, is_active);
        }
        SoloistWsResponse::Event(SoloistWsEvent::PlaybackState {
            device_name,
            is_active: active,
            logged_in,
            status,
            position,
            ..
        }) => {
            if !is_valid_device_name(device_name.as_deref()) {
                return;
            }
            if logged_in == Some(false) {
                let _ = client.send_text(status_cmd);
                return;
            }
            let currently_active = apply_active_state(active, is_active);
            apply_playback_status(status.as_deref(), currently_active);
            if let Some(pos) = position.as_ref() {
                emit_position_update(pos);
            }
        }
        SoloistWsResponse::Event(SoloistWsEvent::PlaybackChanged {
            status,
            is_active: active,
            position,
            ..
        }) => {
            let currently_active = apply_active_state(active, is_active);
            apply_playback_status(status.as_deref(), currently_active);
            if let Some(pos) = position.as_ref() {
                emit_position_update(pos);
            }
        }
        SoloistWsResponse::Event(SoloistWsEvent::DeviceChanged {
            device_name,
            is_active: active,
        }) => {
            if !is_valid_device_name(device_name.as_deref()) {
                return;
            }
            apply_active_state(active, is_active);
        }
        SoloistWsResponse::Event(SoloistWsEvent::PositionSync { position }) => {
            emit_position_update(&position);
        }
        SoloistWsResponse::Event(SoloistWsEvent::TrackChanged { position, .. }) => {
            if let Some(pos) = position.as_ref() {
                emit_position_update(pos);
            }
        }
        SoloistWsResponse::Event(
            SoloistWsEvent::ContextChanged { .. }
            | SoloistWsEvent::OptionsChanged { .. }
            | SoloistWsEvent::QueueChanged {}
            | SoloistWsEvent::VolumeChanged { .. }
            | SoloistWsEvent::Unknown,
        ) => {}
        SoloistWsResponse::Snapshot(snap) => {
            if !is_valid_device_name(snap.device_name.as_deref()) {
                return;
            }
            if snap.logged_in == Some(false) {
                let _ = client.send_text(status_cmd);
                return;
            }
            let currently_active = apply_active_state(snap.is_active, is_active);
            apply_playback_status(snap.status.as_deref(), currently_active);
            if let Some(pos) = snap.position.as_ref() {
                emit_position_update(pos);
            }
        }
    }
}

fn connect_socket_once(
    ws_slot: &Arc<Mutex<Option<WebSocketClient>>>,
    is_active: &Arc<AtomicBool>,
    port: u16,
) -> MoosyncResult<()> {
    let ws_url = format!("ws://127.0.0.1:{}", port);
    info!("Connecting to Soloist WebSocket at {}", ws_url);

    let status_cmd = serde_json::to_string(&SoloistWsCommand::new("get_state"))
        .map_err(|e| MoosyncError::String(e.to_string()))?;

    let client = WebSocketClient::connect(&ws_url)?;
    let status_cmd_clone = status_cmd.clone();
    let client_for_msg = client.clone();
    let is_active_for_msg = Arc::clone(is_active);
    client.on_message(move |msg| {
        if let WebSocketMessage::Text(text) = msg {
            info!("Received msg from Soloist WebSocket: {}", text);
            handle_ws_message(
                &text,
                &client_for_msg,
                &status_cmd_clone,
                &is_active_for_msg,
            );
        }
    });
    client.on_close(|event| {
        warn!("Soloist WebSocket closed: {:?}", event);
    });
    client.send_text(&status_cmd)?;

    *ws_slot.lock().unwrap() = Some(client);
    Ok(())
}
