#![no_main]
#![allow(dead_code)]

use moosync_edk::{
    api::{
        self, duration_to_proto, Accounts, ContextMenu, DatabaseEvents, Extension, PlayerEvents,
        PlayerProvider, PreferenceEvents, Provider,
    },
    cache::{get_cache, set_cache, set_cache_with_ttl},
    handler::register_extension,
    info,
    types::{
        CustomRequest, CustomRequestResponse, ExtensionAccountDetail, GetSecureRequest,
        PerformAccountLoginRequest, PerformAccountLoginResponse, PlaybackDetailsRequestedRequest,
        PlaybackDetailsRequestedResponse, PreferenceChangedRequest, PreferenceData, PreferenceItem,
        PreferenceType, RegisterUserPreferenceRequest, RequestedAlbumSongsRequest,
        RequestedAlbumSongsResponse, RequestedArtistSongsRequest, RequestedArtistSongsResponse,
        RequestedPlaylistSongsRequest, RequestedPlaylistSongsResponse,
        RequestedRecommendationsRequest, RequestedRecommendationsResponse,
        RequestedSearchResultRequest, RequestedSearchResultResponse, RequestedSongFromIdRequest,
        RequestedSongFromIdResponse, SearchResult, SetAccountRequest, SetSecureRequest, Song,
    },
    MoosyncResult,
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
        let resp = api::get_secure(GetSecureRequest {
            data: Some(PreferenceData {
                key: "media_user_token".into(),
                ..Default::default()
            }),
        })
        .ok()?;
        let pref = resp.data?;
        cast_prefdata::<String>(pref).ok().flatten()
    }

    fn sync_account(&self, token: Option<&str>) {
        let logged_in = token.is_some();
        let username = if logged_in {
            Some("Apple Music User".into())
        } else {
            None
        };
        let _ = api::set_account(SetAccountRequest {
            account: Some(ExtensionAccountDetail {
                id: "apple_music".into(),
                package_name: "app.moosync.apple.music".into(),
                name: "Apple Music".into(),
                bg_color: "#FA233B".into(),
                logged_in,
                icon: "".into(),
                username,
            }),
        });
    }
}

impl Extension for AppleMusic {}
impl DatabaseEvents for AppleMusic {}
impl PlayerEvents for AppleMusic {}
impl ContextMenu for AppleMusic {}
impl PlayerProvider for AppleMusic {}

impl PreferenceEvents for AppleMusic {
    fn preference_changed(&self, req: PreferenceChangedRequest) -> MoosyncResult<()> {
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
                        let _ = api::set_secure(SetSecureRequest {
                            data: Some(PreferenceData {
                                key: "media_user_token".into(),
                                value: Some(serde_to_google_value(serde_json::Value::String(token.to_string()))),
                                ..Default::default()
                            }),
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
    fn perform_account_login(&self, args: PerformAccountLoginRequest) -> MoosyncResult<PerformAccountLoginResponse> {
        let status = if args.login_status {
            "Please set your Apple Music media-user-token in extension preferences.".into()
        } else {
            let _ = api::set_secure(SetSecureRequest {
                data: Some(PreferenceData {
                    key: "media_user_token".into(),
                    value: None,
                    ..Default::default()
                }),
            });
            let _ = self.client.set_user_token("");
            self.sync_account(None);
            "".into()
        };
        Ok(PerformAccountLoginResponse { status })
    }
}

impl Provider for AppleMusic {
    fn requested_search_result(&self, req: RequestedSearchResultRequest) -> MoosyncResult<RequestedSearchResultResponse> {
        info!("Search requested for query: {}", req.query);
        let cache_key = cache_keys::search(&req.query);
        if let Ok(Some(cached)) = get_cache::<SearchResult>(&cache_key) {
            return Ok(RequestedSearchResultResponse {
                songs: cached.songs,
                playlists: cached.playlists,
                artists: cached.artists,
                albums: cached.albums,
            });
        }

        let res = self.client.search(req.query)?;
        let _ = set_cache_with_ttl(&cache_key, &res, Some(7200));
        Ok(RequestedSearchResultResponse {
            songs: res.songs,
            playlists: res.playlists,
            artists: res.artists,
            albums: res.albums,
        })
    }

    fn requested_song_from_id(&self, req: RequestedSongFromIdRequest) -> MoosyncResult<RequestedSongFromIdResponse> {
        let id = req.id.trim_start_matches("applemusic:");
        info!("Get song from ID: {}", id);
        let cache_key = cache_keys::song(id);
        if let Ok(Some(cached)) = get_cache::<Song>(&cache_key) {
            return Ok(RequestedSongFromIdResponse { song: Some(cached) });
        }

        let song = self.client.get_song(id)?;
        if let Some(ref s) = song {
            let _ = set_cache(&cache_key, s);
        }
        Ok(RequestedSongFromIdResponse { song })
    }

    fn requested_playlist_songs(
        &self,
        req: RequestedPlaylistSongsRequest,
    ) -> MoosyncResult<RequestedPlaylistSongsResponse> {
        let id = req
            .id
            .trim_start_matches("applemusic:playlist:")
            .trim_start_matches("applemusic:");
        info!("Get playlist songs: {}", id);
        let cache_key = cache_keys::playlist_songs(id, req.page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedPlaylistSongsResponse>(&cache_key) {
            return Ok(cached);
        }

        let songs = self.client.get_playlist_songs(id)?;
        let resp = RequestedPlaylistSongsResponse {
            songs,
            next_page_token: None,
        };
        let _ = set_cache_with_ttl(
            &cache_key,
            &resp,
            Some(3600),
        );

        Ok(resp)
    }

    fn requested_album_songs(
        &self,
        req: RequestedAlbumSongsRequest,
    ) -> MoosyncResult<RequestedAlbumSongsResponse> {
        let album = req.album.ok_or("No album provided")?;
        let id = album.album_id.unwrap_or_default();
        let clean_id = id
            .trim_start_matches("applemusic:album:")
            .trim_start_matches("applemusic:");
        info!("Get album songs: {}", clean_id);
        let cache_key = cache_keys::album_songs(clean_id, req.page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedAlbumSongsResponse>(&cache_key) {
            return Ok(cached);
        }

        let songs = self.client.get_album_songs(clean_id)?;
        let resp = RequestedAlbumSongsResponse {
            songs,
            next_page_token: None,
        };
        let _ = set_cache(
            &cache_key,
            &resp,
        );

        Ok(resp)
    }

    fn requested_artist_songs(
        &self,
        req: RequestedArtistSongsRequest,
    ) -> MoosyncResult<RequestedArtistSongsResponse> {
        let artist = req.artist.ok_or("No artist provided")?;
        let id = artist.artist_id.unwrap_or_default();
        let clean_id = id
            .trim_start_matches("applemusic:artist:")
            .trim_start_matches("applemusic:");
        info!("Get artist songs: {}", clean_id);
        let cache_key = cache_keys::artist_songs(clean_id, req.page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedArtistSongsResponse>(&cache_key) {
            return Ok(cached);
        }

        let songs = self.client.get_artist_songs(clean_id)?;
        let resp = RequestedArtistSongsResponse {
            songs,
            next_page_token: None,
        };
        let _ = set_cache_with_ttl(
            &cache_key,
            &resp,
            Some(86400),
        );

        Ok(resp)
    }

    fn requested_recommendations(
        &self,
        req: RequestedRecommendationsRequest,
    ) -> MoosyncResult<RequestedRecommendationsResponse> {
        info!("Get recommendations");
        let cache_key = cache_keys::recommendations();
        if !req.refresh {
            if let Ok(Some(cached)) = get_cache::<RequestedRecommendationsResponse>(&cache_key) {
                return Ok(cached);
            }
        }

        let songs = self.client.get_recommendations()?;
        let resp = RequestedRecommendationsResponse { songs };
        let _ = set_cache_with_ttl(
            &cache_key,
            &resp,
            Some(3600),
        );

        Ok(resp)
    }

    fn playback_details_requested(
        &self,
        req: PlaybackDetailsRequestedRequest,
    ) -> MoosyncResult<PlaybackDetailsRequestedResponse> {
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

        Ok(PlaybackDetailsRequestedResponse {
            url,
            duration: Some(duration_to_proto(duration.unwrap_or_default())),
            decryption_key: drm_key,
            ..Default::default()
        })
    }

    fn custom_request(&self, req: CustomRequest) -> MoosyncResult<CustomRequestResponse> {
        if req.request_id == "get_drm_key" {
            if let Some(ref payload) = req.payload {
                let val = utils::google_struct_to_serde(payload.clone());
                if let Some(song_id) = val.get("song_id").and_then(|v| v.as_str()) {
                    let clean_id = song_id.trim_start_matches("applemusic:");
                    let (_url, drm_key, _duration) = self.client.get_playback_details(clean_id)?;
                    return Ok(CustomRequestResponse {
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

    let _ = api::register_user_preference(RegisterUserPreferenceRequest {
        prefs: vec![PreferenceItem {
            pref_type: PreferenceType::TextInputGroup as i32,
            title: "Media User Token".into(),
            id: "media_user_token".into(),
            subtitle: "Apple Music account media-user-token cookie".into(),
            ..Default::default()
        }],
    });
    info!("Initialized Apple Music extension");
}

fn main() {}
