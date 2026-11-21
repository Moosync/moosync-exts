package main

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/Moosync/extensions-sdk/wasm-extension-go/pkg/api"
	extensions "github.com/moosync/moosync/types/extensions"
	soundcloudapi "github.com/zackradisic/soundcloud-api"
)

type SoundcloudExtension struct {
	sc *soundcloudapi.API
}

func (s *SoundcloudExtension) CustomRequest(req *extensions.CustomRequest) (ret *extensions.CustomRequestResponse, err error) {
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

func (s *SoundcloudExtension) RequestedSearchResult(req *extensions.RequestedSearchResultRequest) (res *extensions.RequestedSearchResultResponse, err error) {
	cacheKey := SearchCacheKey(req.Query)
	if cached, err := api.GetCache[*extensions.RequestedSearchResultResponse](cacheKey); err == nil && cached != nil {
		return cached, nil
	}

	term := req.Query
	res = &extensions.RequestedSearchResultResponse{}
	sc, err := s.getSc()
	if err != nil {
		return nil, err
	}
	resp, err := sc.Search(soundcloudapi.SearchOptions{
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

	ttl := uint64(7200)
	_ = api.SetCache(cacheKey, res, &ttl)

	return
}

func (s *SoundcloudExtension) RequestedPlaylistSongs(req *extensions.RequestedPlaylistSongsRequest) (ret *extensions.RequestedPlaylistSongsResponse, err error) {
	id := req.Id
	if strings.HasPrefix(id, "moosync.soundcloud:") {
		cacheKey := PlaylistSongsCacheKey(id)
		if cached, err := api.GetCache[*extensions.RequestedPlaylistSongsResponse](cacheKey); err == nil && cached != nil {
			return cached, nil
		}

		playlistId, err := strconv.ParseInt(strings.TrimPrefix(id, "moosync.soundcloud:"), 10, 64)
		if err != nil {
			return ret, err
		}

		sc, err := s.getSc()
		if err != nil {
			return nil, err
		}

		playlist, err := sc.GetPlaylistInfo(fmt.Sprintf("https://api.soundcloud.com/playlists/%d", playlistId))
		if err != nil {
			return nil, err
		}

		ret = &extensions.RequestedPlaylistSongsResponse{}
		for _, t := range playlist.Tracks {
			ret.Songs = append(ret.Songs, scTracksToSong(t))
		}

		ttl := uint64(3600)
		_ = api.SetCache(cacheKey, ret, &ttl)

		return ret, nil
	}

	return nil, fmt.Errorf("Not a playlist reported by this extension")
}

func (s *SoundcloudExtension) getSc() (*soundcloudapi.API, error) {
	if s.sc == nil {
		sc, err := soundcloudapi.New(soundcloudapi.APIOptions{})
		if err != nil {
			return nil, err
		}
		s.sc = sc
	}
	return s.sc, nil
}

func NewSoundcloudExtension() (SoundcloudExtension, error) {
	return SoundcloudExtension{}, nil
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
