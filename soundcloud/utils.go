package main

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"

	"github.com/Moosync/extensions-sdk/wasm-extension-go/pkg/api"
	"github.com/Moosync/extensions-sdk/wasm-extension-go/pkg/types"
	"github.com/Ovenoboyo/scdl/v2/pkg/soundcloud"
	"github.com/antchfx/htmlquery"
	soundcloudapi "github.com/zackradisic/soundcloud-api"
)

func scTracksToSong(track soundcloudapi.Track) types.Song {
	duration := float64(track.DurationMS) / 1000
	dateAdded := int64(api.SystemTime())
	return types.Song{
		QueryableSong: types.QueryableSong{
			ID:                strconv.Itoa(int(track.ID)),
			Title:             track.Title,
			Date:              track.DisplayDate,
			Duration:          &duration,
			Type:              types.SongTypeHLS,
			URL:               track.PermalinkURL,
			SongCoverPathHigh: track.ArtworkURL,
			PlaybackURL:       "extension://moosync.soundcloud/" + strconv.Itoa(int(track.ID)),
			SongCoverPathLow:  track.ArtworkURL,
			DateAdded:         &dateAdded,
		},
		Album: nil,
		Artists: []types.QueryableArtist{types.QueryableArtist{
			ArtistName:      fmt.Sprintf("%s %s", track.User.FirstName, track.User.LastName),
			ArtistCoverPath: track.User.AvatarURL,
		}},
		Genre: []types.QueryableGenre{},
	}
}

func scPlaylistToPlaylists(playlist soundcloudapi.Playlist) types.QueryablePlaylist {
	return types.QueryablePlaylist{
		PlaylistID:    strconv.Itoa(int(playlist.ID)),
		PlaylistName:  playlist.Title,
		PlaylistCover: playlist.ArtworkURL,
	}
}

func (s *SoundcloudExtension) getTrackFromId(id int64) (*soundcloudapi.Track, error) {
	info, err := s.sc.GetTrackInfo(soundcloudapi.GetTrackInfoOptions{
		ID: []int64{id},
	})

	if err != nil {
		return nil, err
	}

	if len(info) != 1 {
		return nil, fmt.Errorf("Somethign went wrong. Len of response is not 1")
	}

	trackInfo := info[0]
	if len(trackInfo.Media.Transcodings) == 0 {
		return nil, fmt.Errorf("Song doesn't have a streamable transcoding")
	}

	return &trackInfo, nil
}

func getStreamUrl(trackInfo *soundcloudapi.Track) (string, error) {
	client := soundcloud.NewClient("", nil)

	req, err := http.NewRequest("GET", trackInfo.PermalinkURL, nil)
	if err != nil {
		return "", err
	}

	// set Non Hacker User Agent
	req.Header.Set("Accept", client.UserAgent)

	resp, err := client.Client.Do(req)
	if err != nil {
		return "", err
	}

	// parse html
	doc, err := htmlquery.Parse(resp.Body)
	if err != nil {
		return "", err
	}

	streamURL, err := client.ConstructStreamURL(doc, soundcloud.StreamTypeHLS)
	if err != nil {
		return "", err
	}

	return streamURL, nil
}

func getDirectURLFromStream(playlistUrl string) (string, error) {
	playlistUrlResp, err := http.Get(playlistUrl)
	if err != nil {
		return "", err
	}

	defer playlistUrlResp.Body.Close()

	body, err := io.ReadAll(playlistUrlResp.Body)
	if err != nil {
		return "", err
	}

	api.LogInfo("got resp %s", string(body))

	var audioResp soundcloud.AudioLink
	err = json.Unmarshal(body, &audioResp)
	if err != nil {
		return "", err
	}

	return audioResp.URL, nil
}
