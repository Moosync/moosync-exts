package main

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/Moosync/extensions-sdk/wasm-extension-go/pkg/api"
	extensions "github.com/moosync/moosync/types/extensions"
	songs "github.com/moosync/moosync/types/songs"
	soundcloudapi "github.com/zackradisic/soundcloud-api"
)

type SoundcloudExtension struct {
	api.DefaultExtension
	sc *soundcloudapi.API
}

func (*SoundcloudExtension) GetProviderScopes() ([]extensions.ExtensionProviderScope, error) {
	return []extensions.ExtensionProviderScope{extensions.ExtensionProviderScope_SEARCH, extensions.ExtensionProviderScope_SONG_FROM_URL, extensions.ExtensionProviderScope_PLAYLIST_FROM_URL, extensions.ExtensionProviderScope_PLAYBACK_DETAILS, extensions.ExtensionProviderScope_PLAYLIST_SONGS}, nil
}

func (s *SoundcloudExtension) HandleCustomRequest(req *extensions.CustomRequest) (ret *extensions.CustomRequestResponse, err error) {
	id, err := strconv.ParseInt(strings.Replace(req.RequestId, "extension://moosync.soundcloud/", "", 1), 10, 64)
	if err != nil {
		return
	}

	trackInfo, err := s.getTrackFromId(id)
	if err != nil {
		return
	}

	api.LogInfo("Track streamable %v", trackInfo.Streamable)

	streamURL, err := getStreamUrl(trackInfo)
	if err != nil {
		return
	}

	api.LogInfo("streamurl %v", streamURL)

	redirectUrl, err := getDirectURLFromStream(streamURL)
	if err != nil {
		return
	}

	return &extensions.CustomRequestResponse{
		MimeType:    ptr(trackInfo.Media.Transcodings[0].Format.MimeType),
		RedirectUrl: ptr(redirectUrl),
	}, nil
}

func (s *SoundcloudExtension) Search(req *extensions.RequestedSearchResultRequest) (res *songs.SearchResult, err error) {
	api.LogInfo("Got here")
	term := req.Query
	res = &songs.SearchResult{}
	resp, err := s.sc.Search(soundcloudapi.SearchOptions{
		Query:  term,
		Limit:  50,
		Offset: 0,
	})

	if err != nil {
		return
	}

	tracks, err := resp.GetTracks()
	if err != nil {
		return
	}

	for _, t := range tracks {
		res.Songs = append(res.Songs, scTracksToSong(t))
	}

	playlists, err := resp.GetPlaylists()
	if err != nil {
		return
	}

	for _, t := range playlists {
		res.Playlists = append(res.Playlists, scPlaylistToPlaylists(t))
	}

	return
}

func (s *SoundcloudExtension) GetSongFromURL(req *extensions.RequestedSongFromUrlRequest) (*extensions.RequestedSongFromUrlResponse, error) {
	url := req.Url
	if s.sc.IsURL(url) {
		tracks, err := s.sc.GetTrackInfo(soundcloudapi.GetTrackInfoOptions{
			URL: url,
		})

		if err != nil {
			return nil, err
		}

		if len(tracks) == 0 {
			return nil, fmt.Errorf("Could not fetch details for the URL")
		}

		return &extensions.RequestedSongFromUrlResponse{Song: scTracksToSong(tracks[0])}, nil
	}

	return nil, fmt.Errorf("Not a valid soundcloud URL")
}

func (s *SoundcloudExtension) GetPlaylistFromURL(req *extensions.RequestedPlaylistFromUrlRequest) (*extensions.RequestedPlaylistFromUrlResponse, error) {
	url := req.Url
	if s.sc.IsURL(url) {
		playlist, err := s.sc.GetPlaylistInfo(url)
		if err != nil {
			return nil, err
		}

		return &extensions.RequestedPlaylistFromUrlResponse{Playlist: scPlaylistToPlaylists(playlist)}, nil

	}
	return nil, fmt.Errorf("Not a valid soundcloud URL")
}

func (s *SoundcloudExtension) GetPlaylistContent(req *extensions.RequestedPlaylistSongsRequest) (ret *extensions.RequestedPlaylistSongsResponse, err error) {
	id := req.Id
	if strings.HasPrefix(id, "moosync.soundcloud:") {
		playlistId, err := strconv.ParseInt(strings.TrimPrefix(id, "moosync.soundcloud:"), 10, 64)
		if err != nil {
			return ret, err
		}

		playlist, err := s.sc.GetPlaylistInfo(fmt.Sprintf("https://api.soundcloud.com/playlists/%d", playlistId))
		if err != nil {
			return nil, err
		}

		ret = &extensions.RequestedPlaylistSongsResponse{}
		for _, t := range playlist.Tracks {
			ret.Songs = append(ret.Songs, scTracksToSong(t))
		}

		return ret, nil
	}

	return nil, fmt.Errorf("Not a playlist reported by this extension")
}

func NewSoundcloudExtension() (SoundcloudExtension, error) {
	sc, err := soundcloudapi.New(soundcloudapi.APIOptions{})
	if err != nil {
		return SoundcloudExtension{}, err
	}

	return SoundcloudExtension{
		sc: sc,
	}, nil
}

func main() {}

//go:wasmexport entry
func entry() {
	api.EnableHttp()

	extension, err := NewSoundcloudExtension()
	if err != nil {
		api.LogError("Failed to create extension: %v", err)
		panic(err)
	}
	api.RegisterExtension(&extension)
}
