#![no_main]
#![allow(dead_code)]

use std::time::Duration;

use moosync_edk::{
    MoosyncResult, PlaybackDetailsRequestedRequest, PreferenceData, PreferenceItem, PreferenceType,
    RequestedAlbumSongsRequest, RequestedAlbumSongsResponse, RequestedArtistSongsRequest,
    RequestedArtistSongsResponse, RequestedPlaylistSongsRequest, RequestedPlaylistSongsResponse,
    RequestedRecommendationsRequest, RequestedRecommendationsResponse, RequestedSearchResultRequest,
    RequestedSongFromIdRequest, SearchResult, Song,
    api::{
        AccountLoginArgs, Accounts, ContextMenu, CustomRequest, CustomRequestReturnType,
        DatabaseEvents, Extension, ExtensionAccountDetail, PlaybackDetailsReturnType,
        PlayerEvents, PreferenceChangedRequest, PreferenceEvents, Provider,
        SongsWithPageTokenReturnType,
        extension_api::{
            get_cache, get_secure, register_user_preferences, set_account, set_cache,
            set_cache_with_ttl, set_secure,
        },
    },
    handler::register_extension,
    info,
};

mod cache_keys;
mod client;
mod utils;

use client::AppleMusicClient;
use utils::{
    cast_prefdata, serde_to_google_value,
};

struct AppleMusic {
    client: AppleMusicClient,
}

impl AppleMusic {
    pub fn new() -> (Self, Option<String>) {
        let token = Self::get_stored_token();
        (
            Self {
                client: AppleMusicClient::new(token.clone()),
            },
            token,
        )
    }

    fn get_stored_token() -> Option<String> {
        let pref = get_secure(PreferenceData {
            key: "media_user_token".into(),
            ..Default::default()
        })
        .ok()?;
        cast_prefdata::<String>(pref).ok().flatten()
    }

    fn sync_account(&self, token: Option<&str>) {
        let logged_in = token.is_some();
        let username = if logged_in {
            Some("Apple Music User".into())
        } else {
            None
        };
        let _ = set_account(ExtensionAccountDetail {
            id: "apple_music".into(),
            package_name: "moosync.apple.music".into(),
            name: "Apple Music".into(),
            bg_color: "#FA233B".into(),
            logged_in,
            icon: "".into(),
            username,
        });
    }
}

impl Extension for AppleMusic {}
impl DatabaseEvents for AppleMusic {}
impl PlayerEvents for AppleMusic {}
impl ContextMenu for AppleMusic {}

impl PreferenceEvents for AppleMusic {
    fn on_preferences_changed(&self, req: PreferenceChangedRequest) -> MoosyncResult<()> {
        if let Some(pref_args) = req.preference {
            if pref_args.key == "media_user_token" {
                if let Some(st) = pref_args.value {
                    let val = utils::google_struct_to_serde(st);
                    let token = val
                        .get("value")
                        .or_else(|| val.get("media_user_token"))
                        .and_then(|v| v.as_str())
                        .or_else(|| val.as_str());

                    if let Some(token) = token {
                        let _ = set_secure(PreferenceData {
                            key: "media_user_token".into(),
                            value: Some(serde_to_google_value(serde_json::Value::String(token.to_string()))),
                            ..Default::default()
                        });
                        self.client.set_user_token(token)?;
                        self.sync_account(Some(token));
                    }
                }
            }
        }
        Ok(())
    }
}

impl Accounts for AppleMusic {
    fn perform_account_login(&self, args: AccountLoginArgs) -> MoosyncResult<String> {
        if args.login_status {
            Ok("Please set your Apple Music media-user-token in extension preferences.".into())
        } else {
            let _ = set_secure(PreferenceData {
                key: "media_user_token".into(),
                value: None,
                ..Default::default()
            });
            let _ = self.client.set_user_token("");
            self.sync_account(None);
            Ok("".into())
        }
    }
}

impl Provider for AppleMusic {
    fn search(&self, req: RequestedSearchResultRequest) -> MoosyncResult<SearchResult> {
        info!("Search requested for query: {}", req.query);
        let cache_key = cache_keys::search(&req.query);
        if let Ok(Some(cached)) = get_cache::<SearchResult>(&cache_key) {
            return Ok(cached);
        }

        let res = self.client.search(req.query)?;
        let _ = set_cache_with_ttl(&cache_key, &res, Some(7200));
        Ok(res)
    }

    fn get_song_from_id(&self, req: RequestedSongFromIdRequest) -> MoosyncResult<Option<Song>> {
        let id = req.id.trim_start_matches("applemusic:");
        info!("Get song from ID: {}", id);
        let cache_key = cache_keys::song(id);
        if let Ok(Some(cached)) = get_cache::<Song>(&cache_key) {
            return Ok(Some(cached));
        }

        let song = self.client.get_song(id)?;
        if let Some(ref s) = song {
            let _ = set_cache(&cache_key, s);
        }
        Ok(song)
    }

    fn get_playlist_content(
        &self,
        req: RequestedPlaylistSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let id = req
            .id
            .trim_start_matches("applemusic:playlist:")
            .trim_start_matches("applemusic:");
        info!("Get playlist songs: {}", id);
        let cache_key = cache_keys::playlist_songs(id, req.page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedPlaylistSongsResponse>(&cache_key) {
            return Ok(SongsWithPageTokenReturnType {
                songs: cached.songs,
                next_page_token: cached.next_page_token,
            });
        }

        let songs = self.client.get_playlist_songs(id)?;
        let _ = set_cache_with_ttl(
            &cache_key,
            &RequestedPlaylistSongsResponse {
                songs: songs.clone(),
                next_page_token: None,
            },
            Some(3600),
        );

        Ok(SongsWithPageTokenReturnType {
            songs,
            next_page_token: None,
        })
    }

    fn get_album_songs(
        &self,
        req: RequestedAlbumSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let album = req.album.ok_or("No album provided")?;
        let id = album.album_id.unwrap_or_default();
        let clean_id = id
            .trim_start_matches("applemusic:album:")
            .trim_start_matches("applemusic:");
        info!("Get album songs: {}", clean_id);
        let cache_key = cache_keys::album_songs(clean_id, req.page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedAlbumSongsResponse>(&cache_key) {
            return Ok(SongsWithPageTokenReturnType {
                songs: cached.songs,
                next_page_token: cached.next_page_token,
            });
        }

        let songs = self.client.get_album_songs(clean_id)?;
        let _ = set_cache(
            &cache_key,
            &RequestedAlbumSongsResponse {
                songs: songs.clone(),
                next_page_token: None,
            },
        );

        Ok(SongsWithPageTokenReturnType {
            songs,
            next_page_token: None,
        })
    }

    fn get_artist_songs(
        &self,
        req: RequestedArtistSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let artist = req.artist.ok_or("No artist provided")?;
        let id = artist.artist_id.unwrap_or_default();
        let clean_id = id
            .trim_start_matches("applemusic:artist:")
            .trim_start_matches("applemusic:");
        info!("Get artist songs: {}", clean_id);
        let cache_key = cache_keys::artist_songs(clean_id, req.page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedArtistSongsResponse>(&cache_key) {
            return Ok(SongsWithPageTokenReturnType {
                songs: cached.songs,
                next_page_token: cached.next_page_token,
            });
        }

        let songs = self.client.get_artist_songs(clean_id)?;
        let _ = set_cache_with_ttl(
            &cache_key,
            &RequestedArtistSongsResponse {
                songs: songs.clone(),
                next_page_token: None,
            },
            Some(86400),
        );

        Ok(SongsWithPageTokenReturnType {
            songs,
            next_page_token: None,
        })
    }

    fn get_recommendations(
        &self,
        req: RequestedRecommendationsRequest,
    ) -> MoosyncResult<Vec<Song>> {
        info!("Get recommendations");
        let cache_key = cache_keys::recommendations();
        if !req.refresh {
            if let Ok(Some(cached)) = get_cache::<RequestedRecommendationsResponse>(&cache_key) {
                return Ok(cached.songs);
            }
        }

        let songs = self.client.get_recommendations()?;
        let _ = set_cache_with_ttl(
            &cache_key,
            &RequestedRecommendationsResponse {
                songs: songs.clone(),
            },
            Some(3600),
        );

        Ok(songs)
    }

    fn get_playback_details(
        &self,
        req: PlaybackDetailsRequestedRequest,
    ) -> MoosyncResult<PlaybackDetailsReturnType> {
        let song = req.song.ok_or("No song provided")?;
        let inner = song.song.ok_or("No inner song provided")?;
        let id = inner.id.unwrap_or_default();
        let clean_id = id.trim_start_matches("applemusic:");
        info!("Get playback details for: {}", clean_id);

        let (url, drm_key, duration) = self.client.get_playback_details(clean_id)?;
        if let Some(ref key) = drm_key {
            info!(
                "Acquired Widevine DRM decryption key for {}: {}",
                clean_id, key
            );
        }

        Ok(PlaybackDetailsReturnType {
            url,
            duration: duration.unwrap_or(Duration::from_secs(0)),
            decryption_key: drm_key,
        })
    }

    fn handle_custom_request(&self, req: CustomRequest) -> MoosyncResult<CustomRequestReturnType> {
        if req.request_id == "get_drm_key" {
            if let Some(ref payload) = req.payload {
                let val = utils::google_struct_to_serde(payload.clone());
                if let Some(song_id) = val.get("song_id").and_then(|v| v.as_str()) {
                    let clean_id = song_id.trim_start_matches("applemusic:");
                    let (_url, drm_key, _duration) = self.client.get_playback_details(clean_id)?;
                    return Ok(CustomRequestReturnType {
                        mime_type: Some("text/plain".into()),
                        data: drm_key.map(|k| k.into_bytes()),
                        redirect_url: None,
                    });
                }
            }
        }

        Err("Unknown custom request".into())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    info!("Initializing Apple Music extension");
    let (ext, token) = AppleMusic::new();
    ext.sync_account(token.as_deref());
    register_extension(Box::new(ext)).unwrap();

    let _ = register_user_preferences(vec![PreferenceItem {
        pref_type: PreferenceType::TextInputGroup as i32,
        title: "Media User Token".into(),
        id: "media_user_token".into(),
        subtitle: "Apple Music account media-user-token cookie".into(),
        ..Default::default()
    }]);
    info!("Initialized Apple Music extension");
}

fn main() {}
