use std::sync::{Arc, Mutex};

use crate::{
    api::{ApiClient, SpotifyConfig, TokenHolder},
    soloist::SoloistPlayer,
    utils::{Pagination, cast_prefdata, parse_next_page_token, serde_to_google_value},
};
use futures::executor::block_on;
use moosync_edk::{
    api::{
        get_preference, get_secure, register_user_preference, set_account, set_secure, Accounts,
        ContextMenu, DatabaseEvents, Extension, PlayerEvents, PlayerProvider, PreferenceEvents,
        Provider,
    },
    cache::{get_cache, set_cache, set_cache_with_ttl},
    error,
    handler::register_extension,
    info,
    types::{
        preference_value, CanPlayRequest, CanPlayResponse, CustomRequest, CustomRequestResponse,
        ExtensionAccountDetail, GetPreferenceRequest, GetSecureRequest, OauthCallbackRequest,
        OauthCallbackResponse, PerformAccountLoginRequest, PerformAccountLoginResponse,
        PlayerLoadTrackRequest, PlayerLoadTrackResponse, PlayerPauseRequest, PlayerPauseResponse,
        PlayerPlayRequest, PlayerPlayResponse, PlayerSeekRequest, PlayerSeekResponse,
        PlayerSetVolumeRequest, PlayerSetVolumeResponse, PlayerStopRequest, PlayerStopResponse,
        Playlist, PreferenceChangedRequest, PreferenceData, PreferenceItem, PreferenceType,
        PreferenceValue, RegisterUserPreferenceRequest, RequestedAlbumSongsRequest,
        RequestedAlbumSongsResponse, RequestedArtistSongsRequest, RequestedArtistSongsResponse,
        RequestedPlaylistSongsRequest, RequestedPlaylistSongsResponse, RequestedPlaylistsRequest,
        RequestedPlaylistsResponse, RequestedRecommendationsRequest,
        RequestedRecommendationsResponse, RequestedSearchResultRequest,
        RequestedSearchResultResponse, RequestedSongFromIdRequest, RequestedSongFromIdResponse,
        SetAccountRequest, SetSecureRequest, Song,
    },
    warn, MoosyncResult,
};

mod api;
mod cache_keys;
mod soloist;
mod utils;

struct SpotifyExtension {
    api_client: Mutex<Option<ApiClient>>,
    soloist_player: Arc<SoloistPlayer>,
}

fn register_prefs() -> MoosyncResult<()> {
    register_user_preference(RegisterUserPreferenceRequest {
        prefs: vec![
            PreferenceItem {
                pref_type: PreferenceType::TextInputGroup as i32,
                title: "Client ID".into(),
                id: "client_id".into(),
                subtitle: "Client ID for Spotify".into(),
                ..Default::default()
            },
            PreferenceItem {
                pref_type: PreferenceType::TextInputGroup as i32,
                title: "Client Secret".into(),
                id: "client_secret".into(),
                subtitle: "Client Secret for Spotify".into(),
                ..Default::default()
            },
            PreferenceItem {
                pref_type: PreferenceType::TextInputGroup as i32,
                title: "Soloist API Key".into(),
                id: "soloist_api_key".into(),
                subtitle: "Spotify Soloist API Key for playback".into(),
                ..Default::default()
            },
            PreferenceItem {
                pref_type: PreferenceType::TextInputGroup as i32,
                title: "Soloist WebSocket Port".into(),
                id: "soloist_ws_port".into(),
                subtitle: "WebSocket port for Spotify Soloist (default: 9090)".into(),
                default: Some(PreferenceValue {
                    value: Some(preference_value::Value::StringValue("9090".into())),
                }),
                ..Default::default()
            },
        ],
    })?;

    Ok(())
}

impl SpotifyExtension {
    pub fn new() -> Self {
        let soloist_player = Arc::new(SoloistPlayer::new());
        let mut client = Self {
            api_client: Mutex::new(None),
            soloist_player,
        };
        client.create_client_if_possible();
        client.set_client_tokens();
        client.init_soloist_port();
        client
    }

    fn init_soloist_port(&self) {
        let port = self.get_soloist_port();
        self.soloist_player.set_port(port);
    }

    fn create_client_if_possible(&mut self) {
        let config = self.get_config();
        match config {
            Ok(config) => {
                self.api_client = Mutex::new(Some(ApiClient::new(config)));
            }
            Err(e) => error!("Could not create api client: {}", e),
        }
    }

    fn set_client_tokens(&self) {
        let tokens = self.get_tokens();
        match tokens {
            Ok(tokens) => {
                let mut api_client = self.api_client.lock().unwrap();
                if let Some(api_client) = api_client.as_mut() {
                    block_on(api_client.set_tokens(tokens));
                }
            }
            Err(e) => warn!(
                "No auth tokens set. This is fine if you wern't logged in before. {}",
                e
            ),
        }
    }

    fn get_config(&self) -> MoosyncResult<SpotifyConfig> {
        let resp_secret = get_secure(GetSecureRequest {
            data: Some(PreferenceData {
                key: "client_secret".into(),
                ..Default::default()
            }),
        })?;
        let client_secret: Option<String> = resp_secret.data.and_then(|d| cast_prefdata(d).ok().flatten());

        let resp_id = get_secure(GetSecureRequest {
            data: Some(PreferenceData {
                key: "client_id".into(),
                ..Default::default()
            }),
        })?;
        let client_id: Option<String> = resp_id.data.and_then(|d| cast_prefdata(d).ok().flatten());

        info!("Got config {:?} {:?}", client_secret, client_id);

        if let (Some(client_secret), Some(client_id)) = (client_secret, client_id) {
            return Ok(SpotifyConfig {
                client_secret,
                client_id,
                redirect_uri: "https://moosync.app/spotify",
                scopes: vec![
                    "playlist-read-private",
                    "user-top-read",
                    "user-library-read",
                    "user-read-private",
                ],
            });
        }

        Err("Config not found".into())
    }

    fn get_tokens(&self) -> MoosyncResult<Option<TokenHolder>> {
        let resp = get_secure(GetSecureRequest {
            data: Some(PreferenceData {
                key: "tokens".into(),
                ..Default::default()
            }),
        })?;
        let tokens: Option<TokenHolder> = resp.data.and_then(|d| cast_prefdata(d).ok().flatten());

        Ok(tokens)
    }

    fn get_soloist_api_key(&self) -> String {
        let secure_resp = get_secure(GetSecureRequest {
            data: Some(PreferenceData {
                key: "soloist_api_key".into(),
                ..Default::default()
            }),
        });
        if let Ok(resp) = secure_resp {
            if let Some(key) = resp.data.and_then(|d| cast_prefdata::<String>(d).ok().flatten()) {
                if !key.is_empty() {
                    return key;
                }
            }
        }

        let resp = get_preference(GetPreferenceRequest {
            data: Some(PreferenceData {
                key: "soloist_api_key".into(),
                ..Default::default()
            }),
        });
        resp.ok()
            .and_then(|r| r.data)
            .and_then(|d| cast_prefdata::<String>(d).ok().flatten())
            .unwrap_or_default()
    }

    fn get_soloist_port(&self) -> u16 {
        let resp = get_preference(GetPreferenceRequest {
            data: Some(PreferenceData {
                key: "soloist_ws_port".into(),
                ..Default::default()
            }),
        });
        resp.ok()
            .and_then(|r| r.data)
            .and_then(|d| cast_prefdata::<String>(d).ok().flatten())
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(9090)
    }

    fn is_premium(&self) -> bool {
        let api_client = self.api_client.lock().unwrap();
        api_client.as_ref().map(|c| c.is_premium()).unwrap_or(false)
    }
}

impl PlayerEvents for SpotifyExtension {}

impl PlayerProvider for SpotifyExtension {
    fn can_play(&self, req: CanPlayRequest) -> MoosyncResult<CanPlayResponse> {
        info!("can_play called for src: {}", req.src);
        let is_spotify = req.src.starts_with("moosync.spotify:")
            || req.src.starts_with("spotify:track:")
            || req
                .song
                .as_ref()
                .and_then(|s| s.song.as_ref())
                .and_then(|i| i.id.as_deref())
                .map(|id| id.starts_with("moosync.spotify:"))
                .unwrap_or(false);

        if !is_spotify {
            info!("can_play: not a spotify track");
            return Ok(CanPlayResponse { can_play: false });
        }

        let is_premium = self.is_premium();
        if !is_premium {
            return Ok(CanPlayResponse { can_play: false });
        }

        if !self.soloist_player.is_paired() {
            let api_key = self.get_soloist_api_key();
            let paired = self.soloist_player.poll_pairing_status().unwrap_or(false);
            if !paired {
                let _ = self.soloist_player.start_pairing_mode(is_premium, &api_key);
            }
            return Ok(CanPlayResponse { can_play: paired });
        }

        Ok(CanPlayResponse { can_play: true })
    }

    fn player_load_track(
        &self,
        req: PlayerLoadTrackRequest,
    ) -> MoosyncResult<PlayerLoadTrackResponse> {
        let is_premium = self.is_premium();
        let api_key = self.get_soloist_api_key();
        self.soloist_player.start_soloist(is_premium, &api_key)?;
        self.soloist_player
            .load_track(&req.src, req.song.as_ref())?;
        Ok(PlayerLoadTrackResponse {})
    }

    fn player_play(&self, _req: PlayerPlayRequest) -> MoosyncResult<PlayerPlayResponse> {
        self.soloist_player.play()?;
        Ok(PlayerPlayResponse {})
    }

    fn player_pause(&self, _req: PlayerPauseRequest) -> MoosyncResult<PlayerPauseResponse> {
        self.soloist_player.pause()?;
        Ok(PlayerPauseResponse {})
    }

    fn player_stop(&self, _req: PlayerStopRequest) -> MoosyncResult<PlayerStopResponse> {
        self.soloist_player.stop()?;
        Ok(PlayerStopResponse {})
    }

    fn player_seek(&self, req: PlayerSeekRequest) -> MoosyncResult<PlayerSeekResponse> {
        self.soloist_player.seek(req.position)?;
        Ok(PlayerSeekResponse {})
    }

    fn player_set_volume(
        &self,
        req: PlayerSetVolumeRequest,
    ) -> MoosyncResult<PlayerSetVolumeResponse> {
        self.soloist_player.set_volume(req.volume)?;
        Ok(PlayerSetVolumeResponse {})
    }
}

impl Provider for SpotifyExtension {
    fn requested_album_songs(
        &self,
        req: RequestedAlbumSongsRequest,
    ) -> MoosyncResult<RequestedAlbumSongsResponse> {
        let album = req.album.unwrap();
        let next_page_token = req.page_token;
        let album_id = album.album_id.as_deref().unwrap_or_default();
        let cache_key = cache_keys::album_songs(album_id, next_page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedAlbumSongsResponse>(&cache_key) {
            return Ok(cached);
        }

        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let (songs, token) = block_on(
                api_client.get_album_content(album, parse_next_page_token(next_page_token)),
            )?;

            let token_str: Option<String> = token.try_into().ok();
            let res = RequestedAlbumSongsResponse {
                songs,
                next_page_token: token_str,
            };
            let _ = set_cache(&cache_key, &res);
            return Ok(res);
        }

        Err("Not logged in".into())
    }

    fn requested_playlist_songs(
        &self,
        req: RequestedPlaylistSongsRequest,
    ) -> MoosyncResult<RequestedPlaylistSongsResponse> {
        let id = req.id;
        let next_page_token = req.page_token;
        let cache_key = cache_keys::playlist_songs(&id, next_page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedPlaylistSongsResponse>(&cache_key) {
            return Ok(cached);
        }

        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let playlist = Playlist {
                playlist_id: Some(id),
                ..Default::default()
            };

            let (songs, token) = block_on(
                api_client.get_playlist_content(playlist, parse_next_page_token(next_page_token)),
            )?;

            let token_str: Option<String> = token.try_into().ok();
            let res = RequestedPlaylistSongsResponse {
                songs,
                next_page_token: token_str,
            };
            let _ = set_cache_with_ttl(&cache_key, &res, Some(3600));

            return Ok(res);
        }
        Err("Not logged in".into())
    }

    fn requested_artist_songs(
        &self,
        req: RequestedArtistSongsRequest,
    ) -> MoosyncResult<RequestedArtistSongsResponse> {
        let artist = req.artist.unwrap();
        let next_page_token = req.page_token;
        let artist_id = artist.artist_id.as_deref().unwrap_or_default();
        let cache_key = cache_keys::artist_songs(artist_id, next_page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedArtistSongsResponse>(&cache_key) {
            return Ok(cached);
        }

        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let (songs, token) = block_on(
                api_client.get_artist_content(artist, parse_next_page_token(next_page_token)),
            )?;

            let token_str: Option<String> = token.try_into().ok();
            let res = RequestedArtistSongsResponse {
                songs,
                next_page_token: token_str,
            };
            let _ = set_cache_with_ttl(&cache_key, &res, Some(86400));

            return Ok(res);
        }
        Err("Not logged in".into())
    }

    fn requested_playlists(&self, _: RequestedPlaylistsRequest) -> MoosyncResult<RequestedPlaylistsResponse> {
        let cache_key = cache_keys::user_playlists();
        if let Ok(Some(cached)) = get_cache::<RequestedPlaylistsResponse>(&cache_key) {
            return Ok(cached);
        }

        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let mut all_playlists = vec![];
            let mut pagination = Pagination::default();

            loop {
                let (mut playlists, next_page) =
                    block_on(api_client.fetch_user_playlists(pagination))?;

                let fetched_count = playlists.len();
                all_playlists.append(&mut playlists);

                if fetched_count == 0 || fetched_count < next_page.limit as usize {
                    break;
                }

                pagination = next_page;
            }

            let res = RequestedPlaylistsResponse {
                playlists: all_playlists,
            };
            let _ = set_cache_with_ttl(&cache_key, &res, Some(3600));

            return Ok(res);
        }
        Err("Not logged in".into())
    }

    fn requested_song_from_id(&self, req: RequestedSongFromIdRequest) -> MoosyncResult<RequestedSongFromIdResponse> {
        let id = req.id;
        let cache_key = cache_keys::song(&id);
        if let Ok(Some(cached)) = get_cache::<Song>(&cache_key) {
            return Ok(RequestedSongFromIdResponse {
                song: Some(cached),
            });
        }

        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let song = block_on(api_client.song_from_id(id))?;
            let _ = set_cache(&cache_key, &song);
            return Ok(RequestedSongFromIdResponse {
                song: Some(song),
            });
        }
        Err("Not logged in".into())
    }

    fn requested_search_result(&self, req: RequestedSearchResultRequest) -> MoosyncResult<RequestedSearchResultResponse> {
        let term = req.query;
        let cache_key = cache_keys::search(&term);
        if let Ok(Some(cached)) = get_cache::<RequestedSearchResultResponse>(&cache_key) {
            return Ok(cached);
        }

        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let results = block_on(api_client.search(term))?;
            let res = RequestedSearchResultResponse {
                songs: results.songs,
                albums: results.albums,
                artists: results.artists,
                playlists: results.playlists,
            };
            let _ = set_cache_with_ttl(&cache_key, &res, Some(7200));
            return Ok(res);
        }
        Err("Not logged in".into())
    }

    fn requested_recommendations(&self, _: RequestedRecommendationsRequest) -> MoosyncResult<RequestedRecommendationsResponse> {
        let cache_key = cache_keys::recommendations();
        if let Ok(Some(cached)) = get_cache::<RequestedRecommendationsResponse>(&cache_key) {
            return Ok(cached);
        }

        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let songs = block_on(api_client.get_suggestions())?;
            let res = RequestedRecommendationsResponse {
                songs,
            };
            let _ = set_cache_with_ttl(&cache_key, &res, Some(3600));
            return Ok(res);
        }
        Err("Not logged in".into())
    }

    fn custom_request(&self, _: CustomRequest) -> MoosyncResult<CustomRequestResponse> {
        Err("Not implemented".into())
    }
}

impl DatabaseEvents for SpotifyExtension {}
impl PreferenceEvents for SpotifyExtension {
    fn preference_changed(&self, req: PreferenceChangedRequest) -> MoosyncResult<()> {
        if let Some(pref) = req.preference {
            if pref.key == "soloist_ws_port" {
                let port = self.get_soloist_port();
                self.soloist_player.set_port(port);
            }
        }
        Ok(())
    }
}
impl Extension for SpotifyExtension {}
impl ContextMenu for SpotifyExtension {}

impl SpotifyExtension {
    fn sync_account(&self) {
        let mut api_client = self.api_client.lock().unwrap();
        let mut username = None;
        let mut logged_in = false;
        if let Some(api_client) = api_client.as_mut() {
            logged_in = true;
            username = block_on(api_client.get_username()).ok().flatten();
        }

        let _ = set_account(SetAccountRequest {
            account: Some(ExtensionAccountDetail {
                id: "spotify".into(),
                package_name: "moosync.spotify".into(),
                name: "Spotify".into(),
                bg_color: "#07C330".into(),
                logged_in,
                icon: "".into(),
                username,
            }),
        });
    }
}

impl Accounts for SpotifyExtension {
    fn perform_account_login(&self, args: PerformAccountLoginRequest) -> MoosyncResult<PerformAccountLoginResponse> {
        let mut guard = self.api_client.lock().unwrap();
        if let Some(api_client) = guard.as_mut() {
            if args.login_status {
                let status = api_client.login()?;
                return Ok(PerformAccountLoginResponse { status });
            } else {
                api_client.logout();
                drop(guard);
                self.sync_account();
                return Ok(PerformAccountLoginResponse {
                    status: "".into(),
                });
            }
        }
        Err("Missing client credentials".into())
    }

    fn oauth_callback(&self, req: OauthCallbackRequest) -> MoosyncResult<OauthCallbackResponse> {
        let mut guard = self.api_client.lock().unwrap();
        if let Some(api_client) = guard.as_mut() {
            block_on(api_client.authorize(req.callback_uri))?;

            let tokens = api_client.get_tokens();
            if let Err(e) = set_secure(SetSecureRequest {
                data: Some(PreferenceData {
                    key: "tokens".into(),
                    value: tokens.map(|t| serde_to_google_value(serde_json::to_value(t).unwrap())),
                    ..Default::default()
                }),
            }) {
                error!("Failed to set tokens in secure storage: {}", e);
            };
            drop(guard);
            self.sync_account();
            return Ok(OauthCallbackResponse {});
        }
        Err("Missing client credentials".into())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    info!("Initializing spotify extension");

    let extension = SpotifyExtension::new();
    extension.sync_account();

    register_extension(Box::new(extension)).unwrap();
    register_prefs().unwrap();

    info!("Initialized spotify extension");
}

fn main() {}
