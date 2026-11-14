use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use futures::executor::block_on;
use moosync_edk::{
    api::{duration_to_proto, get_platform, notify_player_event, spawn_process},
    info,
    types::{
        player_event, ChildProcess, Duration as ProtoDuration, NotifyPlayerEventRequest,
        PlayerEvent, Song, WebSocketClient, WebSocketMessage,
    },
    warn, MoosyncError, MoosyncResult,
};

mod client;
mod downloader;

pub use client::{SoloistWsCommand, SoloistWsResponse};
pub use downloader::{extract_soloist_binary, get_soloist_download_url};

pub const SOLOIST_BIN_PATH: &str = "/cache/bin/soloist";
pub const SOLOIST_DATA_DIR: &str = "/cache/soloist_data";
pub const DEFAULT_WS_PORT: u16 = 9090;
pub const SOLOIST_DEVICE_NAME: &str = "Moosync";
pub const EXPIRATION_EXIT_CODE: i32 = 10;

pub struct SoloistPlayer {
    pairing_process: Mutex<Option<ChildProcess>>,
    process: Mutex<Option<ChildProcess>>,
    ws: Mutex<Option<WebSocketClient>>,
    port: Mutex<u16>,
    is_paired: Mutex<bool>,
}

impl SoloistPlayer {
    pub fn new() -> Self {
        let is_paired = Self::has_stored_session();
        Self {
            pairing_process: Mutex::new(None),
            process: Mutex::new(None),
            ws: Mutex::new(None),
            port: Mutex::new(DEFAULT_WS_PORT),
            is_paired: Mutex::new(is_paired),
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
        let path = Path::new(SOLOIST_DATA_DIR);
        if !path.exists() {
            return false;
        }
        let entries = match fs::read_dir(path) {
            Ok(e) => e,
            Err(_) => return false,
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.ends_with(".pid")
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
            reqwest::Client::new()
                .get(&target_url)
                .send()
                .await
                .map_err(|e| MoosyncError::String(format!("Failed to fetch soloist binary: {}", e)))?
                .bytes()
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
            return Ok(());
        }

        let mut pairing_guard = self.pairing_process.lock().unwrap();
        if pairing_guard.is_some() {
            return Ok(());
        }

        self.ensure_binary_downloaded(is_premium)?;

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
        let child = spawn_process(SOLOIST_BIN_PATH, &args, &env)?;

        *pairing_guard = Some(child);
        Ok(())
    }

    pub fn poll_pairing_status(&self) -> MoosyncResult<bool> {
        if self.is_paired() {
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
                    return Ok(true);
                }
                if exit_code == Some(EXPIRATION_EXIT_CODE) {
                    handle_expiration_cleanup();
                    *pairing_guard = None;
                    return Err(MoosyncError::String(
                        "Spotify Soloist build expired (exit code 10).".into(),
                    ));
                }
                warn!("Soloist pairing process exited with code: {:?}", exit_code);
                *pairing_guard = None;
                return Ok(false);
            }
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
        ];

        let mut env = HashMap::new();
        if !api_key.is_empty() {
            env.insert("SPOTIFY_SOLOIST_API_KEY".to_string(), api_key.to_string());
        }

        info!("Spawning Spotify Soloist on ws port {}", port);
        let child = spawn_process(SOLOIST_BIN_PATH, &args, &env)?;

        let (running, exit_code) = child.get_status().unwrap_or((true, None));
        if !running && exit_code == Some(EXPIRATION_EXIT_CODE) {
            handle_expiration_cleanup();
            return Err(MoosyncError::String(
                "Spotify Soloist build expired (exit code 10).".into(),
            ));
        }

        *proc_guard = Some(child);
        self.connect_and_validate_socket(port)?;
        let mut paired = self.is_paired.lock().unwrap();
        *paired = true;
        Ok(())
    }

    pub fn connect_and_validate_socket(&self, port: u16) -> MoosyncResult<()> {
        let ws_url = format!("ws://127.0.0.1:{}", port);
        info!("Connecting to Soloist WebSocket at {}", ws_url);

        let status_cmd = serde_json::to_string(&SoloistWsCommand {
            command: "status",
            uri: None,
            position_ms: None,
            volume: None,
        })
        .map_err(|e| MoosyncError::String(e.to_string()))?;

        let mut last_err = None;
        for attempt in 1..=25 {
            std::thread::sleep(Duration::from_millis(100));

            let client = match WebSocketClient::connect(&ws_url) {
                Ok(c) => c,
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            };

            if let Err(e) = client.send_text(&status_cmd) {
                last_err = Some(e);
                continue;
            }

            let msg = match client.receive(Some(2000)) {
                Ok(m) => m,
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            };

            info!("Received msg from Soloist WebSocket: {:?}", msg);
            let is_valid = match &msg {
                Some(WebSocketMessage::Text(text)) => serde_json::from_str::<SoloistWsResponse>(text)
                    .ok()
                    .and_then(|r| r.device_name)
                    .map(|d| d == SOLOIST_DEVICE_NAME)
                    .unwrap_or(false),
                _ => false,
            };

            if is_valid {
                info!(
                    "Soloist WebSocket validated for device '{}' (attempt {})",
                    SOLOIST_DEVICE_NAME, attempt
                );
                let mut ws_guard = self.ws.lock().unwrap();
                *ws_guard = Some(client);
                return Ok(());
            }

            last_err = Some(MoosyncError::String(format!(
                "Socket validation failed: expected device '{}', got: {:?}",
                SOLOIST_DEVICE_NAME, msg
            )));
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
        self.send_command(&SoloistWsCommand {
            command: "load",
            uri: Some(&uri),
            position_ms: None,
            volume: None,
        })?;

        let _ = notify_player_event(NotifyPlayerEventRequest {
            event: Some(PlayerEvent {
                event: Some(player_event::Event::Loading(true)),
            }),
        });

        Ok(())
    }

    pub fn play(&self) -> MoosyncResult<()> {
        self.send_command(&SoloistWsCommand {
            command: "play",
            uri: None,
            position_ms: None,
            volume: None,
        })?;

        let _ = notify_player_event(NotifyPlayerEventRequest {
            event: Some(PlayerEvent {
                event: Some(player_event::Event::Play(true)),
            }),
        });

        Ok(())
    }

    pub fn pause(&self) -> MoosyncResult<()> {
        self.send_command(&SoloistWsCommand {
            command: "pause",
            uri: None,
            position_ms: None,
            volume: None,
        })?;

        let _ = notify_player_event(NotifyPlayerEventRequest {
            event: Some(PlayerEvent {
                event: Some(player_event::Event::Pause(true)),
            }),
        });

        Ok(())
    }

    pub fn stop(&self) -> MoosyncResult<()> {
        self.send_command(&SoloistWsCommand {
            command: "stop",
            uri: None,
            position_ms: None,
            volume: None,
        })?;

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
            command: "seek",
            uri: None,
            position_ms: Some(ms),
            volume: None,
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
            command: "volume",
            uri: None,
            position_ms: None,
            volume: Some(volume),
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
