use std::sync::Mutex;

use moosync_edk::{
    api::{
        self, get_cache, set_cache, set_cache_with_ttl, Accounts, ContextMenu, DatabaseEvents,
        Extension, PlayerEvents, PlayerProvider, PreferenceEvents, Provider,
    },
    handler::register_extension,
    info,
    types::{
        CustomRequest, CustomRequestResponse, PerformAccountLoginRequest,
        PerformAccountLoginResponse, PreferenceItem, PreferenceType,
        RegisterUserPreferenceRequest, RequestedAlbumSongsRequest, RequestedAlbumSongsResponse,
        RequestedArtistSongsRequest, RequestedArtistSongsResponse, RequestedPlaylistSongsRequest,
        RequestedPlaylistSongsResponse, RequestedPlaylistsRequest, RequestedPlaylistsResponse,
        RequestedSearchResultRequest, RequestedSearchResultResponse,
    },
    MoosyncResult,
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
impl PlayerProvider for KoelExtension {}
impl Provider for KoelExtension {
    fn requested_search_result(
        &self,
        req: RequestedSearchResultRequest,
    ) -> MoosyncResult<RequestedSearchResultResponse> {
        let cache_key = cache_keys::search(&req.query);
        if let Ok(Some(cached)) = get_cache::<RequestedSearchResultResponse>(&cache_key) {
            return Ok(cached);
        }

        let mut inner = self.inner.lock().unwrap();
        let res = inner.search(req.query)?;
        let resp = RequestedSearchResultResponse {
            songs: res.songs,
            playlists: res.playlists,
            artists: res.artists,
            albums: res.albums,
        };
        let _ = set_cache_with_ttl(&cache_key, &resp, Some(7200));
        Ok(resp)
    }

    fn requested_playlists(
        &self,
        _: RequestedPlaylistsRequest,
    ) -> MoosyncResult<RequestedPlaylistsResponse> {
        let cache_key = cache_keys::user_playlists();
        if let Ok(Some(cached)) = get_cache::<RequestedPlaylistsResponse>(&cache_key) {
            return Ok(cached);
        }

        let mut inner = self.inner.lock().unwrap();
        let playlists = inner.get_playlists()?;
        let resp = RequestedPlaylistsResponse { playlists };
        let _ = set_cache_with_ttl(&cache_key, &resp, Some(3600));
        Ok(resp)
    }

    fn requested_playlist_songs(
        &self,
        req: RequestedPlaylistSongsRequest,
    ) -> MoosyncResult<RequestedPlaylistSongsResponse> {
        let id = req.id.replace("moosync.koel:", "");
        let cache_key = cache_keys::playlist_songs(&id, req.page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedPlaylistSongsResponse>(&cache_key) {
            return Ok(cached);
        }

        let mut inner = self.inner.lock().unwrap();
        let res = inner.get_playlist_content(&id, req.page_token)?;
        let resp = RequestedPlaylistSongsResponse {
            songs: res.songs,
            next_page_token: res.next_page_token,
        };
        let _ = set_cache_with_ttl(&cache_key, &resp, Some(3600));
        Ok(resp)
    }

    fn requested_artist_songs(
        &self,
        req: RequestedArtistSongsRequest,
    ) -> MoosyncResult<RequestedArtistSongsResponse> {
        let artist = req.artist.ok_or("No artist provided")?;
        let artist_name = artist.artist_name.ok_or("Artist not found")?;
        let cache_key = cache_keys::artist_songs(&artist_name, req.page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedArtistSongsResponse>(&cache_key) {
            return Ok(cached);
        }

        let mut inner = self.inner.lock().unwrap();
        let res = inner.get_songs_by_artist_name(&artist_name, req.page_token)?;
        let resp = RequestedArtistSongsResponse {
            songs: res.songs,
            next_page_token: res.next_page_token,
        };
        let _ = set_cache_with_ttl(&cache_key, &resp, Some(86400));
        Ok(resp)
    }

    fn requested_album_songs(
        &self,
        req: RequestedAlbumSongsRequest,
    ) -> MoosyncResult<RequestedAlbumSongsResponse> {
        let album = req.album.ok_or("No album provided")?;
        let album_name = album.album_name.ok_or("Album not found")?;
        let cache_key = cache_keys::album_songs(&album_name, req.page_token.as_deref());
        if let Ok(Some(cached)) = get_cache::<RequestedAlbumSongsResponse>(&cache_key) {
            return Ok(cached);
        }

        let mut inner = self.inner.lock().unwrap();
        let res = inner.get_songs_by_album_name(&album_name, req.page_token)?;
        let resp = RequestedAlbumSongsResponse {
            songs: res.songs,
            next_page_token: res.next_page_token,
        };
        let _ = set_cache(&cache_key, &resp);
        Ok(resp)
    }

    fn custom_request(&self, req: CustomRequest) -> MoosyncResult<CustomRequestResponse> {
        let inner = self.inner.lock().unwrap();
        let payload = req.payload.ok_or("No payload provided")?;
        let url_val = utils::google_struct_to_serde(payload);
        let url = url_val
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or("Invalid payload")?;
        Ok(CustomRequestResponse {
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
        args: PerformAccountLoginRequest,
    ) -> MoosyncResult<PerformAccountLoginResponse> {
        let mut inner = self.inner.lock().unwrap();
        let resp = inner.perform_account_login(args.login_status);
        info!("Got resp {:?}", resp);
        Ok(PerformAccountLoginResponse { status: resp? })
    }
}
impl Extension for KoelExtension {}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    info!("Initializing KoelExtension");
    let extension = KoelExtension::new();
    let _ = extension.inner.lock().unwrap().sync_account();
    register_extension(Box::new(extension)).unwrap();

    let _ = api::register_user_preference(RegisterUserPreferenceRequest {
        prefs: vec![
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
        ],
    });
    info!("Initialized KoelExtension");
}

fn main() {}
