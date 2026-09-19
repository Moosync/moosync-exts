use std::sync::Mutex;

use moosync_edk::{
    ExtensionProviderScope, MoosyncResult, Playlist, PreferenceItem, PreferenceType,
    RequestedAlbumSongsRequest, RequestedArtistSongsRequest, RequestedPlaylistSongsRequest,
    RequestedPlaylistsRequest, RequestedSearchResultRequest, RequestedSongFromUrlRequest,
    SearchResult, Song,
    api::{
        Accounts, ContextMenu, CustomRequest, CustomRequestReturnType, DatabaseEvents, Extension,
        PlayerEvents, PreferenceEvents, Provider, SongsWithPageTokenReturnType,
        extension_api::register_user_preferences,
    },
    handler::register_extension,
    info,
};

mod error;
mod koel;
mod utils;

pub struct KoelExtension {
    pub inner: Mutex<koel::KoelClient>,
}

impl Default for KoelExtension {
    fn default() -> Self {
        Self::new()
    }
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
    fn get_provider_scopes(&self) -> MoosyncResult<Vec<ExtensionProviderScope>> {
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

    fn search(&self, req: RequestedSearchResultRequest) -> MoosyncResult<SearchResult> {
        let mut inner = self.inner.lock().unwrap();
        Ok(inner.search(req.query)?)
    }

    fn get_song_from_url(&self, req: RequestedSongFromUrlRequest) -> MoosyncResult<Option<Song>> {
        let mut inner = self.inner.lock().unwrap();
        Ok(inner.get_song_from_url(req.url)?)
    }

    fn get_playlists(&self, _: RequestedPlaylistsRequest) -> MoosyncResult<Vec<Playlist>> {
        let mut inner = self.inner.lock().unwrap();
        Ok(inner.get_playlists()?)
    }

    fn get_playlist_content(
        &self,
        req: RequestedPlaylistSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let mut inner = self.inner.lock().unwrap();
        let id = req.id.replace("moosync.koel:", "");
        Ok(inner.get_playlist_content(&id, req.page_token)?)
    }

    fn get_artist_songs(
        &self,
        req: RequestedArtistSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let artist = req.artist.ok_or("No artist provided")?;
        let artist_name = artist.artist_name;
        if let Some(artist_name) = artist_name {
            let mut inner = self.inner.lock().unwrap();
            return Ok(inner.get_songs_by_artist_name(&artist_name, req.page_token)?);
        }
        Err("Artist not found".into())
    }

    fn get_album_songs(
        &self,
        req: RequestedAlbumSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let album = req.album.ok_or("No album provided")?;
        let album_name = album.album_name;
        if let Some(album_name) = album_name {
            let mut inner = self.inner.lock().unwrap();
            return Ok(inner.get_songs_by_album_name(&album_name, req.page_token)?);
        }
        Err("Album not found".into())
    }

    fn handle_custom_request(&self, req: CustomRequest) -> MoosyncResult<CustomRequestReturnType> {
        let inner = self.inner.lock().unwrap();
        let payload = req.payload.ok_or("No payload provided")?;
        let url_val = utils::google_struct_to_serde(payload);
        let url = url_val
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or("Invalid payload")?;
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
    fn perform_account_login(
        &self,
        args: moosync_edk::api::AccountLoginArgs,
    ) -> MoosyncResult<String> {
        let mut inner = self.inner.lock().unwrap();
        let resp = inner.perform_account_login(args);
        info!("Got resp {:?}", resp);
        Ok(resp?)
    }
}
impl Extension for KoelExtension {}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    info!("Initializing KoelExtension");
    let extension = KoelExtension::new();
    let _ = extension.inner.lock().unwrap().sync_account();
    register_extension(Box::new(extension)).unwrap();

    let _ = register_user_preferences(vec![
        PreferenceItem {
            pref_type: PreferenceType::TextInputGroup as i32,
            title: "Instance URL".into(),
            id: "koel_instance_url".into(),
            subtitle: "Full URL to your koel instance".into(),
            ..Default::default()
        },
        PreferenceItem {
            pref_type: PreferenceType::TextInputGroup as i32,
            title: "Email".into(),
            id: "koel_username".into(),
            subtitle: "Email for your koel account".into(),
            ..Default::default()
        },
        PreferenceItem {
            pref_type: PreferenceType::TextInputGroup as i32,
            title: "Password".into(),
            id: "koel_password".into(),
            subtitle: "Password for your koel account".into(),
            ..Default::default()
        },
    ]);
    info!("Initialized KoelExtension");
}

fn main() {}
