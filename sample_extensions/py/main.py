from moosync_edk import Extension, register_extension
from core.types.protos import extensions_pb2, songs_pb2


def entry():
    print("calling entry")
    ext = SampleExtension()
    register_extension(ext)
    ext.api.set_account(
        extensions_pb2.ExtensionAccountDetail(
            id="test_account",
            name="Test Account",
            logged_in=False,
        )
    )


def mock_sample_song(song_id, title):
    return songs_pb2.Song(
        song=songs_pb2.InnerSong(id=song_id, title=title),
        artists=[songs_pb2.Artist(artist_id=f"art-{song_id}", artist_name=f"Artist for {title}")],
        album=songs_pb2.Album(album_id=f"alb-{song_id}", album_name=f"Album for {title}"),
    )


class SampleExtension(Extension):
    def get_provider_scopes(self, req):
        return extensions_pb2.GetProviderScopesResponse(
            scopes=[
                extensions_pb2.ExtensionProviderScope.ACCOUNTS,
                extensions_pb2.ExtensionProviderScope.SEARCH,
                extensions_pb2.ExtensionProviderScope.PLAYLISTS,
                extensions_pb2.ExtensionProviderScope.PLAYLIST_SONGS,
                extensions_pb2.ExtensionProviderScope.ARTIST_SONGS,
                extensions_pb2.ExtensionProviderScope.ALBUM_SONGS,
            ]
        )

    def get_playlists(self, req):
        return extensions_pb2.RequestedPlaylistsResponse(
            playlists=[
                songs_pb2.Playlist(playlist_id="ext-playlist-1", playlist_name="Extension Playlist 1"),
                songs_pb2.Playlist(playlist_id="ext-playlist-2", playlist_name="Extension Playlist 2"),
            ]
        )

    def get_playlist_content(self, req):
        return extensions_pb2.RequestedPlaylistSongsResponse(
            songs=[
                mock_sample_song("pl-song-1", "Playlist Song 1"),
                mock_sample_song("pl-song-2", "Playlist Song 2"),
            ]
        )

    def get_artist_songs(self, req):
        return extensions_pb2.RequestedArtistSongsResponse(
            songs=[
                mock_sample_song("artist-song-1", "Artist Song 1"),
                mock_sample_song("artist-song-2", "Artist Song 2"),
            ]
        )

    def get_album_songs(self, req):
        return extensions_pb2.RequestedAlbumSongsResponse(
            songs=[
                mock_sample_song("album-song-1", "Album Song 1"),
                mock_sample_song("album-song-2", "Album Song 2"),
            ]
        )

    def get_search(self, req):
        self.api.open_external_url("test")
        return extensions_pb2.RequestedSearchResultResponse(
            songs=[
                mock_sample_song("search-song-1", "Search Song 1"),
                mock_sample_song("search-song-2", "Search Song 2"),
            ],
            playlists=[
                songs_pb2.Playlist(playlist_id="search-pl-1", playlist_name="Search Playlist 1"),
                songs_pb2.Playlist(playlist_id="search-pl-2", playlist_name="Search Playlist 2"),
            ],
            artists=[
                songs_pb2.Artist(artist_id="search-art-1", artist_name="Search Artist 1"),
                songs_pb2.Artist(artist_id="search-art-2", artist_name="Search Artist 2"),
            ],
            albums=[
                songs_pb2.Album(album_id="search-alb-1", album_name="Search Album 1"),
                songs_pb2.Album(album_id="search-alb-2", album_name="Search Album 2"),
            ],
        )

    def perform_account_login(self, req):
        self.api.register_oauth("https://example.com/callback")
        if req.login_status:
            self.api.set_account(
                extensions_pb2.ExtensionAccountDetail(
                    id="test_account",
                    name="Test Account",
                    logged_in=True,
                )
            )
            return extensions_pb2.PerformAccountLoginResponse(status="success")
        else:
            self.api.set_account(
                extensions_pb2.ExtensionAccountDetail(
                    id="test_account",
                    name="Test Account",
                    logged_in=False,
                )
            )
            return extensions_pb2.PerformAccountLoginResponse(status="logged_out")

    def handle_custom_request(self, req):
        if req.request_id == "hash_test":
            return extensions_pb2.CustomRequestResponse()

        if req.request_id == "preferences_test":
            self.api.register_user_preferences([])
            self.api.unregister_user_preferences([])
            return extensions_pb2.CustomRequestResponse()

        if req.request_id == "http_get_test":
            from moosync_edk import http_get
            try:
                http_get("https://example.com")
            except Exception:
                pass
            return extensions_pb2.CustomRequestResponse()

        if req.request_id == "http_request_test":
            from moosync_edk import http_request
            try:
                http_request("https://example.com", method="POST", headers={"X-Test": "Value"}, body=b"123", timeout_ms=5000)
            except Exception:
                pass
            return extensions_pb2.CustomRequestResponse()

        if req.request_id == "http_batch_get_test":
            from moosync_edk import http_batch_get
            try:
                http_batch_get(["https://example.com/1", "https://example.com/2"])
            except Exception:
                pass
            return extensions_pb2.CustomRequestResponse()

        if req.request_id == "http_batch_request_test":
            from moosync_edk import http_batch_request, HttpRequest
            try:
                http_batch_request([
                    HttpRequest(url="https://example.com/1", method="GET"),
                    HttpRequest(url="https://example.com/2", method="POST", body=b"456"),
                ])
            except Exception:
                pass
            return extensions_pb2.CustomRequestResponse()

        return extensions_pb2.CustomRequestResponse()

    def on_context_menu_action(self, req):
        if req.action_id == "add_test":
            self.api.add_playlist(songs_pb2.Playlist())
            self.api.add_songs([])
            self.api.add_to_playlist("id", [])
        return extensions_pb2.ContextMenuActionResponse()

    def on_preferences_changed(self, req):
        self.api.get_preference(extensions_pb2.PreferenceData(key="test"))
        self.api.get_secure(extensions_pb2.PreferenceData(key="test"))
        return extensions_pb2.PreferenceChangedResponse()

    def on_queue_changed(self, req):
        self.api.get_queue()
        return extensions_pb2.SongQueueChangedResponse()

    def on_volume_changed(self, req):
        self.api.get_volume()
        return extensions_pb2.VolumeChangedResponse()

    def on_player_state_changed(self, req):
        self.api.get_player_state()
        return extensions_pb2.PlayerStateChangedResponse()

    def on_song_changed(self, req):
        self.api.get_current_song()
        return extensions_pb2.SongChangedResponse()

    def on_seeked(self, req):
        self.api.get_time()
        return extensions_pb2.SeekedResponse()
