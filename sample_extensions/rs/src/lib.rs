#![no_main]

use moosync_edk::{
    AddToPlaylistRequest, ContextMenuActionRequest, ContextMenuReturnType, CustomRequest,
    ExtensionAccountDetail, ExtensionProviderScope, OauthCallbackRequest,
    PerformAccountLoginRequest, PlaybackDetailsRequestedRequest, PlayerStateChangedRequest,
    Playlist, PlaylistAddedRequest, PlaylistRemovedRequest, PreferenceChangedRequest,
    PreferenceData, RequestedAlbumSongsRequest, RequestedArtistSongsRequest,
    RequestedLyricsRequest, RequestedPlaylistContextMenuRequest, RequestedPlaylistFromUrlRequest,
    RequestedPlaylistSongsRequest, RequestedPlaylistsRequest, RequestedRecommendationsRequest,
    RequestedSearchResultRequest, RequestedSongContextMenuRequest, RequestedSongFromIdRequest,
    RequestedSongFromUrlRequest, ScrobbleRequest, SearchResult, SeekedRequest, Song,
    SongAddedRequest, SongChangedRequest, SongQueueChangedRequest, SongRemovedRequest,
    VolumeChangedRequest,
    api::{
        self, Accounts, ContextMenu, CustomRequestReturnType, DatabaseEvents, Extension,
        PlaybackDetailsReturnType, PlayerEvents, PreferenceEvents, Provider,
        SongsWithPageTokenReturnType,
        extension_api::{
            add_playlist, add_songs, add_to_playlist, gen_hash, get_current_song, get_player_state,
            get_preference, get_queue, get_secure, get_system_time, get_time, get_volume,
            open_external_url, open_sock, read_sock, register_oauth, register_user_preferences,
            unregister_user_preferences, update_accounts, write_sock,
        },
    },
    info,
};

struct SampleExtension;

impl Extension for SampleExtension {}

impl Accounts for SampleExtension {
    fn get_accounts(&self) -> api::MoosyncResult<Vec<ExtensionAccountDetail>> {
        info!("get_accounts called");
        let _ = update_accounts(Some("test_account".to_string()));
        Ok(vec![ExtensionAccountDetail {
            id: "test_account".to_string(),
            name: "Test Account".to_string(),
            logged_in: true,
            ..Default::default()
        }])
    }

    fn perform_account_login(&self, req: PerformAccountLoginRequest) -> api::MoosyncResult<String> {
        info!("perform_account_login called: {:?}", req);
        let _ = register_oauth("https://example.com/callback".to_string());
        Ok("success".to_string())
    }

    fn oauth_callback(&self, req: OauthCallbackRequest) -> api::MoosyncResult<()> {
        info!("oauth_callback called with code: {}", req.callback_uri);
        Ok(())
    }
}

impl DatabaseEvents for SampleExtension {
    fn on_song_added(&self, req: SongAddedRequest) -> api::MoosyncResult<()> {
        for song in req.songs {
            if let Some(inner) = &song.song {
                info!("on_song_added: {:?}", inner.title);
            }
        }
        Ok(())
    }

    fn on_song_removed(&self, req: SongRemovedRequest) -> api::MoosyncResult<()> {
        for song in req.songs {
            if let Some(inner) = &song.song {
                info!("on_song_removed: {:?}", inner.title);
            }
        }
        Ok(())
    }

    fn on_playlist_added(&self, req: PlaylistAddedRequest) -> api::MoosyncResult<()> {
        for playlist in req.playlists {
            info!("on_playlist_added: {:?}", playlist.playlist_name);
        }
        Ok(())
    }

    fn on_playlist_removed(&self, req: PlaylistRemovedRequest) -> api::MoosyncResult<()> {
        for playlist in req.playlists {
            info!("on_playlist_removed: {:?}", playlist.playlist_name);
        }
        Ok(())
    }
}

impl PreferenceEvents for SampleExtension {
    fn on_preferences_changed(&self, req: PreferenceChangedRequest) -> api::MoosyncResult<()> {
        if let Some(args) = req.preference {
            info!("on_preferences_changed: {:?}", args);
            let key = args.key;
            if !key.is_empty() {
                let _val = get_preference(PreferenceData {
                    key: key.clone(),
                    ..Default::default()
                });
                let _sec = get_secure(PreferenceData {
                    key,
                    ..Default::default()
                });
            }
        }
        Ok(())
    }
}

impl PlayerEvents for SampleExtension {
    fn on_queue_changed(&self, _req: SongQueueChangedRequest) -> api::MoosyncResult<()> {
        info!("on_queue_changed");
        let _q = get_queue();
        Ok(())
    }

    fn on_volume_changed(&self, _req: VolumeChangedRequest) -> api::MoosyncResult<()> {
        info!("on_volume_changed");
        let _vol = get_volume();
        Ok(())
    }

    fn on_player_state_changed(&self, _req: PlayerStateChangedRequest) -> api::MoosyncResult<()> {
        info!("on_player_state_changed");
        let _state = get_player_state();
        Ok(())
    }

    fn on_song_changed(&self, _req: SongChangedRequest) -> api::MoosyncResult<()> {
        info!("on_song_changed");
        let _song = get_current_song();
        Ok(())
    }

    fn on_seeked(&self, req: SeekedRequest) -> api::MoosyncResult<()> {
        info!("on_seeked: {}", req.position);
        let _t = get_time();
        Ok(())
    }
}

impl Provider for SampleExtension {
    fn get_provider_scopes(&self) -> api::MoosyncResult<Vec<ExtensionProviderScope>> {
        Ok(vec![ExtensionProviderScope::Accounts])
    }

    fn get_playlists(&self, _req: RequestedPlaylistsRequest) -> api::MoosyncResult<Vec<Playlist>> {
        info!("get_playlists called");
        Ok(vec![])
    }

    fn get_playlist_content(
        &self,
        req: RequestedPlaylistSongsRequest,
    ) -> api::MoosyncResult<SongsWithPageTokenReturnType> {
        info!("get_playlist_content: {}", req.id);
        Ok(SongsWithPageTokenReturnType {
            songs: vec![],
            next_page_token: None,
        })
    }

    fn get_playlist_from_url(
        &self,
        req: RequestedPlaylistFromUrlRequest,
    ) -> api::MoosyncResult<Option<Playlist>> {
        info!("get_playlist_from_url: {}", req.url);
        Ok(None)
    }

    fn get_playback_details(
        &self,
        req: PlaybackDetailsRequestedRequest,
    ) -> api::MoosyncResult<PlaybackDetailsReturnType> {
        if let Some(song) = req.song {
            if let Some(inner) = &song.song {
                info!("get_playback_details: {:?}", inner.id);
            }
        }
        Ok(PlaybackDetailsReturnType {
            duration: 0,
            url: "https://example.com/song.mp3".to_string(),
        })
    }

    fn search(&self, req: RequestedSearchResultRequest) -> api::MoosyncResult<SearchResult> {
        info!("search: {}", req.query);
        let _time = get_system_time();
        let _ = open_external_url("https://google.com".to_string());
        Ok(SearchResult {
            songs: vec![],
            playlists: vec![],
            artists: vec![],
            albums: vec![],
            genres: vec![],
        })
    }

    fn get_recommendations(
        &self,
        _req: RequestedRecommendationsRequest,
    ) -> api::MoosyncResult<Vec<Song>> {
        info!("get_recommendations");
        Ok(vec![])
    }

    fn get_song_from_url(
        &self,
        req: RequestedSongFromUrlRequest,
    ) -> api::MoosyncResult<Option<Song>> {
        info!("get_song_from_url: {}", req.url);
        Ok(None)
    }

    fn handle_custom_request(
        &self,
        req: CustomRequest,
    ) -> api::MoosyncResult<CustomRequestReturnType> {
        let request_id = req.request_id;
        info!("handle_custom_request: {}", request_id);

        if request_id == "socket_test" {
            if let Ok(fd) = open_sock("/tmp/test.sock".to_string()) {
                let _ = write_sock(fd, vec![1, 2, 3]);
                let _ = read_sock(fd, 3);
            }
        }

        if request_id == "hash_test" {
            let _ = gen_hash("sha256".to_string(), vec![1, 2, 3]);
        }

        if request_id == "preferences_test" {
            let _ = register_user_preferences(vec![]);
            let _ = unregister_user_preferences(vec![]);
        }

        Ok(CustomRequestReturnType {
            mime_type: None,
            data: None,
            redirect_url: None,
        })
    }

    fn get_artist_songs(
        &self,
        req: RequestedArtistSongsRequest,
    ) -> api::MoosyncResult<SongsWithPageTokenReturnType> {
        if let Some(artist) = req.artist {
            info!("get_artist_songs: {:?}", artist.artist_name);
        }
        Ok(SongsWithPageTokenReturnType {
            songs: vec![],
            next_page_token: None,
        })
    }

    fn get_album_songs(
        &self,
        req: RequestedAlbumSongsRequest,
    ) -> api::MoosyncResult<SongsWithPageTokenReturnType> {
        if let Some(album) = req.album {
            info!("get_album_songs: {:?}", album.album_name);
        }
        Ok(SongsWithPageTokenReturnType {
            songs: vec![],
            next_page_token: None,
        })
    }

    fn get_song_from_id(
        &self,
        req: RequestedSongFromIdRequest,
    ) -> api::MoosyncResult<Option<Song>> {
        info!("get_song_from_id: {}", req.id);
        Ok(None)
    }

    fn scrobble(&self, req: ScrobbleRequest) -> api::MoosyncResult<()> {
        if let Some(song) = req.song {
            if let Some(inner) = &song.song {
                info!("scrobble: {:?}", inner.title);
            }
        }
        Ok(())
    }

    fn get_lyrics(&self, req: RequestedLyricsRequest) -> api::MoosyncResult<String> {
        if let Some(song) = req.song {
            if let Some(inner) = &song.song {
                info!("get_lyrics: {:?}", inner.title);
            }
        }
        Ok("Sample lyrics".to_string())
    }
}

impl ContextMenu for SampleExtension {
    fn get_song_context_menu(
        &self,
        _req: RequestedSongContextMenuRequest,
    ) -> api::MoosyncResult<Vec<ContextMenuReturnType>> {
        info!("get_song_context_menu");
        Ok(vec![])
    }

    fn get_playlist_context_menu(
        &self,
        _req: RequestedPlaylistContextMenuRequest,
    ) -> api::MoosyncResult<Vec<ContextMenuReturnType>> {
        info!("get_playlist_context_menu");
        Ok(vec![])
    }

    fn on_context_menu_action(&self, req: ContextMenuActionRequest) -> api::MoosyncResult<()> {
        let action = req.action_id;
        info!("on_context_menu_action: {}", action);
        if action == "add_test" {
            let _ = add_playlist(Playlist::default());
            let _ = add_songs(vec![]);
            let _ = add_to_playlist(AddToPlaylistRequest::default());
        }
        Ok(())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    moosync_edk::handler::register_extension(Box::new(SampleExtension)).unwrap();
}
