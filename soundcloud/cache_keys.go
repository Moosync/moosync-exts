package main

import "strings"

const (
	SearchKeyPrefix        = "search:"
	PlaylistSongsKeyPrefix = "playlist_songs:"
)

func SearchCacheKey(query string) string {
	return SearchKeyPrefix + strings.ToLower(strings.TrimSpace(query))
}

func PlaylistSongsCacheKey(id string) string {
	return PlaylistSongsKeyPrefix + id
}
