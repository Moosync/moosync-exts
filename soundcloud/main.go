package main

import (
	"fmt"
	"strconv"
	"strings"
	"time"

	"github.com/Moosync/extensions-sdk/wasm-extension-go/pkg/api"
	extensions "github.com/moosync/moosync/types/extensions"
	soundcloudapi "github.com/zackradisic/soundcloud-api"
	"google.golang.org/protobuf/types/known/timestamppb"
)

type SoundcloudExtension struct {
	sc *soundcloudapi.API
}

func (s *SoundcloudExtension) PlaybackDetailsRequested(req *extensions.PlaybackDetailsRequestedRequest) (*extensions.PlaybackDetailsRequestedResponse, error) {
	if req == nil || req.Song == nil || req.Song.Song == nil || req.Song.Song.Id == nil {
		return nil, fmt.Errorf("missing song id")
	}

	rawId := strings.TrimPrefix(*req.Song.Song.Id, "app.moosync.soundcloud:")
	id, err := strconv.ParseInt(rawId, 10, 64)
	if err != nil {
		return nil, err
	}

	trackInfo, err := s.getTrackFromId(id)
	if err != nil {
		return nil, err
	}

	api.LogInfo("Track streamable %v", trackInfo.Streamable)

	var redirectUrl string
	streamURL, err := getStreamUrl(trackInfo)
	if err == nil {
		api.LogInfo("streamurl %v", streamURL)
		redirectUrl, err = getDirectURLFromStream(streamURL)
	}
	if err != nil {
		sc, scErr := s.getSc()
		if scErr != nil {
			return nil, scErr
		}
		redirectUrl, err = sc.GetDownloadURL(trackInfo.PermalinkURL, "hls")
		if err != nil {
			return nil, err
		}
	}

	duration := api.DurationToProto(time.Duration(trackInfo.DurationMS) * time.Millisecond)

	var expiresAt *timestamppb.Timestamp
	if timeResp, err := api.SystemTime(&extensions.SystemTimeRequest{}); err == nil && timeResp != nil {
		expiresAt = &timestamppb.Timestamp{
			Seconds: int64(timeResp.GetTimestamp()) + 20*60,
		}
	}

	return &extensions.PlaybackDetailsRequestedResponse{
		Duration:  duration,
		Url:       redirectUrl,
		ExpiresAt: expiresAt,
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
	if strings.HasPrefix(id, "app.moosync.soundcloud:") {
		cacheKey := PlaylistSongsCacheKey(id)
		if cached, err := api.GetCache[*extensions.RequestedPlaylistSongsResponse](cacheKey); err == nil && cached != nil {
			return cached, nil
		}

		playlistId, err := strconv.ParseInt(strings.TrimPrefix(id, "app.moosync.soundcloud:"), 10, 64)
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
