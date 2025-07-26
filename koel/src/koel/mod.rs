use futures::executor::block_on;
use moosync_edk::{
    AccountLoginArgs, ExtensionAccountDetail, ExtensionProviderScope, PreferenceData,
    QueryableAlbum, QueryableArtist, QueryableGenre, QueryablePlaylist, QueryableSong,
    SearchResult, Song, SongsWithPageTokenReturnType,
    api::extension_api::{get_preference, get_secure, set_preference, set_secure, update_accounts},
    info,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::default::Default;

use crate::error::KoelError;

#[derive(Debug, Clone, Deserialize)]
struct KoelSong {
    id: Option<String>,
    title: Option<String>,
    lyrics: Option<String>,
    album_id: Option<String>,
    album_name: Option<String>,
    album_cover: Option<String>,
    artist_id: Option<String>,
    artist_name: Option<String>,
    track: Option<u32>,
    length: Option<f64>,
    genre: Option<String>,
    year: Option<serde_json::Value>,
}

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

impl KoelClient {
    fn map_koel_song(&self, ks: &KoelSong) -> Song {
        Song {
            song: QueryableSong {
                _id: ks.id.clone(),
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
                    .map(|id| format!("extension://moosync.koel/{}", id)),
                ..Default::default()
            },
            album: Some(QueryableAlbum {
                album_id: ks.album_id.clone(),
                album_name: ks.album_name.clone(),
                album_coverpath_high: ks.album_cover.clone(),
                year: ks.year.clone().map(|v| v.to_string()),
                ..Default::default()
            }),
            artists: Some(vec![QueryableArtist {
                artist_id: ks.artist_id.clone(),
                artist_name: ks.artist_name.clone(),
                ..Default::default()
            }]),
            genre: if let Some(genre) = ks.genre.clone() {
                if !genre.is_empty() {
                    Some(vec![QueryableGenre {
                        genre_name: Some(genre),
                        ..Default::default()
                    }])
                } else {
                    None
                }
            } else {
                None
            },
            ..Default::default()
        }
    }
    pub fn new() -> Self {
        let koel_url = get_preference(PreferenceData {
            key: "koel_instance_url".to_string(),
            value: None,
            default_value: None,
        })
        .ok()
        .and_then(|v| v.value.and_then(|v| v.as_str().map(|s| s.to_string())))
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
            default_value: None,
        })
        .ok()
        .and_then(|v| v.value.and_then(|v| v.as_str().map(|s| s.to_string())));
        let password = get_secure(PreferenceData {
            key: "koel_password".to_string(),
            value: None,
            default_value: None,
        })
        .ok()
        .and_then(|v| v.value.and_then(|v| v.as_str().map(|s| s.to_string())));

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
        let token = self.token.as_ref().ok_or(KoelError::MissingToken)?;
        let url = format!(
            "{}/api/search?q={}",
            self.koel_url,
            urlencoding::encode(&term)
        );
        let req = self.client.get(&url).bearer_auth(token);
        let resp = futures::executor::block_on(req.send())?;
        let status = resp.status();
        if !status.is_success() {
            return Err(KoelError::SearchFailure);
        }
        let v: serde_json::Value = futures::executor::block_on(resp.json())?;

        // Songs
        let songs = v
            .get("songs")
            .and_then(|s| s.as_array())
            .unwrap_or(&vec![])
            .iter()
            .map(|ks| Song {
                song: QueryableSong {
                    _id: ks.get("id").and_then(|v| v.as_str().map(|s| s.to_string())),
                    title: ks
                        .get("title")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                    lyrics: ks
                        .get("lyrics")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                    duration: ks.get("length").and_then(|v| v.as_f64()),
                    track_no: ks.get("track").and_then(|v| v.as_u64().map(|t| t as f64)),
                    year: ks.get("year").map(|v| v.to_string()),
                    provider_extension: Some("koel".to_string()),
                    song_cover_path_high: ks
                        .get("album_cover")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                    show_in_library: Some(true),
                    playback_url: ks
                        .get("id")
                        .and_then(|id| id.as_str())
                        .map(|id| format!("extension://moosync.koel/{}", id)),
                    ..Default::default()
                },
                album: Some(QueryableAlbum {
                    album_id: ks
                        .get("album_id")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                    album_name: ks
                        .get("album_name")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                    album_coverpath_high: ks
                        .get("album_cover")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                    year: ks.get("year").map(|v| v.to_string()),
                    ..Default::default()
                }),
                artists: Some(vec![QueryableArtist {
                    artist_id: ks
                        .get("artist_id")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                    artist_name: ks
                        .get("artist_name")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                    ..Default::default()
                }]),
                genre: ks.get("genre").and_then(|genre| {
                    if let Some(genre_str) = genre.as_str() {
                        if !genre_str.is_empty() {
                            Some(vec![QueryableGenre {
                                genre_name: Some(genre_str.to_string()),
                                ..Default::default()
                            }])
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }),
                ..Default::default()
            })
            .collect();

        // Artists
        let artists = v
            .get("artists")
            .and_then(|a| a.as_array())
            .unwrap_or(&vec![])
            .iter()
            .map(|ka| QueryableArtist {
                artist_id: ka.get("id").and_then(|v| v.as_str().map(|s| s.to_string())),
                artist_name: ka
                    .get("name")
                    .and_then(|v| v.as_str().map(|s| s.to_string())),
                artist_coverpath: ka
                    .get("image")
                    .and_then(|v| v.as_str().map(|s| s.to_string())),
                ..Default::default()
            })
            .collect();

        // Albums
        let albums = v
            .get("albums")
            .and_then(|a| a.as_array())
            .unwrap_or(&vec![])
            .iter()
            .map(|ka| QueryableAlbum {
                album_id: ka.get("id").and_then(|v| v.as_str().map(|s| s.to_string())),
                album_name: ka
                    .get("name")
                    .and_then(|v| v.as_str().map(|s| s.to_string())),
                album_coverpath_high: ka
                    .get("cover")
                    .and_then(|v| v.as_str().map(|s| s.to_string())),
                year: ka.get("year").map(|v| v.to_string()),
                ..Default::default()
            })
            .collect();

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

        let token = self.token.as_ref().ok_or(KoelError::MissingToken)?;
        let api_url = format!("{}/api/songs/{}", self.koel_url, song_id);

        let req = self.client.get(&api_url).bearer_auth(token);
        let resp = futures::executor::block_on(req.send())?;
        let status = resp.status();
        if !status.is_success() {
            return Err(KoelError::SearchFailure);
        }

        let ks: KoelSong = futures::executor::block_on(resp.json())?;
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
                default_value: None,
            })?;
            set_secure(PreferenceData {
                key: "koel_password".to_string(),
                value: None,
                default_value: None,
            })?;
            return Ok("".into());
        }

        self.login_and_get_token()?;
        Ok("".into())
    }

    pub fn get_playlists(&mut self) -> Result<Vec<QueryablePlaylist>, KoelError> {
        let token = self.token.as_ref().ok_or(KoelError::MissingToken)?;
        let url = format!("{}/api/playlists", self.koel_url);

        let req = self.client.get(&url).bearer_auth(token);

        let resp = futures::executor::block_on(req.send())?;
        let status = resp.status();
        let text = futures::executor::block_on(resp.text())?;

        if !status.is_success() {
            return Err(KoelError::PlaylistError);
        }

        let v: serde_json::Value = serde_json::from_str(&text)?;
        // Koel returns either { "playlists": [...] } or just an array
        let playlists = v
            .get("playlists")
            .or_else(|| Some(&v))
            .ok_or(KoelError::Other("Missing playlists field".into()))?;

        let arr = if let Some(arr) = playlists.as_array() {
            arr
        } else {
            return Err(KoelError::Other("Playlists field is not an array".into()));
        };

        let mut result: Vec<QueryablePlaylist> = arr
            .iter()
            .map(|pl| QueryablePlaylist {
                playlist_id: pl.get("id").map(|v| v.to_string().replace('"', "")),
                playlist_name: pl
                    .get("name")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .unwrap_or_default(),
                playlist_coverpath: None,
                playlist_song_count: 0.0,
                playlist_desc: None,
                playlist_path: None,
                extension: Some("koel".to_string()),
                icon: None,
                library_item: Some(true),
                ..Default::default()
            })
            .collect();

        result.insert(
            0,
            QueryablePlaylist {
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

        let token = self.token.as_ref().ok_or(KoelError::MissingToken)?;
        let url = format!("{}/api/playlists/{}/songs", self.koel_url, playlist_id);

        let req = self.client.get(&url).bearer_auth(token);
        let resp = futures::executor::block_on(req.send())?;
        let status = resp.status();
        if !status.is_success() {
            let text = futures::executor::block_on(resp.text())?;
            info!("Got resp {}", text);
            return Err(KoelError::PlaylistContentError);
        }

        let koel_songs: Vec<KoelSong> = futures::executor::block_on(resp.json())?;
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
        let page = next_page_token
            .as_ref()
            .and_then(|v| {
                serde_json::from_str::<serde_json::Value>(v)
                    .ok()
                    .and_then(|val| val.as_u64())
            })
            .unwrap_or(1);

        info!("Got page {}, {:?}", page, next_page_token);

        let token = self.token.as_ref().ok_or(KoelError::MissingToken)?;
        let url = format!("{}/api/songs?page={}", self.koel_url, page);

        let req = self.client.get(&url).bearer_auth(token);
        let resp = futures::executor::block_on(req.send())?;
        let status = resp.status();
        info!("Status {}", status);
        if !status.is_success() {
            return Err(KoelError::SearchFailure);
        }

        let songs_response: SongsResponse = futures::executor::block_on(resp.json())?;
        let songs: Vec<Song> = songs_response
            .data
            .iter()
            .map(|ks| self.map_koel_song(ks))
            .collect();
        let next_page_token = if songs.is_empty() {
            None
        } else {
            Some(json!(page + 1))
        };
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
        let page = next_page_token
            .as_ref()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(1);

        let token = self.token.as_ref().ok_or(KoelError::MissingToken)?;
        let url = format!(
            "{}/api/albums/{}/songs?page={}",
            self.koel_url, album_id, page
        );

        let req = self.client.get(&url).bearer_auth(token);
        let resp = futures::executor::block_on(req.send())?;
        let status = resp.status();
        if !status.is_success() {
            return Err(KoelError::SearchFailure);
        }

        let koel_songs: Vec<KoelSong> = futures::executor::block_on(resp.json())?;
        let songs: Vec<Song> = koel_songs.iter().map(|ks| self.map_koel_song(ks)).collect();
        let next_page_token = if songs.is_empty() {
            None
        } else {
            Some(json!(page + 1))
        };
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
        let page = next_page_token
            .as_ref()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(1);

        let token = self.token.as_ref().ok_or(KoelError::MissingToken)?;
        let url = format!(
            "{}/api/artists/{}/songs?page={}",
            self.koel_url, artist_id, page
        );

        let req = self.client.get(&url).bearer_auth(token);
        let resp = futures::executor::block_on(req.send())?;
        let status = resp.status();
        if !status.is_success() {
            return Err(KoelError::SearchFailure);
        }

        let koel_koel_songs: Vec<KoelSong> = futures::executor::block_on(resp.json())?;
        let songs: Vec<Song> = koel_koel_songs
            .iter()
            .map(|ks| self.map_koel_song(ks))
            .collect();
        let next_page_token = if songs.is_empty() {
            None
        } else {
            Some(json!(page + 1))
        };
        Ok(SongsWithPageTokenReturnType {
            songs,
            next_page_token,
        })
    }
}
