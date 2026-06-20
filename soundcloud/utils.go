package main

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"time"

	"github.com/Moosync/extensions-sdk/wasm-extension-go/pkg/api"
	"github.com/Ovenoboyo/scdl/v2/pkg/soundcloud"
	"github.com/antchfx/htmlquery"
	songs "github.com/moosync/moosync/types/songs"
	soundcloudapi "github.com/zackradisic/soundcloud-api"
)

func ptr[T any](v T) *T {
	return &v
}

func scTracksToSong(track soundcloudapi.Track) *songs.Song {
	duration := api.DurationToProto(time.Duration(track.DurationMS) * time.Millisecond)
	dateAdded := int64(api.SystemTime())
	return &songs.Song{
		Song: &songs.InnerSong{
			Id:                ptr(strconv.Itoa(int(track.ID))),
			Title:             ptr(track.Title),
			Date:              ptr(track.DisplayDate),
			Duration:          duration,
			Type:              songs.SongType_URL, // Assuming URL type for HLS, or check if HLS enum exists
			Url:               ptr(track.PermalinkURL),
			SongCoverPathHigh: ptr(track.ArtworkURL),
			PlaybackUrl:       ptr("extension://moosync.soundcloud/" + strconv.Itoa(int(track.ID))),
			SongCoverPathLow:  ptr(track.ArtworkURL),
			DateAdded:         &dateAdded,
		},
		Album: nil,
		Artists: []*songs.Artist{{
			ArtistName:      ptr(fmt.Sprintf("%s %s", track.User.FirstName, track.User.LastName)),
			ArtistCoverpath: ptr(track.User.AvatarURL),
		}},
		Genre: []*songs.Genre{},
	}
}

func scPlaylistToPlaylists(playlist soundcloudapi.Playlist) *songs.Playlist {
	return &songs.Playlist{
		PlaylistId:        ptr(strconv.Itoa(int(playlist.ID))),
		PlaylistName:      playlist.Title,
		PlaylistCoverpath: ptr(playlist.ArtworkURL),
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
