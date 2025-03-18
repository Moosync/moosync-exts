package main

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/Moosync/extensions-sdk/wasm-extension-go/pkg/api"
	"github.com/Moosync/extensions-sdk/wasm-extension-go/pkg/types"
	soundcloudapi "github.com/zackradisic/soundcloud-api"
)

type SoundcloudExtension struct {
	api.DefaultExtension
	sc *soundcloudapi.API
}

func (*SoundcloudExtension) GetProviderScopes() ([]types.ExtensionProviderScope, error) {
	return []types.ExtensionProviderScope{types.ScopeSearch, types.ScopeSongFromUrl, types.ScopePlaylistFromUrl, types.ScopePlaybackDetails, types.ScopePlaylistSongs}, nil
}

func (s *SoundcloudExtension) HandleCustomRequest(url string) (ret types.CustomRequestReturnType, err error) {
	id, err := strconv.ParseInt(strings.Replace(url, "extension://moosync.soundcloud/", "", 1), 10, 64)
	if err != nil {
		return
	}

	trackInfo, err := s.getTrackFromId(id)
	if err != nil {
		return
	}

	streamURL, err := getStreamUrl(trackInfo)
	if err != nil {
		return
	}

	api.LogInfo("streamurl %v", streamURL)

	url, err = getDirectURLFromStream(streamURL)
	if err != nil {
		return
	}

	return types.CustomRequestReturnType{
		MimeType:    trackInfo.Media.Transcodings[0].Format.MimeType,
		RedirectURL: url,
	}, nil
}

func (s *SoundcloudExtension) Search(term string) (res types.SearchResult, err error) {
	api.LogInfo("Got here")
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

func (s *SoundcloudExtension) GetSongFromURL(url string) (types.Song, error) {
	if s.sc.IsURL(url) {
		tracks, err := s.sc.GetTrackInfo(soundcloudapi.GetTrackInfoOptions{
			URL: url,
		})

		if err != nil {
			return types.Song{}, err
		}

		if len(tracks) == 0 {
			return types.Song{}, fmt.Errorf("Could not fetch details for the URL")
		}

		return scTracksToSong(tracks[0]), nil
	}

	return types.Song{}, fmt.Errorf("Not a valid soundcloud URL")
}

func (s *SoundcloudExtension) GetPlaylistFromURL(url string) (types.QueryablePlaylist, error) {
	if s.sc.IsURL(url) {
		playlist, err := s.sc.GetPlaylistInfo(url)
		if err != nil {
			return types.QueryablePlaylist{}, err
		}

		return scPlaylistToPlaylists(playlist), nil

	}
	return types.QueryablePlaylist{}, fmt.Errorf("Not a valid soundcloud URL")
}

func (s *SoundcloudExtension) GetPlaylistContent(id string, nextPageToken string) (ret []types.Song, err error) {
	if strings.HasPrefix(id, "moosync.soundcloud:") {
		playlistId, err := strconv.ParseInt(strings.TrimPrefix(id, "moosync.soundcloud:"), 10, 64)
		if err != nil {
			return ret, err
		}

		playlist, err := s.sc.GetPlaylistInfo(fmt.Sprintf("https://api.soundcloud.com/playlists/%d", playlistId))
		if err != nil {
			return ret, err
		}

		for _, t := range playlist.Tracks {
			ret = append(ret, scTracksToSong(t))
		}

		return ret, nil
	}

	return ret, fmt.Errorf("Not a playlist reported by this extension")
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
