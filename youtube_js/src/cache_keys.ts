export const SEARCH_PREFIX = "search:";
export const PLAYBACK_PREFIX = "playback:";
export const PLAYLIST_SONGS_PREFIX = "playlist_songs:";
export const ARTIST_SONGS_PREFIX = "artist_songs:";
export const ALBUM_SONGS_PREFIX = "album_songs:";
export const USER_PLAYLISTS_KEY = "user_playlists";
export const RECOMMENDATIONS_PREFIX = "recommendations:";

export function searchCacheKey(query: string): string {
  return `${SEARCH_PREFIX}${query.trim().toLowerCase()}`;
}

export function playbackCacheKey(videoId: string): string {
  return `${PLAYBACK_PREFIX}${videoId}`;
}

export function playlistSongsCacheKey(playlistId: string): string {
  return `${PLAYLIST_SONGS_PREFIX}${playlistId}`;
}

export function artistSongsCacheKey(artistId: string): string {
  return `${ARTIST_SONGS_PREFIX}${artistId}`;
}

export function albumSongsCacheKey(albumId: string): string {
  return `${ALBUM_SONGS_PREFIX}${albumId}`;
}

export function userPlaylistsCacheKey(): string {
  return USER_PLAYLISTS_KEY;
}

export function recommendationsCacheKey(authStatus: string): string {
  return `${RECOMMENDATIONS_PREFIX}${authStatus}`;
}
