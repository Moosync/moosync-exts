use std::sync::Mutex;

use futures::executor::block_on;
use moosync_edk::{
    AccountLoginArgs, ExtensionProviderScope, PreferenceArgs, PreferenceData, PreferenceTypes, PreferenceUIData, QueryableAlbum, QueryableArtist, QueryablePlaylist, Result, Result as MoosyncResult, SearchResult, Song, SongsWithPageTokenReturnType, api::{
        Accounts, ContextMenu, DatabaseEvents, Extension, PlayerEvents, PreferenceEvents, Provider,
    }, handler::register_extension, info
};
use serde_json::Value;

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
        PreferenceUIData {
            _type: PreferenceTypes::EditText,
            title: "Client ID".into(),
            key: "client_id".into(),
            description: "Client ID for Youtube".into(),
            mobile: Some(true),
            ..Default::default()
        },
        PreferenceUIData {
            _type: PreferenceTypes::EditText,
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
            default_value: _,
        }) = moosync_edk::api::extension_api::get_preference(PreferenceData {
            key: "client_id".into(),
            ..Default::default()
        }) {
            if let Ok(PreferenceData {
                value: client_secret,
                key: _,
                default_value: _,
            }) = moosync_edk::api::extension_api::get_preference(PreferenceData {
                key: "client_secret".into(),
                ..Default::default()
            }) {
                if let Some(Value::String(client_id)) = client_id {
                    if let Some(Value::String(client_secret)) = client_secret {
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

impl PlayerEvents for YoutubeExtension {}
impl Provider for YoutubeExtension {
    fn get_provider_scopes(&self) -> Result<Vec<ExtensionProviderScope>> {
        Ok(vec![
            ExtensionProviderScope::Playlists,
            ExtensionProviderScope::PlaylistSongs,
            ExtensionProviderScope::Accounts,
        ])
    }

    fn get_playlists(&self) -> Result<Vec<QueryablePlaylist>> {
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
        id: String,
        next_page_token: Option<String>,
    ) -> Result<SongsWithPageTokenReturnType> {
        if let Some(id) = self.match_id(id) {
            if let Some(api_client) = self.api_client.lock().unwrap().as_ref() {
                if api_client.account_details.logged_in {
                    let (songs, next_page) =
                        block_on(api_client.get_playlist_content(&id, next_page_token))
                            .map_err(to_moosync_string_error)?;
                    return Ok(SongsWithPageTokenReturnType {
                        songs,
                        next_page_token: next_page.map(|n| serde_json::to_value(n).unwrap()),
                    });
                }
            }
        }

        Err("Not logged in or invalid ID".into())
    }

    fn search(&self, term: String) -> MoosyncResult<SearchResult> {
        let scraper = no_api::YoutubeScraper::default();
        // Use the generic search type to return videos, playlists and channels
        let res = block_on(scraper.search_yt(term, rusty_ytdl::search::SearchType::All))?;
        Ok(res)
    }

    fn get_album_songs(
        &self,
        album: QueryableAlbum,
        _next_page_token: Option<String>,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
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
        artist: QueryableArtist,
        _next_page_token: Option<String>,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
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

    fn get_recommendations(&self) -> MoosyncResult<Vec<Song>> {
        let scraper = no_api::YoutubeScraper::default();
        let suggestions = block_on(scraper.get_suggestions())?;
        Ok(suggestions)
    }
}

impl DatabaseEvents for YoutubeExtension {}
impl PreferenceEvents for YoutubeExtension {
    fn on_preferences_changed(&self, args: PreferenceArgs) -> MoosyncResult<()> {
        if args.key == "client_id" || args.key == "client_secret" {
            self.create_api_instance();
        }

        Ok(())
    }
}
impl ContextMenu for YoutubeExtension {}
impl Accounts for YoutubeExtension {
    fn get_accounts(&self) -> Result<Vec<moosync_edk::ExtensionAccountDetail>> {
        if let Some(api_client) = self.api_client.lock().unwrap().as_ref() {
            info!("Got account details {:?}", api_client.account_details);
            return Ok(vec![api_client.account_details.clone()]);
        }
        Err("Not logged in".into())
    }

    fn perform_account_login(&self, args: AccountLoginArgs) -> Result<String> {
        if let Some(api_client) = self.api_client.lock().unwrap().as_mut() {
            if args.login_status {
                return Ok(api_client.get_authorization_url("test_state"));
            } else {
                api_client.logout().map_err(to_moosync_string_error)?;
                moosync_edk::api::extension_api::update_accounts(None)?;
                return Ok("".into());
            }
        }
        Err("No Client ID or secret".into())
    }

    fn oauth_callback(&self, code: String) -> Result<()> {
        if let Some(api_client) = self.api_client.lock().unwrap().as_mut() {
            block_on(api_client.exchange_code_for_token(&code)).map_err(to_moosync_string_error)?;
            moosync_edk::api::extension_api::update_accounts(None)?;
            Ok(())
        } else {
            Err("Not logged in".into())
        }
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
