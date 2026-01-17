use futures::executor::block_on;
use moosync_edk::{
    Album, Artist, Genre, InnerSong, Playlist, SearchResult, Song,
    api::{
        AccountLoginArgs, ExtensionAccountDetail, PreferenceData, SongsWithPageTokenReturnType,
        extension_api::{get_preference, get_secure, set_preference, set_secure, update_accounts},
    },
    info,
};
use serde_json::{Value, json};
use std::default::Default;

use crate::error::KoelError;
use crate::utils::{
    KoelSong, make_next_page_token, parse_albums, parse_artists, parse_next_page_token,
    parse_playlists, parse_queryable_songs,
};

#[derive(Debug, Clone, serde::Deserialize)]
struct SongsResponse {
    data: Vec<KoelSong>,
    // Optionally: links, meta, etc.
}

pub struct KoelClient {
    pub token: Option<String>,
    pub koel_url: String,
    username: Option<String>,
    client: reqwest::Client,
}

impl Default for KoelClient {
    fn default() -> Self {
        Self::new()
    }
}

impl KoelClient {
    fn send_json_request<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        use_auth: bool,
        body: Option<&serde_json::Value>,
        error: KoelError,
    ) -> Result<T, KoelError> {
        let url = if endpoint.starts_with("http") {
            endpoint.to_string()
        } else {
            format!(
                "{}/{}",
                self.koel_url.trim_end_matches('/'),
                endpoint.trim_start_matches('/')
            )
        };
        let mut req = self.client.request(method, &url);
        if use_auth {
            let token = self.token.as_ref().ok_or(KoelError::MissingToken)?;
            req = req.bearer_auth(token);
        }
        if let Some(json_body) = body {
            req = req.json(json_body);
        }
        let resp = futures::executor::block_on(req.send())?;
        let status = resp.status();
        if !status.is_success() {
            return Err(error);
        }
        let data = futures::executor::block_on(resp.json())?;
        Ok(data)
    }

    fn map_koel_song(&self, ks: &KoelSong) -> Song {
        Song {
            song: Some(InnerSong {
                id: ks.id.clone(),
                title: ks.title.clone(),
                lyrics: ks.lyrics.clone(),
                duration: ks.length,
                track_no: ks.track.map(|t| t as f64),
                year: ks.year.clone().map(|v| v.to_string()),
                song_cover_path_high: ks.album_cover.clone(),
                show_in_library: Some(true),
                playback_url: ks
                    .id
                    .clone()
                    .map(|id| format!("extension://moosync.koel/{id}")),
                ..Default::default()
            }),
            album: Some(Album {
                album_id: ks.album_id.clone(),
                album_name: ks.album_name.clone(),
                album_coverpath_high: ks.album_cover.clone(),
                year: ks.year.clone().map(|v| v.to_string()),
                ..Default::default()
            }),
            artists: vec![Artist {
                artist_id: ks.artist_id.clone(),
                artist_name: ks.artist_name.clone(),
                ..Default::default()
            }],
            genre: if let Some(genre) = ks.genre.clone() {
                if !genre.is_empty() {
                    vec![Genre {
                        genre_name: Some(genre),
                        ..Default::default()
                    }]
                } else {
                    vec![]
                }
            } else {
                vec![]
            },
            ..Default::default()
        }
    }
    pub fn new() -> Self {
        let koel_url = get_preference(PreferenceData {
            key: "koel_instance_url".to_string(),
            value: None,
        })
        .ok()
        .and_then(|v| v.value.and_then(crate::utils::google_value_to_string))
        .unwrap_or("http://localhost:8000".into());
        let mut s = Self {
            token: None,
            username: None,
            koel_url,
            client: reqwest::Client::new(),
        };
        if let Err(e) = s.login_and_get_token() {
            info!("Error getting token {}", e);
        }
        s
    }

    fn login_and_get_token(&mut self) -> Result<(), KoelError> {
        if self.token.is_some() {
            info!("Token already exists. Skipping...");
            return Ok(());
        }
        let email = get_preference(PreferenceData {
            key: "koel_username".to_string(),
            value: None,
        })
        .ok()
        .and_then(|v| v.value.and_then(crate::utils::google_value_to_string));
        let password = get_secure(PreferenceData {
            key: "koel_password".to_string(),
            value: None,
        })
        .ok()
        .and_then(|v| v.value.and_then(crate::utils::google_value_to_string));

        info!("Loggnig in {:?}, {:?}", email, password);
        if email.is_none() || password.is_none() {
            return Err(KoelError::MissingToken);
        }
        let username = email.unwrap();
        let password = password.unwrap();

        let url = format!("{}/api/me", self.koel_url);
        let body = json!({
            "email": username,
            "password": password
        });
        let v: Value = block_on(block_on(self.client.post(&url).json(&body).send())?.json())?;

        if let Some(token) = v.get("token").and_then(|t| t.as_str()) {
            self.token = Some(token.to_string());
            info!("Setting token {:?}", self.token);
            update_accounts(Some("moosync.koel".into()))?;
            Ok(())
        } else {
            Err(KoelError::MissingToken)
        }
    }

    pub fn search(&mut self, term: String) -> Result<SearchResult, KoelError> {
        let url = format!("api/search?q={}", urlencoding::encode(&term));
        let v: serde_json::Value = self.send_json_request(
            reqwest::Method::GET,
            &url,
            true,
            None,
            KoelError::SearchFailure,
        )?;

        // Songs
        let songs = v
            .get("songs")
            .map(|s| {
                parse_queryable_songs(s)
                    .into_iter()
                    .map(|qs| Song {
                        song: Some(qs),
                        ..Default::default()
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Artists
        let artists = v.get("artists").map(parse_artists).unwrap_or_default();

        // Albums
        let albums = v.get("albums").map(parse_albums).unwrap_or_default();

        Ok(SearchResult {
            songs,
            artists,
            albums,
            ..Default::default()
        })
    }

    pub fn get_song_from_url(&mut self, url: String) -> Result<Option<Song>, KoelError> {
        // Extract song ID from URL (e.g., http://127.0.0.1:8000/#/songs/<id>)
        let id = url
            .split("/#/songs/")
            .nth(1)
            .and_then(|s| {
                // Split on '/', '?', or '#' to sanitize the ID
                s.split(&['/', '?', '#'][..]).next()
            })
            .map(|s| s.to_string());

        let song_id = match id {
            Some(id) if !id.is_empty() => id,
            _ => return Ok(None),
        };

        let api_url = format!("api/songs/{song_id}");
        let ks: KoelSong = self.send_json_request(
            reqwest::Method::GET,
            &api_url,
            true,
            None,
            KoelError::SearchFailure,
        )?;
        Ok(Some(self.map_koel_song(&ks)))
    }

    pub fn get_accounts(&self) -> Result<Vec<ExtensionAccountDetail>, KoelError> {
        Ok(vec![ExtensionAccountDetail {
            id: "koel".into(),
            package_name: "koel".into(),
            name: "Koel".into(),
            bg_color: "#2196f3".into(),
            icon: "".into(),
            logged_in: self.token.is_some(),
            username: self.username.clone(),
        }])
    }

    pub fn perform_account_login(&mut self, args: AccountLoginArgs) -> Result<String, KoelError> {
        if !args.login_status {
            set_preference(PreferenceData {
                key: "koel_username".to_string(),
                value: None,
            })?;
            set_secure(PreferenceData {
                key: "koel_password".to_string(),
                value: None,
            })?;
            return Ok("".into());
        }

        self.login_and_get_token()?;
        Ok("".into())
    }

    pub fn get_playlists(&mut self) -> Result<Vec<Playlist>, KoelError> {
        let v: serde_json::Value = self.send_json_request(
            reqwest::Method::GET,
            "api/playlists",
            true,
            None,
            KoelError::PlaylistError,
        )?;
        // Koel returns either { "playlists": [...] } or just an array
        let playlists = v
            .get("playlists")
            .or(Some(&v))
            .ok_or(KoelError::Other("Missing playlists field".into()))?;

        let mut result: Vec<Playlist> = parse_playlists(playlists);

        result.insert(
            0,
            Playlist {
                playlist_id: Some("all_songs".into()),
                playlist_name: "All songs".into(),
                ..Default::default()
            },
        );
        Ok(result)
    }

    pub fn get_playlist_content(
        &mut self,
        playlist_id: &str,
        next_page_token: Option<String>,
    ) -> Result<SongsWithPageTokenReturnType, KoelError> {
        if playlist_id == "all_songs" {
            return self.get_all_songs(next_page_token);
        }

        let url = format!("api/playlists/{playlist_id}/songs");
        let koel_songs: Vec<KoelSong> = self.send_json_request(
            reqwest::Method::GET,
            &url,
            true,
            None,
            KoelError::PlaylistContentError,
        )?;
        let songs = koel_songs.iter().map(|ks| self.map_koel_song(ks)).collect();

        Ok(SongsWithPageTokenReturnType {
            songs,
            next_page_token: None,
        })
    }

    pub fn get_all_songs(
        &mut self,
        next_page_token: Option<String>,
    ) -> Result<SongsWithPageTokenReturnType, KoelError> {
        // Extract current page from next_page_token, default to 1
        let page = parse_next_page_token(&next_page_token, 1);

        info!("Got page {}, {:?}", page, next_page_token);

        let url = format!("api/songs?page={page}");
        let songs_response: SongsResponse = self.send_json_request(
            reqwest::Method::GET,
            &url,
            true,
            None,
            KoelError::SearchFailure,
        )?;
        let songs: Vec<Song> = songs_response
            .data
            .iter()
            .map(|ks| self.map_koel_song(ks))
            .collect();
        let next_page_token = make_next_page_token(page, !songs.is_empty());
        Ok(SongsWithPageTokenReturnType {
            songs,
            next_page_token,
        })
    }

    pub fn handle_custom_request(&self, url: String) -> Result<String, KoelError> {
        let id = url.replace("extension://moosync.koel/", "");
        if let Some(token) = &self.token {
            Ok(format!("{}/play/{}?t={}", self.koel_url, id, token))
        } else {
            Err(KoelError::MissingToken)
        }
    }

    /// Search for an album by name, and if an exact match is found,
    /// return its songs (with pagination support if available) and the next page token.
    pub fn get_songs_by_album_name(
        &mut self,
        album_name: &str,
        next_page_token: Option<String>,
    ) -> Result<SongsWithPageTokenReturnType, KoelError> {
        // 1. Search for the album
        let search_result = self.search(album_name.to_string())?;
        let album = search_result
            .albums
            .iter()
            .find(|a| a.album_name.as_deref() == Some(album_name));
        let album_id = match album.and_then(|a| a.album_id.clone()) {
            Some(id) => id,
            None => {
                // No exact match found
                return Ok(SongsWithPageTokenReturnType {
                    songs: vec![],
                    next_page_token: None,
                });
            }
        };

        // 2. Fetch songs for the album using the documented endpoint
        let page = parse_next_page_token(&next_page_token, 1);

        let url = format!("api/albums/{album_id}/songs?page={page}");
        let koel_songs: Vec<KoelSong> = self.send_json_request(
            reqwest::Method::GET,
            &url,
            true,
            None,
            KoelError::SearchFailure,
        )?;
        let songs: Vec<Song> = koel_songs.iter().map(|ks| self.map_koel_song(ks)).collect();
        let next_page_token = make_next_page_token(page, !songs.is_empty());
        Ok(SongsWithPageTokenReturnType {
            songs,
            next_page_token,
        })
    }

    /// Search for an artist by name, and if an exact match is found,
    /// return their songs (with pagination support if available) and the next page token.
    pub fn get_songs_by_artist_name(
        &mut self,
        artist_name: &str,
        next_page_token: Option<String>,
    ) -> Result<SongsWithPageTokenReturnType, KoelError> {
        // 1. Search for the artist
        let search_result = self.search(artist_name.to_string())?;
        let artist = search_result
            .artists
            .iter()
            .find(|a| a.artist_name.as_deref() == Some(artist_name));
        let artist_id = match artist.and_then(|a| a.artist_id.clone()) {
            Some(id) => id,
            None => {
                // No exact match found
                return Ok(SongsWithPageTokenReturnType {
                    songs: vec![],
                    next_page_token: None,
                });
            }
        };

        // 2. Fetch songs for the artist using the documented endpoint
        let page = parse_next_page_token(&next_page_token, 1);

        let url = format!("api/artists/{artist_id}/songs?page={page}");
        let koel_koel_songs: Vec<KoelSong> = self.send_json_request(
            reqwest::Method::GET,
            &url,
            true,
            None,
            KoelError::SearchFailure,
        )?;
        let songs: Vec<Song> = koel_koel_songs
            .iter()
            .map(|ks| self.map_koel_song(ks))
            .collect();
        let next_page_token = make_next_page_token(page, !songs.is_empty());
        Ok(SongsWithPageTokenReturnType {
            songs,
            next_page_token,
        })
    }
}
