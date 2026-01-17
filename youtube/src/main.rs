use std::sync::Mutex;

use futures::executor::block_on;
use moosync_edk::MoosyncResult as Result;
use moosync_edk::{
    Album, Artist, ExtensionProviderScope, MoosyncResult, Playlist, PreferenceData,
    PreferenceTypes, PreferenceUiData, SearchResult, Song,
    api::{
        Accounts, ContextMenu, CustomRequestReturnType, DatabaseEvents, Extension, PlayerEvents,
        PreferenceEvents, Provider, SongsWithPageTokenReturnType,
    },
    handler::register_extension,
    info,
};

use crate::{
    api::YoutubeAuth,
    utils::{sanitize_id, to_moosync_string_error},
};

mod api;
mod no_api;
mod utils;

struct YoutubeExtension {
    api_client: Mutex<Option<YoutubeAuth>>,
}

fn register_prefs() -> MoosyncResult<()> {
    moosync_edk::api::extension_api::register_user_preferences(vec![
        PreferenceUiData {
            r#type: PreferenceTypes::EditText.into(),
            title: "Client ID".into(),
            key: "client_id".into(),
            description: "Client ID for Youtube".into(),
            mobile: Some(true),
            ..Default::default()
        },
        PreferenceUiData {
            r#type: PreferenceTypes::EditText.into(),
            title: "Client Secret".into(),
            key: "client_secret".into(),
            description: "Client Secret for Youtube".into(),
            mobile: Some(true),
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
    fn get_provider_scopes(&self) -> Result<Vec<ExtensionProviderScope>> {
        Ok(vec![
            ExtensionProviderScope::Playlists,
            ExtensionProviderScope::PlaylistSongs,
            ExtensionProviderScope::Accounts,
        ])
    }

    fn get_playlists(&self, _req: moosync_edk::RequestedPlaylistsRequest) -> Result<Vec<Playlist>> {
        if let Some(api_client) = self.api_client.lock().unwrap().as_ref() {
            if api_client.account_details.logged_in {
                return Ok(
                    block_on(api_client.get_all_playlists()).map_err(to_moosync_string_error)?
                );
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
        if let Some(id) = self.match_id(id) {
            if let Some(api_client) = self.api_client.lock().unwrap().as_ref() {
                if api_client.account_details.logged_in {
                    let (songs, next_page) =
                        block_on(api_client.get_playlist_content(&id, next_page_token))
                            .map_err(to_moosync_string_error)?;
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
        let scraper = no_api::YoutubeScraper::default();
        // Use the generic search type to return videos, playlists and channels
        let res = block_on(scraper.search_yt(req.query, rusty_ytdl::search::SearchType::All))?;
        Ok(res)
    }

    fn get_album_songs(
        &self,
        req: moosync_edk::RequestedAlbumSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let album = req.album.ok_or("No album provided")?;
        let scraper = no_api::YoutubeScraper::default();
        // Fallback: search by album name and return matching videos as songs
        let query = album.album_name.unwrap_or_default();
        let search_res = block_on(scraper.search_yt(query, rusty_ytdl::search::SearchType::Video))?;
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
        let scraper = no_api::YoutubeScraper::default();
        // Prefer artist name; if absent, try to extract id from artist_id
        let query = if let Some(name) = artist.artist_name.clone() {
            name
        } else if let Some(id) = artist.artist_id.clone() {
            // strip known prefix if present
            sanitize_id(&id).to_string()
        } else {
            String::new()
        };

        let search_res = block_on(scraper.search_yt(query, rusty_ytdl::search::SearchType::Video))?;
        Ok(SongsWithPageTokenReturnType {
            songs: search_res.songs,
            next_page_token: None,
        })
    }

    fn get_recommendations(
        &self,
        _req: moosync_edk::RequestedRecommendationsRequest,
    ) -> MoosyncResult<Vec<Song>> {
        let scraper = no_api::YoutubeScraper::default();
        let suggestions = block_on(scraper.get_suggestions())?;
        Ok(suggestions)
    }

    fn get_song_from_url(
        &self,
        _url: moosync_edk::RequestedSongFromUrlRequest,
    ) -> Result<Option<moosync_edk::Song>> {
        Ok(None)
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
