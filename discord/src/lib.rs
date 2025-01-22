use std::{collections::HashMap, fmt::format, sync::Mutex};

use bytes::BytesMut;
use moosync_edk::{
    api::{
        extension_api::{
            self, get_current_song, get_secure, get_system_time, open_sock, read_sock,
            register_oauth, set_secure, update_accounts, write_sock,
        },
        Accounts, ContextMenu, DatabaseEvents, Extension, PlayerEvents, PreferenceEvents, Provider,
    },
    config, error,
    handler::register_extension,
    info, ExtensionProviderScope, PlayerState, Result as MoosyncResult, Song, SongType,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
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
    id: i64,
}

impl Sock {
    pub fn new(id: i64) -> Self {
        Self { id }
    }
}

impl Sock {
    fn encode<T>(op: Operation, data: T) -> MoosyncResult<BytesMut>
    where
        T: Serialize,
    {
        let data_string = serde_json::to_string(&data)?;

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
        let data = read_sock(self.id, 8)?;
        if data.len() < 8 {
            return Err("Packet not big enough to be decoded".into());
        }

        let op = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let op = Operation::from_u32(op);

        let len = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;

        let buf = read_sock(self.id, len as u64)?;
        info!("Read buf {:?}", buf);
        let data: T = serde_json::from_slice(&buf)?;
        Ok(DecodedData { op, data })
    }

    fn write<T>(&self, op: Operation, data: T) -> MoosyncResult<()>
    where
        T: Serialize,
    {
        let encoded = Self::encode(op, data)?;
        info!("Encoded");
        let res = write_sock(self.id, encoded.to_vec());
        info!("Wrote to sock");
        // if res == -1 {
        // return Err("Failed to write to sock".into());
        // }
        Ok(())
    }

    fn uuid4122() -> String {
        let mut uuid = String::new();
        let mut buffer = [0u8; 16];
        getrandom::getrandom(&mut buffer).expect("Failed to generate random bytes");

        for i in 0..32 {
            if i == 8 || i == 12 || i == 16 || i == 20 {
                uuid.push('-');
            }

            let n = if i == 12 {
                4
            } else if i == 16 {
                (buffer[i / 2] & 0b0011_1111) | 0b1000_0000
            } else {
                buffer[i / 2] >> ((1 - (i % 2)) * 4) & 0xF
            };

            uuid.push_str(&format!("{:x}", n));
        }

        uuid
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
    pub evt: String,
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
    instance: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetActivityRequest {
    activity: Activity,
    pid: u64,
}

impl DiscordRPC {
    pub fn new() -> Self {
        Self {
            sock: Mutex::new(None),
        }
    }

    fn open_sock(&self) -> MoosyncResult<()> {
        let mut sock = self.sock.lock().unwrap();

        let id = open_sock("/discord-ipc-0".to_string())?;
        if id == -1 {
            return Err("Failed to connect to sock".into());
        }

        *sock = Some(Sock::new(id));

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
            sock.write(
                Operation::HANDSHAKE,
                HandshakeRequest {
                    v: 1,
                    client_id: CLIENT_ID.to_string(),
                },
            )?;

            let data = sock.read_and_decode::<HandshakeResponse>()?;
            info!("Got res {:?}", data.data);
            if let Operation::CLOSE = data.op {
                *sock_mutex = None
            }
        }

        Ok(())
    }

    fn parse_song(&self, song: Song, status: PlayerState, start_time: u64) -> Activity {
        let artists = song.artists.unwrap_or_default();

        let state = format!(
            "{} - {}",
            artists
                .iter()
                .filter_map(|a| a.artist_name.clone())
                .collect::<Vec<String>>()
                .join(", "),
            song.album
                .unwrap_or_default()
                .album_name
                .unwrap_or("Misc".into())
        );
        let details = format!(
            "{} {}",
            song.song.title.unwrap_or_default(),
            if let PlayerState::Paused = status {
                "(Paused)"
            } else {
                ""
            }
        );

        let start_time = get_system_time() - start_time;

        let mut buttons = vec![];
        if song.song.playback_url.is_some() {
            if let SongType::YOUTUBE = song.song.type_ {
                buttons.push(ActivityButton {
                    label: "Show on YouTube".into(),
                    url: format!(
                        "https://www.youtube.com/watch?v={}",
                        song.song.playback_url.unwrap()
                    ),
                });
            }
        }

        if song.song.url.is_some() {
            if let SongType::SPOTIFY = song.song.type_ {
                buttons.push(ActivityButton {
                    label: "Show on Spotify".into(),
                    url: format!("https://open.spotify.com/track/{}", song.song.url.unwrap()),
                });
            }
        }

        Activity {
            state: Some(state),
            details: Some(details),
            timestamps: Some(Timestamps {
                start: start_time,
                end: None,
            }),
            assets: Some(Assets {
                large_image: "logo_border".to_string(),
                large_text: "Title".into(),
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

        if let Some(sock) = self.sock.lock().unwrap().as_ref() {
            let pid = config::get("pid")
                .expect("Missing pid config")
                .unwrap()
                .parse::<u64>()
                .unwrap();

            if let Some(song) = song {
                let activity = self.parse_song(song, state, start_time);
                sock.request("SET_ACTIVITY", SetActivityRequest { pid, activity })?;
            } else {
                sock.request(
                    "SET_ACTIVITY",
                    SetActivityRequest {
                        pid,
                        activity: Activity::default(),
                    },
                )?;
            }
        }

        Ok(())
    }
}

impl PlayerEvents for DiscordRPC {
    fn on_song_changed(&self) -> MoosyncResult<()> {
        let current_song = extension_api::get_current_song()?;
        let player_state = extension_api::get_player_state()?;
        let time = extension_api::get_time()?;
        self.set_activity(current_song, player_state, time as u64)?;
        Ok(())
    }

    fn on_seeked(&self, time: f64) -> MoosyncResult<()> {
        let current_song = extension_api::get_current_song()?;
        let player_state = extension_api::get_player_state()?;
        self.set_activity(current_song, player_state, time as u64)?;

        Ok(())
    }

    fn on_player_state_changed(&self) -> MoosyncResult<()> {
        let current_song = extension_api::get_current_song()?;
        let player_state = extension_api::get_player_state()?;
        let time = extension_api::get_time()?;

        self.set_activity(current_song, player_state, time as u64)?;

        Ok(())
    }
}
impl Provider for DiscordRPC {
    fn get_provider_scopes(&self) -> MoosyncResult<Vec<ExtensionProviderScope>> {
        Ok(vec![])
    }
}
impl DatabaseEvents for DiscordRPC {}
impl PreferenceEvents for DiscordRPC {}
impl Extension for DiscordRPC {}
impl ContextMenu for DiscordRPC {}
impl Accounts for DiscordRPC {}

#[no_mangle]
pub extern "C" fn init() {
    info!("Initializing discord rpc");

    let extension = DiscordRPC::new();
    if let Err(e) = extension.set_activity(Some(Song::default()), PlayerState::Paused, 0) {
        info!("error connecting to rpc {:?}", e);
    }

    register_extension(Box::new(extension)).unwrap();
    info!("Initialized discord rpc");
}
