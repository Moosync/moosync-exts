use std::{path::Path, sync::Mutex};

use bytes::BytesMut;
use moosync_edk::{
    api::{
        self, get_preference, host_get_cache, host_set_cache, register_user_preference, Accounts,
        ContextMenu, DatabaseEvents, Extension, PlayerEvents, PlayerProvider, PreferenceEvents,
        Provider,
    },
    config,
    handler::register_extension,
    http, info,
    socket::SocketClient,
    types::{
        GetCacheRequest, GetCurrentSongRequest, GetPlayerStateRequest, GetPreferenceRequest,
        GetTimeRequest, PlayerState, PlayerStateChangedRequest, PreferenceData, PreferenceItem,
        PreferenceType, RegisterUserPreferenceRequest, SeekedRequest, SetCacheRequest, Song,
        SongChangedRequest, SongType, value::Kind as PreferenceValueKind,
    },
    MoosyncError, MoosyncResult,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

const CLIENT_ID: &str = "867757838679670784";

#[repr(i32)]
#[derive(Debug)]
enum Operation {
    HANDSHAKE = 0,
    FRAME,
    CLOSE,
    PING,
    PONG,
}

impl Operation {
    pub fn from_u32(a: u32) -> Self {
        match a {
            0 => Self::HANDSHAKE,
            1 => Self::FRAME,
            2 => Self::CLOSE,
            3 => Self::PING,
            4 => Self::PONG,
            _ => Self::CLOSE,
        }
    }
}

#[derive(Debug)]
struct DecodedData<T> {
    op: Operation,
    data: T,
}

struct Sock {
    client: SocketClient,
}

impl Sock {
    pub fn new(client: SocketClient) -> Self {
        Self { client }
    }
}

impl Sock {
    fn encode<T>(op: Operation, data: T) -> MoosyncResult<BytesMut>
    where
        T: Serialize,
    {
        let data_string =
            serde_json::to_string(&data).map_err(|e| MoosyncError::String(e.to_string()))?;

        let len = data_string.len();
        let mut packet = BytesMut::with_capacity(8 + len);

        packet.extend_from_slice(&(op as i32).to_le_bytes());
        packet.extend_from_slice(&(len as i32).to_le_bytes());
        packet.extend_from_slice(data_string.as_bytes());
        info!("encoded and writing {}", data_string);

        Ok(packet)
    }

    pub fn read_and_decode<T>(&self) -> MoosyncResult<DecodedData<T>>
    where
        T: DeserializeOwned,
    {
        info!("Reading from sock {}", self.client.sock_id);
        let data = self.client.read(8)?;
        if data.len() < 8 {
            return Err("Packet not big enough to be decoded".into());
        }

        let op = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let op = Operation::from_u32(op);

        let len = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;

        let buf = self.client.read(len as u64)?;
        info!("Read buf {:?}", str::from_utf8(&buf));
        let data: T =
            serde_json::from_slice(&buf).map_err(|e| MoosyncError::String(e.to_string()))?;
        Ok(DecodedData { op, data })
    }

    fn write<T>(&self, op: Operation, data: T) -> MoosyncResult<()>
    where
        T: Serialize,
    {
        let encoded = Self::encode(op, data)?;
        info!("Encoded");
        let _res = self.client.write(&encoded)?;
        info!("Wrote to sock");
        Ok(())
    }

    fn uuid4122() -> String {
        // Return a fixed UUID for testing purposes to allow exact matching of writes.
        "00000000-0000-0000-0000-000000000000".to_string()
    }

    fn request<T>(&self, cmd: &str, data: T) -> MoosyncResult<()>
    where
        T: Serialize,
    {
        #[derive(Serialize)]
        struct Request<T>
        where
            T: Serialize,
        {
            cmd: String,
            args: T,
            nonce: String,
        }

        let nonce = Self::uuid4122();
        self.write(
            Operation::FRAME,
            Request {
                cmd: cmd.to_string(),
                args: data,
                nonce,
            },
        )?;

        let resp = self.read_and_decode::<Value>()?;
        info!("Got response {:?}", resp);

        Ok(())
    }
}

struct DiscordRPC {
    sock: Mutex<Option<Sock>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct HandshakeRequest {
    pub v: u8,
    pub client_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct HandshakeResponse {
    pub cmd: String,
    pub data: Value,
    pub evt: Option<String>,
    pub nonce: Option<String>,
}

#[derive(Serialize)]
struct ActivityButton {
    label: String,
    url: String,
}

#[derive(Serialize)]
struct Timestamps {
    start: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<u64>,
}

#[derive(Serialize)]
struct Assets {
    large_image: String,
    large_text: String,
    small_image: String,
    small_text: String,
}

#[derive(Serialize, Default)]
struct Party {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<Vec<u32>>,
}

#[derive(Serialize)]
struct Secrets {
    #[serde(rename = "match")]
    match_: String,
    join: String,
    spectate: String,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Activity {
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamps: Option<Timestamps>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assets: Option<Assets>,
    #[serde(skip_serializing_if = "Option::is_none")]
    party: Option<Party>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secrets: Option<Secrets>,
    #[serde(skip_serializing_if = "Option::is_none")]
    buttons: Option<Vec<ActivityButton>>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    activity_type: Option<u8>,
    #[serde(rename = "status_display_type", skip_serializing_if = "Option::is_none")]
    status_display_type: Option<u8>,
    instance: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetActivityRequest {
    activity: Activity,
    pid: u64,
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(triple >> 18) & 0x3F] as char);
        out.push(TABLE[(triple >> 12) & 0x3F] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(triple >> 6) & 0x3F] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[triple & 0x3F] as char
        } else {
            '='
        });
    }
    out
}

fn read_local_thumbnail(song: &Song) -> Option<(String, Vec<u8>)> {
    let song_inner = song.song.as_ref()?;
    let raw_path = song_inner
        .song_cover_path_high
        .as_deref()
        .or(song_inner.song_cover_path_low.as_deref())
        .or_else(|| {
            song.album.as_ref().and_then(|a| {
                a.album_coverpath_high
                    .as_deref()
                    .or(a.album_coverpath_low.as_deref())
            })
        })?;
    if raw_path.is_empty() || raw_path.starts_with("http://") || raw_path.starts_with("https://") {
        return None;
    }

    let filename = Path::new(raw_path).file_name()?.to_string_lossy().to_string();
    let mapped_path = format!("/thumbnails/{filename}");
    let bytes = std::fs::read(&mapped_path)
        .or_else(|_| std::fs::read(raw_path))
        .ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some((filename, bytes))
}

fn get_imgbb_api_key() -> String {
    get_preference(GetPreferenceRequest {
        data: Some(PreferenceData {
            key: "imgbb_api_key".into(),
            ..Default::default()
        }),
    })
    .ok()
    .and_then(|res| res.data)
    .and_then(|data| data.value)
    .and_then(|val| val.kind)
    .and_then(|v| match v {
        PreferenceValueKind::StringValue(s) => Some(s.trim().to_string()),
        _ => None,
    })
    .unwrap_or_default()
}

fn upload_to_imgbb(api_key: &str, bytes: &[u8]) -> Option<String> {
    let url = format!(
        "https://api.imgbb.com/1/upload?key={}&expiration=86400",
        encode_query_component(api_key)
    );
    let body = format!("image={}", encode_query_component(&base64_encode(bytes)));
    let req = http::HttpRequest::post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body.into_bytes());
    let resp = http::request(&req).ok()?;
    if resp.status_code < 200 || resp.status_code >= 300 {
        return None;
    }
    let parsed: Value = serde_json::from_slice(&resp.body).ok()?;
    parsed
        .get("data")?
        .get("url")?
        .as_str()
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
        .map(|u| u.to_string())
}

fn upload_to_litterbox(filename: &str, bytes: &[u8]) -> Option<String> {
    let boundary = "----MoosyncDiscordCoverBoundary";
    let mut body = Vec::with_capacity(bytes.len() + 256);
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"reqtype\"\r\n\r\nfileupload\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"time\"\r\n\r\n24h\r\n\
             --{boundary}\r\nContent-Disposition: form-data; name=\"fileToUpload\"; filename=\"{filename}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let req = http::HttpRequest::post("https://litterbox.catbox.moe/resources/internals/api.php")
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body);
    let resp = http::request(&req).ok()?;
    if resp.status_code < 200 || resp.status_code >= 300 {
        return None;
    }
    let url = String::from_utf8(resp.body).ok()?.trim().to_string();
    if url.starts_with("http://") || url.starts_with("https://") {
        Some(url)
    } else {
        None
    }
}

fn upload_local_cover_art(song: &Song) -> Option<String> {
    let (filename, bytes) = read_local_thumbnail(song)?;
    let cache_key = format!("discord_upload:{filename}");
    if let Ok(resp) = host_get_cache(GetCacheRequest {
        key: cache_key.clone(),
    }) {
        if let Some(cached_bytes) = resp.data {
            if let Ok(url) = String::from_utf8(cached_bytes) {
                if url.starts_with("http://") || url.starts_with("https://") {
                    return Some(url);
                }
            }
        }
    }

    let api_key = get_imgbb_api_key();
    let uploaded_url = if !api_key.is_empty() {
        upload_to_imgbb(&api_key, &bytes).or_else(|| upload_to_litterbox(&filename, &bytes))
    } else {
        upload_to_litterbox(&filename, &bytes)
    }?;

    let _ = host_set_cache(SetCacheRequest {
        key: cache_key,
        data: uploaded_url.as_bytes().to_vec(),
        ttl_seconds: Some(82800),
    });

    Some(uploaded_url)
}

fn encode_query_component(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char);
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded
}

fn fetch_local_cover_art(song: &Song) -> Option<String> {
    let title = song
        .song
        .as_ref()
        .and_then(|s| s.title.as_deref())
        .map(str::trim)
        .unwrap_or_default();
    let artist = song
        .artists
        .first()
        .and_then(|a| a.artist_name.as_deref())
        .map(str::trim)
        .unwrap_or_default();
    let album = song
        .album
        .as_ref()
        .and_then(|a| a.album_name.as_deref())
        .map(str::trim)
        .unwrap_or_default();

    let query = match (artist.is_empty(), album.is_empty(), title.is_empty()) {
        (false, false, _) => format!("{artist} {album}"),
        (false, true, false) => format!("{artist} {title}"),
        (true, false, _) => album.to_string(),
        (true, true, false) => title.to_string(),
        (false, true, true) => artist.to_string(),
        (true, true, true) => return None,
    };

    let cache_key = format!("discord_cover:{}", query.to_lowercase());
    if let Ok(resp) = host_get_cache(GetCacheRequest {
        key: cache_key.clone(),
    }) {
        if let Some(bytes) = resp.data {
            if let Ok(url) = String::from_utf8(bytes) {
                if !url.is_empty() {
                    return Some(url);
                }
            }
        }
    }

    let url = format!(
        "https://api.deezer.com/search?q={}&limit=1",
        encode_query_component(&query)
    );
    let resp = http::get(&url, None).ok()?;
    if resp.status_code < 200 || resp.status_code >= 300 {
        return None;
    }

    let parsed: Value = serde_json::from_slice(&resp.body).ok()?;
    let first = parsed.get("data")?.as_array()?.first()?;
    let album_obj = first.get("album")?;
    let cover_url = album_obj
        .get("cover_big")
        .or_else(|| album_obj.get("cover_medium"))
        .or_else(|| album_obj.get("cover"))
        .and_then(|v| v.as_str())
        .filter(|u| u.starts_with("http://") || u.starts_with("https://"))?
        .to_string();

    let _ = host_set_cache(SetCacheRequest {
        key: cache_key,
        data: cover_url.as_bytes().to_vec(),
        ttl_seconds: Some(86400),
    });

    Some(cover_url)
}

impl DiscordRPC {
    pub fn new() -> Self {
        Self {
            sock: Mutex::new(None),
        }
    }

    fn open_sock(&self) -> MoosyncResult<()> {
        let mut sock = self.sock.lock().unwrap();

        let client = SocketClient::connect("/discord-ipc-0")?;

        info!("Connected to sock {}", client.sock_id);
        *sock = Some(Sock::new(client));

        Ok(())
    }

    pub fn connect_rpc(&self) -> MoosyncResult<()> {
        {
            let sock = self.sock.lock().unwrap();
            if sock.is_some() {
                return Ok(());
            }
        }

        self.open_sock()?;

        let mut sock_mutex = self.sock.lock().unwrap();
        if let Some(sock) = sock_mutex.as_ref() {
            let handshake_res = sock
                .write(
                    Operation::HANDSHAKE,
                    HandshakeRequest {
                        v: 1,
                        client_id: CLIENT_ID.to_string(),
                    },
                )
                .and_then(|_| sock.read_and_decode::<HandshakeResponse>());

            match handshake_res {
                Ok(data) => {
                    info!("Got res {:?}", data.data);
                    if let Operation::CLOSE = data.op {
                        *sock_mutex = None;
                    }
                }
                Err(e) => {
                    *sock_mutex = None;
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    fn parse_song(&self, song: Song, status: PlayerState, start_time: u64) -> Activity {
        let artists = song.artists.clone();
        let song_inner = song.song.clone().unwrap_or_default();

        let truncate_128 = |s: String| -> String {
            if s.chars().count() <= 128 {
                s
            } else {
                let mut out: String = s.chars().take(125).collect();
                out.push_str("...");
                out
            }
        };

        let state = truncate_128(format!(
            "{} - {}",
            artists
                .iter()
                .filter_map(|a| a.artist_name.clone())
                .collect::<Vec<String>>()
                .join(", "),
            song.album
                .as_ref()
                .and_then(|a| a.album_name.as_deref())
                .unwrap_or("Misc")
        ));
        let details = truncate_128(format!(
            "{} {}",
            song_inner.title.clone().unwrap_or_default(),
            if let PlayerState::Paused = status {
                "(Paused)"
            } else {
                ""
            }
        ));

        let start_time = api::system_time(moosync_edk::types::SystemTimeRequest {})
            .map(|r| r.timestamp)
            .unwrap_or(0)
            .saturating_sub(start_time);

        let mut buttons = vec![];
        if let Some(ref playback_url) = song_inner.playback_url {
            if song_inner.r#type == SongType::Url as i32
                && song_inner
                    .id
                    .as_ref()
                    .map_or(false, |id| id.contains("youtube"))
            {
                buttons.push(ActivityButton {
                    label: "Show on YouTube".into(),
                    url: format!("https://www.youtube.com/watch?v={}", playback_url.url),
                });
            }
        }

        if song_inner.url.is_some() {
            if song_inner.r#type == SongType::Spotify as i32 {
                let raw_url = song_inner.url.as_ref().unwrap();
                let track_id = raw_url.strip_prefix("spotify:track:").unwrap_or(raw_url);
                buttons.push(ActivityButton {
                    label: "Show on Spotify".into(),
                    url: format!("https://open.spotify.com/track/{track_id}"),
                });
            }
        }

        let large_image = song_inner
            .song_cover_path_high
            .as_deref()
            .or(song_inner.song_cover_path_low.as_deref())
            .or_else(|| {
                song.album.as_ref().and_then(|a| {
                    a.album_coverpath_high
                        .as_deref()
                        .or(a.album_coverpath_low.as_deref())
                })
            })
            .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
            .map(|u| u.to_string())
            .or_else(|| {
                if song_inner.r#type == SongType::Local as i32 {
                    upload_local_cover_art(&song).or_else(|| fetch_local_cover_art(&song))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "logo_border".to_string());

        let large_text = song
            .album
            .as_ref()
            .and_then(|a| a.album_name.as_deref())
            .filter(|s| !s.is_empty())
            .unwrap_or("Moosync")
            .to_string();

        Activity {
            state: Some(state),
            details: Some(details),
            timestamps: Some(Timestamps {
                start: start_time,
                end: None,
            }),
            assets: Some(Assets {
                large_image,
                large_text,
                small_image: "logo_circle".into(),
                small_text: "Moosync".into(),
            }),
            party: None,
            secrets: None,
            buttons: if !buttons.is_empty() {
                Some(buttons)
            } else {
                None
            },
            activity_type: Some(2),
            status_display_type: Some(2),
            instance: true,
        }
    }

    pub fn set_activity(
        &self,
        song: Option<Song>,
        state: PlayerState,
        start_time: u64,
    ) -> MoosyncResult<()> {
        self.connect_rpc()?;

        let mut sock_mutex = self.sock.lock().unwrap();
        if let Some(sock) = sock_mutex.as_ref() {
            let pid = config::get("pid")?
                .and_then(|p| p.parse::<u64>().ok())
                .unwrap_or(0);

            let res = if let Some(song) = song {
                let activity = self.parse_song(song, state, start_time);
                sock.request("SET_ACTIVITY", SetActivityRequest { pid, activity })
            } else {
                sock.request(
                    "SET_ACTIVITY",
                    SetActivityRequest {
                        pid,
                        activity: Activity::default(),
                    },
                )
            };
            if let Err(e) = res {
                *sock_mutex = None;
                return Err(e);
            }
        }

        Ok(())
    }

    fn update_presence(&self) -> MoosyncResult<()> {
        let current_song = api::get_current_song(GetCurrentSongRequest {})?.song;
        let player_state = PlayerState::try_from(
            api::get_player_state(GetPlayerStateRequest {})?.state,
        )
        .unwrap_or(PlayerState::Stopped);
        let time = api::get_time(GetTimeRequest {})?.time as u64;
        self.set_activity(current_song, player_state, time)?;
        Ok(())
    }
}

impl PlayerEvents for DiscordRPC {
    fn song_changed(&self, _: SongChangedRequest) -> MoosyncResult<()> {
        self.update_presence()
    }

    fn seeked(&self, _: SeekedRequest) -> MoosyncResult<()> {
        self.update_presence()
    }

    fn player_state_changed(&self, _: PlayerStateChangedRequest) -> MoosyncResult<()> {
        self.update_presence()
    }
}

impl Provider for DiscordRPC {}
impl PlayerProvider for DiscordRPC {}
impl DatabaseEvents for DiscordRPC {}
impl PreferenceEvents for DiscordRPC {}
impl Extension for DiscordRPC {}
impl ContextMenu for DiscordRPC {}
impl Accounts for DiscordRPC {}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    info!("Initializing discord rpc");

    let extension = DiscordRPC::new();
    if let Err(e) = extension.set_activity(Some(Song::default()), PlayerState::Paused, 0) {
        info!("error connecting to rpc {:?}", e);
    }

    register_extension(Box::new(extension)).unwrap();
    let _ = register_user_preference(RegisterUserPreferenceRequest {
        prefs: vec![PreferenceItem {
            pref_type: PreferenceType::TextInputGroup as i32,
            title: "ImgBB API Key".into(),
            id: "imgbb_api_key".into(),
            subtitle: "Optional ImgBB API key for uploading local cover art (falls back to 24h Litterbox upload if empty)".into(),
            ..Default::default()
        }],
    });
    info!("Initialized discord rpc");
}

pub fn main() {}
