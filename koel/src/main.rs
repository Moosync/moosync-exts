use std::sync::Mutex;

use moosync_edk::{
    ExtensionProviderScope, MoosyncResult, Playlist, PreferenceItem, PreferenceType,
    RequestedAlbumSongsRequest, RequestedAlbumSongsResponse, RequestedArtistSongsRequest,
    RequestedArtistSongsResponse, RequestedPlaylistSongsRequest, RequestedPlaylistSongsResponse,
    RequestedPlaylistsRequest, RequestedPlaylistsResponse, RequestedSearchResultRequest,
    SearchResult, Song,
    api::{
        Accounts, ContextMenu, CustomRequest, CustomRequestReturnType, DatabaseEvents, Extension,
        PlayerEvents, PreferenceEvents, Provider, SongsWithPageTokenReturnType,
        extension_api::{get_cache, register_user_preferences, set_cache, set_cache_with_ttl},
    },
    handler::register_extension,
    info,
};

mod cache_keys;
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
    fn search(&self, req: RequestedSearchResultRequest) -> MoosyncResult<SearchResult> {
        let cache_key = cache_keys::search(&req.query);
        if let Ok(Some(cached)) = get_cache::<SearchResult>(&cache_key) {
            return Ok(cached);
        }

        let mut inner = self.inner.lock().unwrap();
        let res = inner.search(req.query)?;
        let _ = set_cache_with_ttl(&cache_key, &res, Some(7200));
        Ok(res)
    }

    fn get_playlists(&self, _: RequestedPlaylistsRequest) -> MoosyncResult<Vec<Playlist>> {
        let cache_key = cache_keys::user_playlists();
        if let Ok(Some(cached)) = get_cache::<RequestedPlaylistsResponse>(&cache_key) {
            return Ok(cached.playlists);
        }

        let mut inner = self.inner.lock().unwrap();
        let playlists = inner.get_playlists()?;
        let _ = set_cache_with_ttl(
            &cache_key,
            &RequestedPlaylistsResponse {
                playlists: playlists.clone(),
            },
            Some(3600),
        );
        Ok(playlists)
    }

    fn get_playlist_content(
        &self,
        req: RequestedPlaylistSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let id = req.id.replace("moosync.koel:", "");
        let cache_key = cache_keys::playlist_songs(&id, req.page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedPlaylistSongsResponse>(&cache_key) {
            return Ok(SongsWithPageTokenReturnType {
                songs: cached.songs,
                next_page_token: cached.next_page_token,
            });
        }

        let mut inner = self.inner.lock().unwrap();
        let res = inner.get_playlist_content(&id, req.page_token)?;
        let _ = set_cache_with_ttl(
            &cache_key,
            &RequestedPlaylistSongsResponse {
                songs: res.songs.clone(),
                next_page_token: res.next_page_token.clone(),
            },
            Some(3600),
        );
        Ok(res)
    }

    fn get_artist_songs(
        &self,
        req: RequestedArtistSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let artist = req.artist.ok_or("No artist provided")?;
        let artist_name = artist.artist_name.ok_or("Artist not found")?;
        let cache_key = cache_keys::artist_songs(&artist_name, req.page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedArtistSongsResponse>(&cache_key) {
            return Ok(SongsWithPageTokenReturnType {
                songs: cached.songs,
                next_page_token: cached.next_page_token,
            });
        }

        let mut inner = self.inner.lock().unwrap();
        let res = inner.get_songs_by_artist_name(&artist_name, req.page_token)?;
        let _ = set_cache_with_ttl(
            &cache_key,
            &RequestedArtistSongsResponse {
                songs: res.songs.clone(),
                next_page_token: res.next_page_token.clone(),
            },
            Some(86400),
        );
        Ok(res)
    }

    fn get_album_songs(
        &self,
        req: RequestedAlbumSongsRequest,
    ) -> MoosyncResult<SongsWithPageTokenReturnType> {
        let album = req.album.ok_or("No album provided")?;
        let album_name = album.album_name.ok_or("Album not found")?;
        let cache_key = cache_keys::album_songs(&album_name, req.page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedAlbumSongsResponse>(&cache_key) {
            return Ok(SongsWithPageTokenReturnType {
                songs: cached.songs,
                next_page_token: cached.next_page_token,
            });
        }

        let mut inner = self.inner.lock().unwrap();
        let res = inner.get_songs_by_album_name(&album_name, req.page_token)?;
        let _ = set_cache(
            &cache_key,
            &RequestedAlbumSongsResponse {
                songs: res.songs.clone(),
                next_page_token: res.next_page_token.clone(),
            },
        );
        Ok(res)
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
