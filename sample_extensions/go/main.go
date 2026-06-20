package main

import (
	extensions "github.com/moosync/moosync/types/extensions"
	songs "github.com/moosync/moosync/types/songs"
	ui "github.com/moosync/moosync/types/ui"

	"github.com/Moosync/extensions-sdk/wasm-extension-go/pkg/api"
)

type SampleExtension struct {
	api.DefaultExtension
}

func (s *SampleExtension) GetAccounts() ([]*extensions.ExtensionAccountDetail, error) {
	api.LogInfo("get_accounts called")
	acc := "test_account"
	_ = api.UpdateAccounts(&acc)
	return []*extensions.ExtensionAccountDetail{
		{
			Id:       "test_account",
			Name:     "Test Account",
			LoggedIn: true,
		},
	}, nil
}

func (s *SampleExtension) PerformAccountLogin(req *extensions.PerformAccountLoginRequest) (string, error) {
	api.LogInfo("perform_account_login called: %v", req)
	_ = api.RegisterOAuth("https://example.com/callback")
	return "success", nil
}

func (s *SampleExtension) OauthCallback(req *extensions.OauthCallbackRequest) error {
	api.LogInfo("oauth_callback called with code: %s", req.GetCallbackUri())
	return nil
}

func (s *SampleExtension) OnSongAdded(req *extensions.SongAddedRequest) error {
	for _, song := range req.GetSongs() {
		if inner := song.GetSong(); inner != nil {
			api.LogInfo("on_song_added: %s", inner.GetTitle())
		}
	}
	return nil
}

func (s *SampleExtension) OnSongRemoved(req *extensions.SongRemovedRequest) error {
	for _, song := range req.GetSongs() {
		if inner := song.GetSong(); inner != nil {
			api.LogInfo("on_song_removed: %s", inner.GetTitle())
		}
	}
	return nil
}

func (s *SampleExtension) OnPlaylistAdded(req *extensions.PlaylistAddedRequest) error {
	for _, playlist := range req.GetPlaylists() {
		api.LogInfo("on_playlist_added: %s", playlist.GetPlaylistName())
	}
	return nil
}

func (s *SampleExtension) OnPlaylistRemoved(req *extensions.PlaylistRemovedRequest) error {
	for _, playlist := range req.GetPlaylists() {
		api.LogInfo("on_playlist_removed: %s", playlist.GetPlaylistName())
	}
	return nil
}

func (s *SampleExtension) OnPreferencesChanged(req *extensions.PreferenceChangedRequest) error {
	if args := req.GetPreference(); args != nil {
		api.LogInfo("on_preferences_changed: %v", args)
		key := args.GetKey()
		if key != "" {
			_, _ = api.GetPreference(&extensions.PreferenceData{
				Key: key,
			})
			_, _ = api.GetSecure(&extensions.PreferenceData{
				Key: key,
			})
		}
	}
	return nil
}

func (s *SampleExtension) OnQueueChanged(req *extensions.SongQueueChangedRequest) error {
	api.LogInfo("on_queue_changed")
	_, _ = api.GetQueue()
	return nil
}

func (s *SampleExtension) OnVolumeChanged(req *extensions.VolumeChangedRequest) error {
	api.LogInfo("on_volume_changed")
	_, _ = api.GetVolume()
	return nil
}

func (s *SampleExtension) OnPlayerStateChanged(req *extensions.PlayerStateChangedRequest) error {
	api.LogInfo("on_player_state_changed")
	_, _ = api.GetPlayerState()
	return nil
}

func (s *SampleExtension) OnSongChanged(req *extensions.SongChangedRequest) error {
	api.LogInfo("on_song_changed")
	_, _ = api.GetCurrentSong()
	return nil
}

func (s *SampleExtension) OnSeeked(req *extensions.SeekedRequest) error {
	api.LogInfo("on_seeked: %f", req.GetPosition())
	_, _ = api.GetTime()
	return nil
}

func (s *SampleExtension) GetProviderScopes() ([]extensions.ExtensionProviderScope, error) {
	return []extensions.ExtensionProviderScope{extensions.ExtensionProviderScope_ACCOUNTS}, nil
}

func (s *SampleExtension) GetPlaylists(req *extensions.RequestedPlaylistsRequest) ([]*songs.Playlist, error) {
	api.LogInfo("get_playlists called")
	return []*songs.Playlist{}, nil
}

func (s *SampleExtension) GetPlaylistContent(req *extensions.RequestedPlaylistSongsRequest) (*extensions.RequestedPlaylistSongsResponse, error) {
	api.LogInfo("get_playlist_content: %s", req.GetId())
	return &extensions.RequestedPlaylistSongsResponse{
		Songs:         []*songs.Song{},
		NextPageToken: nil,
	}, nil
}

func (s *SampleExtension) GetPlaylistFromURL(req *extensions.RequestedPlaylistFromUrlRequest) (*extensions.RequestedPlaylistFromUrlResponse, error) {
	api.LogInfo("get_playlist_from_url: %s", req.GetUrl())
	// Rust returns None (nil)
	return nil, nil
}

func (s *SampleExtension) GetPlaybackDetails(req *extensions.PlaybackDetailsRequestedRequest) (*extensions.PlaybackDetailsRequestedResponse, error) {
	if song := req.GetSong(); song != nil {
		if inner := song.GetSong(); inner != nil {
			api.LogInfo("get_playback_details: %s", inner.GetId())
		}
	}
	return &extensions.PlaybackDetailsRequestedResponse{
		Duration: 0,
		Url:      "https://example.com/song.mp3",
	}, nil
}

func (s *SampleExtension) Search(req *extensions.RequestedSearchResultRequest) (*songs.SearchResult, error) {
	api.LogInfo("search: %s", req.GetQuery())
	_ = api.SystemTime() // Just to use it? Rust code does.
	_ = api.OpenExternalUrl("https://google.com")
	return &songs.SearchResult{
		Songs:     []*songs.Song{},
		Playlists: []*songs.Playlist{},
		Artists:   []*songs.Artist{},
		Albums:    []*songs.Album{},
		Genres:    []*songs.Genre{},
	}, nil
}

func (s *SampleExtension) GetRecommendations(req *extensions.RequestedRecommendationsRequest) ([]*songs.Song, error) {
	api.LogInfo("get_recommendations")
	return []*songs.Song{}, nil
}

func (s *SampleExtension) GetSongFromURL(req *extensions.RequestedSongFromUrlRequest) (*extensions.RequestedSongFromUrlResponse, error) {
	api.LogInfo("get_song_from_url: %s", req.GetUrl())
	return nil, nil // Rust returns None
}

func (s *SampleExtension) HandleCustomRequest(req *extensions.CustomRequest) (*extensions.CustomRequestResponse, error) {
	requestId := req.GetRequestId()
	api.LogInfo("handle_custom_request: %s", requestId)

	if requestId == "socket_test" {
		if fd := api.OpenSock("/tmp/test.sock"); fd >= 0 {
			_ = api.WriteSock(fd, []byte{1, 2, 3})
			_ = api.ReadSock(fd, 3)
		}
	}

	if requestId == "hash_test" {
		_ = api.Hash(api.HashSHA256, []byte{1, 2, 3})
	}

	if requestId == "preferences_test" {
		_ = api.RegisterUserPreference([]*ui.PreferenceUiData{})
		_ = api.UnregisterUserPreference([]string{})
	}

	return &extensions.CustomRequestResponse{}, nil
}

func (s *SampleExtension) GetArtistSongs(req *extensions.RequestedArtistSongsRequest) (*extensions.RequestedArtistSongsResponse, error) {
	if artist := req.GetArtist(); artist != nil {
		api.LogInfo("get_artist_songs: %s", artist.GetArtistName())
	}
	return &extensions.RequestedArtistSongsResponse{
		Songs: []*songs.Song{},
	}, nil
}

func (s *SampleExtension) GetAlbumSongs(req *extensions.RequestedAlbumSongsRequest) (*extensions.RequestedAlbumSongsResponse, error) {
	if album := req.GetAlbum(); album != nil {
		api.LogInfo("get_album_songs: %s", album.GetAlbumName())
	}
	return &extensions.RequestedAlbumSongsResponse{
		Songs: []*songs.Song{},
	}, nil
}

func (s *SampleExtension) GetSongFromID(req *extensions.RequestedSongFromIdRequest) (*extensions.RequestedSongFromIdResponse, error) {
	api.LogInfo("get_song_from_id: %s", req.GetId())
	return nil, nil
}

func (s *SampleExtension) Scrobble(req *extensions.ScrobbleRequest) error {
	if song := req.GetSong(); song != nil {
		if inner := song.GetSong(); inner != nil {
			api.LogInfo("scrobble: %s", inner.GetTitle())
		}
	}
	return nil
}

func (s *SampleExtension) GetLyrics(req *extensions.RequestedLyricsRequest) (string, error) {
	if song := req.GetSong(); song != nil {
		if inner := song.GetSong(); inner != nil {
			api.LogInfo("get_lyrics: %s", inner.GetTitle())
		}
	}
	return "Sample lyrics", nil
}

func (s *SampleExtension) GetSongContextMenu(req *extensions.RequestedSongContextMenuRequest) ([]*extensions.ContextMenuReturnType, error) {
	api.LogInfo("get_song_context_menu")
	return []*extensions.ContextMenuReturnType{}, nil
}

func (s *SampleExtension) GetPlaylistContextMenu(req *extensions.RequestedPlaylistContextMenuRequest) ([]*extensions.ContextMenuReturnType, error) {
	api.LogInfo("get_playlist_context_menu")
	return []*extensions.ContextMenuReturnType{}, nil
}

func (s *SampleExtension) OnContextMenuAction(req *extensions.ContextMenuActionRequest) error {
	action := req.GetActionId()
	api.LogInfo("on_context_menu_action: %s", action)

	if action == "add_test" {
		_, _ = api.AddPlaylist(&songs.Playlist{})
		_ = api.AddSongs([]*songs.Song{})
		_ = api.AddToPlaylist(&extensions.AddToPlaylistRequest{})
	}
	return nil
}

//go:wasmexport entry
func entry() {
	api.EnableHttp()

	extension := &SampleExtension{}
	api.RegisterExtension(extension)
}

func main() {}
