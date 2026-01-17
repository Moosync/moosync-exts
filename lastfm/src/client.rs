use core::{fmt, str};
use std::collections::HashMap;

use md5::{Digest, Md5};
use moosync_edk::{
    api::extension_api::get_system_time, error, http, info, HttpRequest, MoosyncResult, Song,
};
use regex::Regex;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use url::Url;

const BASE_URL: &str = "http://ws.audioscrobbler.com/2.0";

#[derive(Debug, Serialize, Deserialize)]
struct GetSessionResponse {
    pub session: Session,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Session {
    pub name: String,
    pub key: String,
}

#[derive(Debug, Default)]
pub struct Client {
    api_key: String,
    secret: String,
    session: Option<Session>,
}

#[derive(Debug, Default)]
struct ScrobbleData {
    artist: String,
    track: String,
    timestamp: u64,
    sk: String,
    album: Option<String>,
    mbid: Option<String>,
    duration: Option<f64>,
    album_artist: Option<String>,
}

enum ApiMethod {
    GetSession(String),
    Scrobble(ScrobbleData),
    UpdateNowPlaying(ScrobbleData),
}

impl fmt::Display for ApiMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parsed_session = match self {
            ApiMethod::GetSession(_) => "auth.getSession",
            ApiMethod::Scrobble(_) => "track.scrobble",
            ApiMethod::UpdateNowPlaying(_) => "track.updateNowPlaying",
        };
        write!(f, "{}", parsed_session)
    }
}

impl Client {
    fn get_method<T>(&self, method: ApiMethod) -> MoosyncResult<T>
    where
        T: DeserializeOwned + for<'de> Deserialize<'de>,
    {
        let http_method = match method {
            ApiMethod::GetSession(_) => "GET",
            ApiMethod::UpdateNowPlaying(_) | ApiMethod::Scrobble(_) => "POST",
        };

        let mut params = HashMap::new();
        params.insert("method".to_string(), method.to_string());
        params.insert("api_key".to_string(), self.api_key.clone());
        params.insert("format".to_string(), "json".to_string());

        match method {
            ApiMethod::GetSession(token) => {
                params.insert("token".to_string(), token);
            }
            ApiMethod::UpdateNowPlaying(data) | ApiMethod::Scrobble(data) => {
                params.insert("artist".to_string(), data.artist);
                params.insert("track".to_string(), data.track);
                params.insert("timestamp".to_string(), data.timestamp.to_string());
                params.insert("album".to_string(), data.album.unwrap_or_default());
                params.insert("mbid".to_string(), data.mbid.unwrap_or_default());
                params.insert(
                    "albumArtist".to_string(),
                    data.album_artist.unwrap_or_default(),
                );
                params.insert(
                    "duration".to_string(),
                    data.duration.unwrap_or_default().to_string(),
                );
                params.insert("sk".to_string(), data.sk);
            }
        }

        params = params.into_iter().filter(|a| !a.1.is_empty()).collect();

        let mut url = Url::parse(BASE_URL).unwrap();

        let sig = self.get_sig(&params);
        params.insert("api_sig".to_string(), sig);

        let (url, body) = if http_method == "GET" {
            let query_string = params
                .into_iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<String>>()
                .join("&");
            url.set_query(Some(query_string.as_str()));
            (url, None)
        } else {
            let body = params
                .into_iter()
                .map(|p| format!("{}={}", p.0, urlencoding::encode(&p.1)))
                .collect::<Vec<String>>()
                .join("&");
            (url, Some(body))
        };

        info!("Calling URL {}", url);
        let request = HttpRequest::new(url)
            .with_method(http_method)
            .with_header("Content-Type", "application/x-www-form-urlencoded");
        info!("http body {:?}", body);
        match http::request(&request, body) {
            Ok(resp) => {
                // info!("Got response {:?}", str::from_utf8(&resp.body()));
                let body = resp.body();
                if let Ok(parsed) = serde_json::from_slice(&body) {
                    return Ok(parsed);
                }
                Err(format!("Failed to parse response {:?}", str::from_utf8(&body)).into())
            }
            Err(e) => Err(format!("Error calling lastfm API {:?}", e).into()),
        }
    }

    fn get_sig(&self, params: &HashMap<String, String>) -> String {
        let mut sig_raw = String::new();

        let mut keys: Vec<&String> = params
            .keys()
            .filter(|&k| k != "format" && k != "callback" && k != "cb")
            .collect();
        keys.sort();
        // Iterate through the HashMap, processing only the valid keys
        for key in keys {
            if let Some(value) = params.get(key) {
                sig_raw.push_str(&format!("{}{}", key, value));
            }
        }

        let sig_raw = format!("{}{}", sig_raw, self.secret);

        let mut hasher = Md5::new();
        hasher.update(sig_raw.clone());
        let res = hasher.finalize();

        let ret = format!("{:X}", res);
        info!("Calculating signature for {}: {}", sig_raw, ret);
        ret
    }

    pub fn login(&self) -> String {
        format!(
            "http://www.last.fm/api/auth/?api_key={}&cb=https://moosync.app/lastfm",
            self.api_key
        )
    }

    pub fn logout(&mut self) {
        self.session = None;
    }

    pub fn authorize(&mut self, code: String) -> MoosyncResult<Session> {
        let re = Regex::new(r"(?i)[?&]token=([^&]+)").unwrap();
        if let Some(token) = re.captures(&code) {
            let token = &token[1];
            info!("Got token {}", token);

            return match self
                .get_method::<GetSessionResponse>(ApiMethod::GetSession(token.to_string()))
            {
                Ok(method) => {
                    self.session = Some(method.session.clone());
                    Ok(method.session)
                }
                Err(e) => Err(format!("Error while authorizing: {}", e).into()),
            };
        }
        Err("Invalid token".into())
    }

    pub fn get_username(&self) -> Option<String> {
        if let Some(session) = &self.session {
            return Some(session.name.clone());
        }
        None
    }

    pub fn set_session(&mut self, session: Session) {
        self.session = Some(session);
    }

    fn get_scrobble_data(&self, song: Song) -> MoosyncResult<ScrobbleData> {
        if let Some(session) = &self.session {
            let artist = song
                .artists
                .first()
                .map(|a| a.artist_name.clone())
                .flatten()
                .unwrap_or("unknown".to_string());
            let track = song
                .song
                .clone()
                .and_then(|s| s.title)
                .unwrap_or("unknown".to_string());
            let timestamp = get_system_time() - 20;
            let sk = session.key.clone();
            let album = song.album.clone().and_then(|a| a.album_name);
            let duration = song.song.clone().and_then(|s| s.duration);
            let album_artist = song.album.and_then(|a| a.album_artist);

            return Ok(ScrobbleData {
                artist,
                track,
                timestamp,
                sk,
                album,
                duration,
                album_artist,
                ..Default::default()
            });
        }
        Err("User not logged in".into())
    }

    pub fn scrobble(&self, song: Song) {
        if let Ok(scrobble_data) = self.get_scrobble_data(song) {
            if let Err(e) = self.get_method::<Value>(ApiMethod::Scrobble(scrobble_data)) {
                error!("Failed to scrobble track {}", e);
            }
        }
    }

    pub fn set_now_playing(&self, song: Song) {
        if let Ok(scrobble_data) = self.get_scrobble_data(song) {
            if let Err(e) = self.get_method::<Value>(ApiMethod::UpdateNowPlaying(scrobble_data)) {
                error!("Failed to update now playing track {}", e);
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Builder {
    api_key: Option<String>,
    secret: Option<String>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn with_api_key(&self, api_key: impl Into<String>) -> Self {
        Self {
            api_key: Some(api_key.into()),
            secret: self.secret.clone(),
        }
    }

    pub fn with_secret(&self, secret: impl Into<String>) -> Self {
        Self {
            api_key: self.api_key.clone(),
            secret: Some(secret.into()),
        }
    }

    pub fn build(&self) -> Client {
        Client {
            api_key: self.api_key.clone().unwrap(),
            secret: self.secret.clone().unwrap(),
            session: None,
        }
    }
}
