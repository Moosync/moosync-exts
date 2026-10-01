package main

import (
	"fmt"
	"strings"

	extensions "github.com/moosync/moosync/types/extensions"
	preferences "github.com/moosync/moosync/types/preferences"
	songs "github.com/moosync/moosync/types/songs"

	"github.com/Moosync/extensions-sdk/wasm-extension-go/pkg/api"
)

var loggedIn bool = false

type SampleExtension struct {
	api.DefaultExtension
}

func syncAccount(isLoggedIn bool) {
	loggedIn = isLoggedIn
	_ = api.SetAccount(&extensions.ExtensionAccountDetail{
		Id:       "test_account",
		Name:     "Test Account",
		LoggedIn: isLoggedIn,
	})
}

func (s *SampleExtension) PerformAccountLogin(req *extensions.PerformAccountLoginRequest) (string, error) {
	api.LogInfo("perform_account_login called: %v", req)
	if req.GetLoginStatus() {
		_ = api.RegisterOAuth("https://example.com/callback")
		syncAccount(true)
		return "success", nil
	}
	syncAccount(false)
	return "logged_out", nil
}

func (s *SampleExtension) OauthCallback(req *extensions.OauthCallbackRequest) error {
	api.LogInfo("oauth_callback called with code: %s", req.GetCallbackUri())
	syncAccount(true)
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

func stringPtr(s string) *string {
	return &s
}

func mockSampleSong(id string, title string) *songs.Song {
	return &songs.Song{
		Song: &songs.InnerSong{
			Id:    stringPtr(id),
			Title: stringPtr(title),
		},
		Artists: []*songs.Artist{
			{
				ArtistId:   stringPtr("art-" + id),
				ArtistName: stringPtr("Artist for " + title),
			},
		},
		Album: &songs.Album{
			AlbumId:   stringPtr("alb-" + id),
			AlbumName: stringPtr("Album for " + title),
		},
	}
}

func (s *SampleExtension) GetProviderScopes() ([]extensions.ExtensionProviderScope, error) {
	return []extensions.ExtensionProviderScope{
		extensions.ExtensionProviderScope_ACCOUNTS,
		extensions.ExtensionProviderScope_SEARCH,
		extensions.ExtensionProviderScope_PLAYLISTS,
		extensions.ExtensionProviderScope_PLAYLIST_SONGS,
		extensions.ExtensionProviderScope_ARTIST_SONGS,
		extensions.ExtensionProviderScope_ALBUM_SONGS,
	}, nil
}

func (s *SampleExtension) GetPlaylists(req *extensions.RequestedPlaylistsRequest) ([]*songs.Playlist, error) {
	api.LogInfo("get_playlists called")
	return []*songs.Playlist{
		{
			PlaylistId:   stringPtr("ext-playlist-1"),
			PlaylistName: "Extension Playlist 1",
		},
		{
			PlaylistId:   stringPtr("ext-playlist-2"),
			PlaylistName: "Extension Playlist 2",
		},
	}, nil
}

func (s *SampleExtension) GetPlaylistContent(req *extensions.RequestedPlaylistSongsRequest) (*extensions.RequestedPlaylistSongsResponse, error) {
	api.LogInfo("get_playlist_content: %s", req.GetId())
	return &extensions.RequestedPlaylistSongsResponse{
		Songs: []*songs.Song{
			mockSampleSong("pl-song-1", "Playlist Song 1"),
			mockSampleSong("pl-song-2", "Playlist Song 2"),
		},
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
	if strings.EqualFold(strings.TrimSpace(req.GetQuery()), "logged in") && !loggedIn {
		return &songs.SearchResult{
			Songs:     []*songs.Song{},
			Playlists: []*songs.Playlist{},
			Artists:   []*songs.Artist{},
			Albums:    []*songs.Album{},
			Genres:    []*songs.Genre{},
		}, nil
	}
	return &songs.SearchResult{
		Songs: []*songs.Song{
			mockSampleSong("search-song-1", "Search Song 1"),
			mockSampleSong("search-song-2", "Search Song 2"),
		},
		Playlists: []*songs.Playlist{
			{
				PlaylistId:   stringPtr("search-pl-1"),
				PlaylistName: "Search Playlist 1",
			},
			{
				PlaylistId:   stringPtr("search-pl-2"),
				PlaylistName: "Search Playlist 2",
			},
		},
		Artists: []*songs.Artist{
			{
				ArtistId:   stringPtr("search-art-1"),
				ArtistName: stringPtr("Search Artist 1"),
			},
			{
				ArtistId:   stringPtr("search-art-2"),
				ArtistName: stringPtr("Search Artist 2"),
			},
		},
		Albums: []*songs.Album{
			{
				AlbumId:   stringPtr("search-alb-1"),
				AlbumName: stringPtr("Search Album 1"),
			},
			{
				AlbumId:   stringPtr("search-alb-2"),
				AlbumName: stringPtr("Search Album 2"),
			},
		},
		Genres: []*songs.Genre{},
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

	if requestId == "preferences_test" {
		_ = api.RegisterUserPreference([]*preferences.PreferenceItem{})
		_ = api.UnregisterUserPreference([]string{})
	}

	if requestId == "http_get_test" {
		_, _ = api.HttpGet("https://example.com", nil)
	}

	if requestId == "http_request_test" {
		_, _ = api.SendHttpRequest(api.HttpRequest{
			URL:       "https://example.com",
			Method:    "POST",
			Headers:   map[string]string{"X-Test": "Value"},
			Body:      []byte{1, 2, 3},
			TimeoutMs: 5000,
		})
	}

	if requestId == "http_batch_get_test" {
		_, _ = api.BatchHttpGet([]string{"https://example.com/1", "https://example.com/2"}, nil)
	}

	if requestId == "http_batch_request_test" {
		_, _ = api.BatchHttpRequest([]api.HttpRequest{
			{URL: "https://example.com/1", Method: "GET"},
			{URL: "https://example.com/2", Method: "POST", Body: []byte{4, 5, 6}},
		})
	}

	return &extensions.CustomRequestResponse{}, nil
}

func (s *SampleExtension) GetArtistSongs(req *extensions.RequestedArtistSongsRequest) (*extensions.RequestedArtistSongsResponse, error) {
	if artist := req.GetArtist(); artist != nil {
		api.LogInfo("get_artist_songs: %s", artist.GetArtistName())
	}
	return &extensions.RequestedArtistSongsResponse{
		Songs: []*songs.Song{
			mockSampleSong("artist-song-1", "Artist Song 1"),
			mockSampleSong("artist-song-2", "Artist Song 2"),
		},
	}, nil
}

func (s *SampleExtension) GetAlbumSongs(req *extensions.RequestedAlbumSongsRequest) (*extensions.RequestedAlbumSongsResponse, error) {
	if album := req.GetAlbum(); album != nil {
		api.LogInfo("get_album_songs: %s", album.GetAlbumName())
	}
	return &extensions.RequestedAlbumSongsResponse{
		Songs: []*songs.Song{
			mockSampleSong("album-song-1", "Album Song 1"),
			mockSampleSong("album-song-2", "Album Song 2"),
		},
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

func (s *SampleExtension) GetLyrics(req *extensions.RequestedLyricsRequest) (*songs.Lyrics, error) {
	if song := req.GetSong(); song != nil {
		if inner := song.GetSong(); inner != nil {
			api.LogInfo("get_lyrics: %s", inner.GetTitle())
		}
	}
	lines := make([]*songs.LyricLine, 100)
	for i := 0; i < 100; i++ {
		lines[i] = &songs.LyricLine{
			Text:   fmt.Sprintf("Sample Go lyric line %d", i+1),
			TimeMs: int64(i * 500),
		}
	}
	return &songs.Lyrics{
		Lines:    lines,
		IsSynced: true,
	}, nil
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
	syncAccount(false)
}

func main() {}
