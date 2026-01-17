use std::{f32::consts::E, sync::Mutex};

use crate::{
    api::{ApiClient, SpotifyConfig, TokenHolder},
    utils::{Pagination, cast_prefdata, parse_next_page_token, serde_to_google_value},
};
use futures::executor::block_on;
use moosync_edk::{
    Album, Artist, MoosyncResult, Playlist, PreferenceData, PreferenceTypes, PreferenceUiData,
    SearchResult, Song,
    api::{
        AccountLoginArgs, Accounts, ContextMenu, CustomRequest, CustomRequestReturnType,
        DatabaseEvents, Extension, ExtensionAccountDetail, ExtensionProviderScope,
        OauthCallbackRequest, PlayerEvents, PreferenceEvents, Provider, RequestedAlbumSongsRequest,
        RequestedArtistSongsRequest, RequestedPlaylistFromUrlRequest,
        RequestedPlaylistSongsRequest, RequestedPlaylistsRequest, RequestedRecommendationsRequest,
        RequestedSearchResultRequest, RequestedSongFromIdRequest, RequestedSongFromUrlRequest,
        SongsWithPageTokenReturnType,
        extension_api::{get_secure, set_secure},
    },
    error,
    handler::register_extension,
    info, warn,
};

mod api;
mod utils;

struct SpotifyExtension {
    api_client: Mutex<Option<ApiClient>>,
}

fn register_prefs() -> MoosyncResult<()> {
    moosync_edk::api::extension_api::register_user_preferences(vec![
        PreferenceUiData {
            r#type: PreferenceTypes::EditText.into(),
            title: "Client ID".into(),
            key: "client_id".into(),
            description: "Client ID for Spotify".into(),
            mobile: Some(true),
            ..Default::default()
        },
        PreferenceUiData {
            r#type: PreferenceTypes::EditText.into(),
            title: "Client Secret".into(),
            key: "client_secret".into(),
            description: "Client Secret for Spotify".into(),
            mobile: Some(true),
            ..Default::default()
        },
    ])?;

    Ok(())
}

impl SpotifyExtension {
    pub fn new() -> Self {
        let mut client = Self {
            api_client: Mutex::new(None),
        };
        client.create_client_if_possible();
        client.set_client_tokens();
        client
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
        let client_secret: Option<String> = cast_prefdata(get_secure(PreferenceData {
            key: "client_secret".into(),
            ..Default::default()
        })?)?;
        let client_id: Option<String> = cast_prefdata(get_secure(PreferenceData {
            key: "client_id".into(),
            ..Default::default()
        })?)?;

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
        let tokens: Option<TokenHolder> = cast_prefdata(get_secure(PreferenceData {
            key: "tokens".into(),
            ..Default::default()
        })?)?;

        Ok(tokens)
    }
}

impl PlayerEvents for SpotifyExtension {}

impl Provider for SpotifyExtension {
    fn get_provider_scopes(&self) -> MoosyncResult<Vec<ExtensionProviderScope>> {
        Ok(vec![])
    }

    fn get_album_songs(
        &self,
        req: RequestedAlbumSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let album = req.album.unwrap();
        let next_page_token = req.page_token;
        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let (songs, token) = block_on(
                api_client.get_album_content(album, parse_next_page_token(next_page_token)),
            )?;

            return Ok(SongsWithPageTokenReturnType {
                songs,
                next_page_token: Some(token.try_into()?),
            });
        }

        Err("Not logged in".into())
    }

    fn get_playlist_content(
        &self,
        req: RequestedPlaylistSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let id = req.id;
        let next_page_token = req.page_token;
        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            // Construct a temporary Playlist object with the ID to pass to the API client
            let playlist = Playlist {
                playlist_id: Some(id),
                ..Default::default()
            };

            let (songs, token) = block_on(
                api_client.get_playlist_content(playlist, parse_next_page_token(next_page_token)),
            )?;

            return Ok(SongsWithPageTokenReturnType {
                songs,
                next_page_token: Some(token.try_into()?),
            });
        }
        Err("Not logged in".into())
    }

    fn get_artist_songs(
        &self,
        req: RequestedArtistSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let artist = req.artist.unwrap();
        let next_page_token = req.page_token;
        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let (songs, token) = block_on(
                api_client.get_artist_content(artist, parse_next_page_token(next_page_token)),
            )?;

            return Ok(SongsWithPageTokenReturnType {
                songs,
                next_page_token: Some(token.try_into()?),
            });
        }
        Err("Not logged in".into())
    }

    fn get_playlists(&self, _: RequestedPlaylistsRequest) -> MoosyncResult<Vec<Playlist>> {
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

            return Ok(all_playlists);
        }
        Err("Not logged in".into())
    }

    fn get_song_from_id(&self, req: RequestedSongFromIdRequest) -> MoosyncResult<Option<Song>> {
        let id = req.id;
        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let song = block_on(api_client.song_from_id(id))?;
            return Ok(Some(song));
        }
        Err("Not logged in".into())
    }

    fn get_song_from_url(&self, req: RequestedSongFromUrlRequest) -> MoosyncResult<Option<Song>> {
        let url = req.url;
        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let song = block_on(api_client.song_from_url(url))?;
            return Ok(Some(song));
        }
        Err("Not logged in".into())
    }

    fn search(&self, req: RequestedSearchResultRequest) -> MoosyncResult<SearchResult> {
        let term = req.query;
        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let results = block_on(api_client.search(term))?;
            return Ok(results);
        }
        Err("Not logged in".into())
    }

    fn get_playlist_from_url(
        &self,
        req: RequestedPlaylistFromUrlRequest,
    ) -> MoosyncResult<Option<Playlist>> {
        let url = req.url;
        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let playlist = block_on(api_client.playlist_from_url(url))?;
            return Ok(Some(playlist));
        }
        Err("Not logged in".into())
    }

    fn get_recommendations(&self, _: RequestedRecommendationsRequest) -> MoosyncResult<Vec<Song>> {
        let api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_ref() {
            let songs = block_on(api_client.get_suggestions())?;
            return Ok(songs);
        }
        Err("Not logged in".into())
    }

    fn handle_custom_request(&self, _: CustomRequest) -> MoosyncResult<CustomRequestReturnType> {
        Err("Not implemented".into())
    }
}
impl DatabaseEvents for SpotifyExtension {}
impl PreferenceEvents for SpotifyExtension {}
impl Extension for SpotifyExtension {}
impl ContextMenu for SpotifyExtension {}
impl Accounts for SpotifyExtension {
    fn perform_account_login(&self, args: AccountLoginArgs) -> MoosyncResult<String> {
        let mut api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_mut() {
            if args.login_status {
                return api_client.login();
            } else {
                api_client.logout();
                return Ok("".into());
            }
        }
        Err("Missing client credentials".into())
    }

    fn oauth_callback(&self, req: OauthCallbackRequest) -> MoosyncResult<()> {
        let mut api_client = self.api_client.lock().unwrap();
        if let Some(api_client) = api_client.as_mut() {
            block_on(api_client.authorize(req.callback_uri))?;

            let tokens = api_client.get_tokens();
            if let Err(e) = set_secure(PreferenceData {
                key: "tokens".into(),
                value: tokens.map(|t| serde_to_google_value(serde_json::to_value(t).unwrap())),
                ..Default::default()
            }) {
                error!("Failed to set tokens in secure storage: {}", e);
            };
            return Ok(());
        }
        Err("Missing client credentials".into())
    }

    fn get_accounts(&self) -> MoosyncResult<Vec<ExtensionAccountDetail>> {
        let mut api_client = self.api_client.lock().unwrap();
        let mut username = None;
        let mut logged_in = false;
        if let Some(api_client) = api_client.as_mut() {
            logged_in = true;
            username = block_on(api_client.get_username())?;
        }

        Ok(vec![ExtensionAccountDetail {
            id: "spotify".into(),
            package_name: "moosync.spotify".into(),
            name: "Spotify".into(),
            bg_color: "#07C330".into(),
            logged_in,
            icon: "".into(),
            username,
        }])
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    info!("Initializing spotify extension");

    let extension = SpotifyExtension::new();

    register_extension(Box::new(extension)).unwrap();
    register_prefs().unwrap();

    info!("Initialized spotify extension");
}

fn main() {}
