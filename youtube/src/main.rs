use std::sync::Mutex;

use futures::executor::block_on;
use moosync_edk::{
    api::{
        get_preference, register_user_preference, Accounts, ContextMenu, DatabaseEvents, Extension,
        PlayerEvents, PlayerProvider, PreferenceEvents, Provider,
    },
    cache::{get_cache, set_cache, set_cache_with_ttl},
    handler::register_extension,
    info,
    types::{
        CustomRequest, CustomRequestResponse, GetPreferenceRequest, Playlist, PreferenceData,
        PreferenceItem, PreferenceType, RegisterUserPreferenceRequest, RequestedAlbumSongsRequest,
        RequestedAlbumSongsResponse, RequestedArtistSongsRequest, RequestedArtistSongsResponse,
        RequestedPlaylistSongsRequest, RequestedPlaylistSongsResponse, RequestedPlaylistsRequest,
        RequestedPlaylistsResponse, RequestedRecommendationsRequest,
        RequestedRecommendationsResponse, RequestedSearchResultRequest,
        RequestedSearchResultResponse, SearchResult, Song,
    },
    MoosyncResult,
};

use crate::{
    api::YoutubeAuth,
    utils::{sanitize_id, to_moosync_string_error},
};

mod api;
mod cache_keys;
mod no_api;
mod utils;

struct YoutubeExtension {
    api_client: Mutex<Option<YoutubeAuth>>,
}

fn register_prefs() -> MoosyncResult<()> {
    register_user_preference(RegisterUserPreferenceRequest {
        prefs: vec![
            PreferenceItem {
                pref_type: PreferenceType::TextInputGroup as i32,
                title: "Client ID".into(),
                id: "client_id".into(),
                subtitle: "Client ID for Youtube".into(),
                ..Default::default()
            },
            PreferenceItem {
                pref_type: PreferenceType::TextInputGroup as i32,
                title: "Client Secret".into(),
                id: "client_secret".into(),
                subtitle: "Client Secret for Youtube".into(),
                ..Default::default()
            },
        ],
    })?;

    Ok(())
}

impl YoutubeExtension {
    pub fn new() -> Self {
        let s = Self {
            api_client: Mutex::new(None),
        };

        s.create_api_instance();
        s
    }

    fn create_api_instance(&self) {
        if let Ok(resp_id) = get_preference(GetPreferenceRequest {
            data: Some(PreferenceData {
                key: "client_id".into(),
                ..Default::default()
            }),
        }) {
            if let Ok(resp_secret) = get_preference(GetPreferenceRequest {
                data: Some(PreferenceData {
                    key: "client_secret".into(),
                    ..Default::default()
                }),
            }) {
                let client_id = resp_id.data.and_then(|d| d.value).and_then(utils::google_value_to_string);
                let client_secret = resp_secret.data.and_then(|d| d.value).and_then(utils::google_value_to_string);

                if let Some(client_id) = client_id {
                    if let Some(client_secret) = client_secret {
                        info!("Creating api client");
                        let mut api_client = YoutubeAuth::new(
                            &client_id,
                            &client_secret,
                            "https://moosync.app/youtube",
                        );
                        if let Err(e) = block_on(api_client.load_tokens()) {
                            moosync_edk::error!("Failed to load existing tokens: {:?}", e);
                        }

                        self.api_client.lock().unwrap().replace(api_client);
                    }
                }
            }
        }
    }

    fn match_id(&self, id: String) -> Option<String> {
        if id.starts_with("moosync.youtube:") {
            let new_id = id.replace("moosync.youtube:", "");
            if new_id.starts_with("youtube-playlist:")
                || new_id.starts_with("youtube-artist:")
                || new_id.starts_with("youtube-album:")
                || new_id.starts_with("youtube:")
            {
                return Some(new_id);
            }
        }

        return None;
    }
}

impl ContextMenu for YoutubeExtension {}
impl Accounts for YoutubeExtension {}
impl DatabaseEvents for YoutubeExtension {}
impl PreferenceEvents for YoutubeExtension {}
impl PlayerEvents for YoutubeExtension {}
impl PlayerProvider for YoutubeExtension {}

impl Provider for YoutubeExtension {
    fn requested_playlists(&self, _req: RequestedPlaylistsRequest) -> MoosyncResult<RequestedPlaylistsResponse> {
        let cache_key = cache_keys::user_playlists();
        if let Ok(Some(cached)) = get_cache::<RequestedPlaylistsResponse>(&cache_key) {
            return Ok(cached);
        }

        if let Some(api_client) = self.api_client.lock().unwrap().as_ref() {
            if api_client.account_details.logged_in {
                let playlists = block_on(api_client.get_all_playlists()).map_err(to_moosync_string_error)?;
                let res = RequestedPlaylistsResponse {
                    playlists,
                };
                let _ = set_cache_with_ttl(
                    &cache_key,
                    &res,
                    Some(3600),
                );
                return Ok(res);
            }
        }
        Err("Not logged in".into())
    }

    fn requested_playlist_songs(
        &self,
        req: RequestedPlaylistSongsRequest,
    ) -> MoosyncResult<RequestedPlaylistSongsResponse> {
        let id = req.id;
        let next_page_token = req.page_token;
        if let Some(clean_id) = self.match_id(id.clone()) {
            let cache_key = cache_keys::playlist_songs(&clean_id, next_page_token.as_deref());
            if let Ok(Some(cached)) = get_cache::<RequestedPlaylistSongsResponse>(&cache_key) {
                return Ok(cached);
            }

            if let Some(api_client) = self.api_client.lock().unwrap().as_ref() {
                if api_client.account_details.logged_in {
                    let (songs, next_page) =
                        block_on(api_client.get_playlist_content(&clean_id, next_page_token))
                            .map_err(to_moosync_string_error)?;
                    let res = RequestedPlaylistSongsResponse {
                        songs,
                        next_page_token: next_page,
                    };
                    let _ = set_cache_with_ttl(
                        &cache_key,
                        &res,
                        Some(3600),
                    );
                    return Ok(res);
                }
            }
        }

        Err("Not logged in or invalid ID".into())
    }

    fn requested_search_result(
        &self,
        req: RequestedSearchResultRequest,
    ) -> MoosyncResult<RequestedSearchResultResponse> {
        let cache_key = cache_keys::search(&req.query);
        if let Ok(Some(cached)) = get_cache::<RequestedSearchResultResponse>(&cache_key) {
            return Ok(cached);
        }

        let scraper = no_api::YoutubeScraper::default();
        let res = block_on(scraper.search_yt(req.query, rusty_ytdl::search::SearchType::All))?;
        let resp = RequestedSearchResultResponse {
            songs: res.songs,
            albums: res.albums,
            artists: res.artists,
            playlists: res.playlists,
        };
        let _ = set_cache_with_ttl(&cache_key, &resp, Some(7200));
        Ok(resp)
    }

    fn requested_album_songs(
        &self,
        req: RequestedAlbumSongsRequest,
    ) -> MoosyncResult<RequestedAlbumSongsResponse> {
        let album = req.album.ok_or("No album provided")?;
        let query = album.album_name.unwrap_or_default();
        let cache_key = cache_keys::album_songs(&query);
        if let Ok(Some(cached)) = get_cache::<RequestedAlbumSongsResponse>(&cache_key) {
            return Ok(cached);
        }

        let scraper = no_api::YoutubeScraper::default();
        let search_res = block_on(scraper.search_yt(query, rusty_ytdl::search::SearchType::Video))?;
        let res = RequestedAlbumSongsResponse {
            songs: search_res.songs,
            next_page_token: None,
        };
        let _ = set_cache(&cache_key, &res);
        Ok(res)
    }

    fn requested_artist_songs(
        &self,
        req: RequestedArtistSongsRequest,
    ) -> MoosyncResult<RequestedArtistSongsResponse> {
        let artist = req.artist.ok_or("No artist provided")?;
        let query = if let Some(name) = artist.artist_name.clone() {
            name
        } else if let Some(id) = artist.artist_id.clone() {
            sanitize_id(&id).to_string()
        } else {
            String::new()
        };

        let cache_key = cache_keys::artist_songs(&query);
        if let Ok(Some(cached)) = get_cache::<RequestedArtistSongsResponse>(&cache_key) {
            return Ok(cached);
        }

        let scraper = no_api::YoutubeScraper::default();
        let search_res = block_on(scraper.search_yt(query, rusty_ytdl::search::SearchType::Video))?;
        let res = RequestedArtistSongsResponse {
            songs: search_res.songs,
            next_page_token: None,
        };
        let _ = set_cache_with_ttl(&cache_key, &res, Some(86400));
        Ok(res)
    }

    fn requested_recommendations(
        &self,
        _req: RequestedRecommendationsRequest,
    ) -> MoosyncResult<RequestedRecommendationsResponse> {
        let cache_key = cache_keys::recommendations();
        if let Ok(Some(cached)) = get_cache::<RequestedRecommendationsResponse>(&cache_key) {
            return Ok(cached);
        }

        let scraper = no_api::YoutubeScraper::default();
        let suggestions = block_on(scraper.get_suggestions())?;
        let res = RequestedRecommendationsResponse {
            songs: suggestions,
        };
        let _ = set_cache_with_ttl(&cache_key, &res, Some(3600));
        Ok(res)
    }

    fn custom_request(
        &self,
        _request: CustomRequest,
    ) -> MoosyncResult<CustomRequestResponse> {
        Ok(CustomRequestResponse {
            mime_type: None,
            data: None,
            redirect_url: None,
        })
    }
}

impl Extension for YoutubeExtension {}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    info!("Initializing YoutubeExtension");

    let extension = YoutubeExtension::new();
    register_extension(Box::new(extension)).unwrap();

    register_prefs().unwrap();

    info!("Initialized YoutubeExtension");
}

fn main() {}
