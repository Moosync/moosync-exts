use std::sync::Mutex;

use moosync_edk::{
    CustomRequestReturnType, ExtensionProviderScope, InputType, PreferenceTypes, PreferenceUIData,
    QueryablePlaylist, Result, SearchResult, Song, SongsWithPageTokenReturnType,
    api::{
        Accounts, ContextMenu, DatabaseEvents, Extension, PlayerEvents, PreferenceEvents, Provider,
        extension_api::register_user_preferences,
    },
    handler::register_extension,
    info,
};

mod error;
mod koel;

pub struct KoelExtension {
    pub inner: Mutex<koel::KoelClient>,
}

impl KoelExtension {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(koel::KoelClient::new()),
        }
    }
}

impl PlayerEvents for KoelExtension {}
impl Provider for KoelExtension {
    fn get_provider_scopes(&self) -> Result<Vec<ExtensionProviderScope>> {
        Ok(vec![
            ExtensionProviderScope::Playlists,
            ExtensionProviderScope::PlaylistSongs,
            ExtensionProviderScope::PlaybackDetails,
            ExtensionProviderScope::Search,
            ExtensionProviderScope::Accounts,
            ExtensionProviderScope::SongFromUrl,
            ExtensionProviderScope::AlbumSongs,
            ExtensionProviderScope::ArtistSongs,
        ])
    }

    fn search(&self, term: String) -> Result<SearchResult> {
        let mut inner = self.inner.lock().unwrap();
        Ok(inner.search(term)?)
    }

    fn get_song_from_url(&self, url: String) -> Result<Option<Song>> {
        let mut inner = self.inner.lock().unwrap();
        Ok(inner.get_song_from_url(url)?)
    }

    fn get_playlists(&self) -> Result<Vec<QueryablePlaylist>> {
        let mut inner = self.inner.lock().unwrap();
        Ok(inner.get_playlists()?)
    }

    fn get_playlist_content(
        &self,
        id: String,
        next_page_token: Option<String>,
    ) -> Result<SongsWithPageTokenReturnType> {
        let mut inner = self.inner.lock().unwrap();
        let id = id.replace("moosync.koel:", "");
        Ok(inner.get_playlist_content(&id, next_page_token)?)
    }

    fn get_artist_songs(
        &self,
        artist: moosync_edk::QueryableArtist,
        next_page_token: Option<String>,
    ) -> Result<SongsWithPageTokenReturnType> {
        let artist_name = artist.artist_name;
        if let Some(artist_name) = artist_name {
            let mut inner = self.inner.lock().unwrap();
            return Ok(inner.get_songs_by_artist_name(&artist_name, next_page_token)?);
        }
        Err("Artist not found".into())
    }

    fn get_album_songs(
        &self,
        album: moosync_edk::QueryableAlbum,
        next_page_token: Option<String>,
    ) -> Result<SongsWithPageTokenReturnType> {
        let album_name = album.album_name;
        if let Some(album_name) = album_name {
            let mut inner = self.inner.lock().unwrap();
            return Ok(inner.get_songs_by_album_name(&album_name, next_page_token)?);
        }
        Err("Album not found".into())
    }

    fn handle_custom_request(&self, url: String) -> Result<CustomRequestReturnType> {
        let inner = self.inner.lock().unwrap();
        Ok(CustomRequestReturnType {
            mime_type: None,
            data: None,
            redirect_url: Some(inner.handle_custom_request(url)?),
        })
    }
}
impl DatabaseEvents for KoelExtension {}
impl PreferenceEvents for KoelExtension {}
impl ContextMenu for KoelExtension {}
impl Accounts for KoelExtension {
    fn get_accounts(&self) -> moosync_edk::Result<Vec<moosync_edk::ExtensionAccountDetail>> {
        let inner = self.inner.lock().unwrap();
        Ok(inner.get_accounts()?)
    }

    fn perform_account_login(
        &self,
        args: moosync_edk::AccountLoginArgs,
    ) -> moosync_edk::Result<String> {
        let mut inner = self.inner.lock().unwrap();
        let resp = inner.perform_account_login(args);
        info!("Got resp {:?}", resp);
        return Ok(resp?);
    }
}
impl Extension for KoelExtension {}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    info!("Initializing KoelExtension");
    register_extension(Box::new(KoelExtension::new())).unwrap();

    register_user_preferences(vec![
        PreferenceUIData {
            _type: PreferenceTypes::EditText,
            title: "Instance URL".into(),
            key: "koel_instance_url".into(),
            description: "Full URL to your koel instance".into(),
            input_type: Some(InputType::Text),
            ..Default::default()
        },
        PreferenceUIData {
            _type: PreferenceTypes::EditText,
            title: "Email".into(),
            key: "koel_username".into(),
            description: "Email for your koel account".into(),
            input_type: Some(InputType::Text),
            ..Default::default()
        },
        PreferenceUIData {
            _type: PreferenceTypes::EditText,
            title: "Password".into(),
            key: "koel_password".into(),
            description: "Password for your koel account".into(),
            input_type: Some(InputType::SecureText),
            ..Default::default()
        },
    ]);
    info!("Initialized KoelExtension");
}
