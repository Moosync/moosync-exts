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
pub use downloader::{extract_soloist_binary, get_soloist_download_url};

pub const SOLOIST_BIN_PATH: &str = "/cache/bin/soloist";
pub const SOLOIST_DATA_DIR: &str = "/cache/soloist_data";
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

pub struct SoloistPlayer {
    pairing_process: Mutex<Option<ChildProcess>>,
    process: Mutex<Option<ChildProcess>>,
    ws: Mutex<Option<WebSocketClient>>,
    port: Mutex<u16>,
    is_paired: Mutex<bool>,
    is_active: Arc<AtomicBool>,
    status: Mutex<SoloistStatus>,
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
            ws: Mutex::new(None),
            port: Mutex::new(DEFAULT_WS_PORT),
            is_paired: Mutex::new(is_paired),
            is_active: Arc::new(AtomicBool::new(false)),
            status: Mutex::new(initial_status),
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
        if let Ok(user_entries) = fs::read_dir(&users_dir) {
            for entry in user_entries.flatten() {
                if !entry.file_name().to_string_lossy().starts_with('.') {
                    return true;
                }
            }
        }

        let path = Path::new(SOLOIST_DATA_DIR);
        if !path.exists() {
            return false;
        }
        let entries = match fs::read_dir(path) {
            Ok(e) => e,
            Err(_) => return false,
        };
        for entry in entries.flatten() {
            if !entry.path().is_file() {
                continue;
            }
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with('.')
                && !name_str.ends_with(".pid")
                && !name_str.ends_with(".addr")
                && !name_str.ends_with(".port")
            {
                return true;
            }
        }
        false
    }

    pub fn set_port(&self, port: u16) {
        let mut p = self.port.lock().unwrap();
        *p = port;
    }

    pub fn get_port(&self) -> u16 {
        *self.port.lock().unwrap()
    }

    pub fn ensure_binary_downloaded(&self, is_premium: bool) -> MoosyncResult<()> {
        if !is_premium {
            return Err(MoosyncError::String(
                "Spotify Premium is required to download and use Spotify Soloist".into(),
            ));
        }

        if Path::new(SOLOIST_BIN_PATH).exists() {
            return Ok(());
        }

        info!("Downloading Spotify Soloist binary");
        let platform = get_platform()?;
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

        if let Some(parent) = Path::new(SOLOIST_BIN_PATH).parent() {
            fs::create_dir_all(parent).map_err(|e| {
                MoosyncError::String(format!("Failed to create bin directory: {}", e))
            })?;
        }

        let is_zip = target_url.ends_with(".zip");
        extract_soloist_binary(&bytes, is_zip, Path::new(SOLOIST_BIN_PATH))?;

        info!("Spotify Soloist binary downloaded successfully");
        Ok(())
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

        let mut args = vec![
            "--pair",
            "--device-name",
            SOLOIST_DEVICE_NAME,
            "--data-dir",
            SOLOIST_DATA_DIR,
        ];
        if !api_key.is_empty() {
            args.push("--api-key");
            args.push(api_key);
        }

        let env = HashMap::new();
        info!("Spawning Spotify Soloist in pairing mode");
        let child = match spawn_process(SOLOIST_BIN_PATH, &args, &env) {
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

        let ws_arg = format!("127.0.0.1:{}", port);
        let args = vec![
            "--device-name",
            SOLOIST_DEVICE_NAME,
            "--ws",
            &ws_arg,
            "--data-dir",
            SOLOIST_DATA_DIR,
            "--api-key",
            api_key,
        ];

        let mut env = HashMap::new();
        env.insert("SPOTIFY_SOLOIST_API_KEY".to_string(), api_key.to_string());

        info!("Spawning Spotify Soloist on ws port {}", port);
        let child = spawn_process(SOLOIST_BIN_PATH, &args, &env)?;

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

        if let Err(e) = self.connect_and_validate_socket(port, Some(&child)) {
            self.set_status(SoloistStatus::Error(e.to_string()));
            return Err(e);
        }
        *proc_guard = Some(child);
        let mut paired = self.is_paired.lock().unwrap();
        *paired = true;
        self.set_status(SoloistStatus::Connected("Paired and ready".into()));
        Ok(())
    }

    pub fn connect_and_validate_socket(
        &self,
        port: u16,
        child: Option<&ChildProcess>,
    ) -> MoosyncResult<()> {
        let ws_url = format!("ws://127.0.0.1:{}", port);
        info!("Connecting to Soloist WebSocket at {}", ws_url);

        let status_cmd = serde_json::to_string(&SoloistWsCommand::new("get_state"))
            .map_err(|e| MoosyncError::String(e.to_string()))?;

        let mut last_err = None;
        for _attempt in 1..=25 {
            if let Some(proc) = child {
                if let Ok((false, exit_code)) = proc.get_status() {
                    return Err(MoosyncError::String(format!(
                        "Spotify Soloist exited before WebSocket was ready (exit code: {:?})",
                        exit_code
                    )));
                }
            }
            std::thread::sleep(Duration::from_millis(100));

            let client = match WebSocketClient::connect(&ws_url) {
                Ok(c) => c,
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            };

            let status_cmd_clone = status_cmd.clone();
            let client_for_msg = client.clone();
            let is_active_for_msg = Arc::clone(&self.is_active);
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

            if let Err(e) = client.send_text(&status_cmd) {
                last_err = Some(e);
                continue;
            }

            let mut ws_guard = self.ws.lock().unwrap();
            *ws_guard = Some(client);
            return Ok(());
        }

        Err(last_err.unwrap_or_else(|| {
            MoosyncError::String(format!(
                "Socket validation timed out for device '{}'",
                SOLOIST_DEVICE_NAME
            ))
        }))
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
            if let Some(raw_id) = id.strip_prefix("moosync.spotify:") {
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
