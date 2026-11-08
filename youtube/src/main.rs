use std::sync::Mutex;

use futures::executor::block_on;
use moosync_edk::MoosyncResult as Result;
use moosync_edk::{
    ExtensionProviderScope, MoosyncResult, Playlist, PreferenceData, PreferenceItem, PreferenceType,
    RequestedAlbumSongsResponse, RequestedArtistSongsResponse, RequestedPlaylistSongsResponse,
    RequestedPlaylistsResponse, RequestedRecommendationsResponse, SearchResult, Song,
    api::{
        Accounts, ContextMenu, CustomRequestReturnType, DatabaseEvents, Extension, PlayerEvents,
        PreferenceEvents, Provider, SongsWithPageTokenReturnType,
        extension_api::{get_cache, set_cache, set_cache_with_ttl},
    },
    handler::register_extension,
    info,
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
    moosync_edk::api::extension_api::register_user_preferences(vec![
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
    ])?;

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
        if let Ok(PreferenceData {
            value: client_id,
            key: _,
        }) = moosync_edk::api::extension_api::get_preference(PreferenceData {
            key: "client_id".into(),
            ..Default::default()
        }) {
            if let Ok(PreferenceData {
                value: client_secret,
                key: _,
            }) = moosync_edk::api::extension_api::get_preference(PreferenceData {
                key: "client_secret".into(),
                ..Default::default()
            }) {
                let client_id = client_id.and_then(utils::google_value_to_string);
                let client_secret = client_secret.and_then(utils::google_value_to_string);

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
impl Provider for YoutubeExtension {
    fn get_playlists(&self, _req: moosync_edk::RequestedPlaylistsRequest) -> Result<Vec<Playlist>> {
        let cache_key = cache_keys::user_playlists();
        if let Ok(Some(cached)) = get_cache::<RequestedPlaylistsResponse>(&cache_key) {
            return Ok(cached.playlists);
        }

        if let Some(api_client) = self.api_client.lock().unwrap().as_ref() {
            if api_client.account_details.logged_in {
                let playlists = block_on(api_client.get_all_playlists()).map_err(to_moosync_string_error)?;
                let _ = set_cache_with_ttl(
                    &cache_key,
                    &RequestedPlaylistsResponse {
                        playlists: playlists.clone(),
                    },
                    Some(3600),
                );
                return Ok(playlists);
            }
        }
        Err("Not logged in".into())
    }

    fn get_playlist_content(
        &self,
        req: moosync_edk::RequestedPlaylistSongsRequest,
    ) -> Result<SongsWithPageTokenReturnType> {
        let id = req.id;
        let next_page_token = req.page_token;
        if let Some(clean_id) = self.match_id(id.clone()) {
            let cache_key = cache_keys::playlist_songs(&clean_id, next_page_token.as_deref());
            if let Ok(Some(cached)) = get_cache::<RequestedPlaylistSongsResponse>(&cache_key) {
                return Ok(SongsWithPageTokenReturnType {
                    songs: cached.songs,
                    next_page_token: cached.next_page_token,
                });
            }

            if let Some(api_client) = self.api_client.lock().unwrap().as_ref() {
                if api_client.account_details.logged_in {
                    let (songs, next_page) =
                        block_on(api_client.get_playlist_content(&clean_id, next_page_token))
                            .map_err(to_moosync_string_error)?;
                    let _ = set_cache_with_ttl(
                        &cache_key,
                        &RequestedPlaylistSongsResponse {
                            songs: songs.clone(),
                            next_page_token: next_page.clone(),
                        },
                        Some(3600),
                    );
                    return Ok(SongsWithPageTokenReturnType {
                        songs,
                        next_page_token: next_page,
                    });
                }
            }
        }

        Err("Not logged in or invalid ID".into())
    }

    fn search(
        &self,
        req: moosync_edk::RequestedSearchResultRequest,
    ) -> MoosyncResult<SearchResult> {
        let cache_key = cache_keys::search(&req.query);
        if let Ok(Some(cached)) = get_cache::<SearchResult>(&cache_key) {
            return Ok(cached);
        }

        let scraper = no_api::YoutubeScraper::default();
        // Use the generic search type to return videos, playlists and channels
        let res = block_on(scraper.search_yt(req.query, rusty_ytdl::search::SearchType::All))?;
        let _ = set_cache_with_ttl(&cache_key, &res, Some(7200));
        Ok(res)
    }

    fn get_album_songs(
        &self,
        req: moosync_edk::RequestedAlbumSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let album = req.album.ok_or("No album provided")?;
        let query = album.album_name.unwrap_or_default();
        let cache_key = cache_keys::album_songs(&query);
        if let Ok(Some(cached)) = get_cache::<RequestedAlbumSongsResponse>(&cache_key) {
            return Ok(SongsWithPageTokenReturnType {
                songs: cached.songs,
                next_page_token: cached.next_page_token,
            });
        }

        let scraper = no_api::YoutubeScraper::default();
        // Fallback: search by album name and return matching videos as songs
        let search_res = block_on(scraper.search_yt(query, rusty_ytdl::search::SearchType::Video))?;
        let _ = set_cache(
            &cache_key,
            &RequestedAlbumSongsResponse {
                songs: search_res.songs.clone(),
                next_page_token: None,
            },
        );
        Ok(SongsWithPageTokenReturnType {
            songs: search_res.songs,
            next_page_token: None,
        })
    }

    fn get_artist_songs(
        &self,
        req: moosync_edk::RequestedArtistSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let artist = req.artist.ok_or("No artist provided")?;
        // Prefer artist name; if absent, try to extract id from artist_id
        let query = if let Some(name) = artist.artist_name.clone() {
            name
        } else if let Some(id) = artist.artist_id.clone() {
            // strip known prefix if present
            sanitize_id(&id).to_string()
        } else {
            String::new()
        };

        let cache_key = cache_keys::artist_songs(&query);
        if let Ok(Some(cached)) = get_cache::<RequestedArtistSongsResponse>(&cache_key) {
            return Ok(SongsWithPageTokenReturnType {
                songs: cached.songs,
                next_page_token: cached.next_page_token,
            });
        }

        let scraper = no_api::YoutubeScraper::default();
        let search_res = block_on(scraper.search_yt(query, rusty_ytdl::search::SearchType::Video))?;
        let _ = set_cache_with_ttl(
            &cache_key,
            &RequestedArtistSongsResponse {
                songs: search_res.songs.clone(),
                next_page_token: None,
            },
            Some(86400),
        );
        Ok(SongsWithPageTokenReturnType {
            songs: search_res.songs,
            next_page_token: None,
        })
    }

    fn get_recommendations(
        &self,
        _req: moosync_edk::RequestedRecommendationsRequest,
    ) -> MoosyncResult<Vec<Song>> {
        let cache_key = cache_keys::recommendations();
        if let Ok(Some(cached)) = get_cache::<RequestedRecommendationsResponse>(&cache_key) {
            return Ok(cached.songs);
        }

        let scraper = no_api::YoutubeScraper::default();
        let suggestions = block_on(scraper.get_suggestions())?;
        let _ = set_cache_with_ttl(
            &cache_key,
            &RequestedRecommendationsResponse {
                songs: suggestions.clone(),
            },
            Some(3600),
        );
        Ok(suggestions)
    }

    fn handle_custom_request(
        &self,
        _request: moosync_edk::CustomRequest,
    ) -> moosync_edk::MoosyncResult<CustomRequestReturnType> {
        Ok(CustomRequestReturnType {
            mime_type: None,
            data: None,
            redirect_url: None,
        })
    }
}
impl Extension for YoutubeExtension {}

// SAFETY: there is no other global function of this name
#[unsafe(no_mangle)]
pub extern "C" fn init() {
    info!("Initializing YoutubeExtension");

    let extension = YoutubeExtension::new();
    register_extension(Box::new(extension)).unwrap();

    register_prefs().unwrap();

    info!("Initialized YoutubeExtension");
}

fn main() {}
