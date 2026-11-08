pub const SEARCH_PREFIX: &str = "search:";
pub const USER_PLAYLISTS_KEY: &str = "user_playlists";
pub const PLAYLIST_SONGS_PREFIX: &str = "playlist_songs:";
pub const ALBUM_SONGS_PREFIX: &str = "album_songs:";
pub const ARTIST_SONGS_PREFIX: &str = "artist_songs:";
pub const RECOMMENDATIONS_KEY: &str = "recommendations";

pub fn search(query: &str) -> String {
    format!("{SEARCH_PREFIX}{}", query.trim().to_lowercase())
}

pub fn user_playlists() -> String {
    USER_PLAYLISTS_KEY.to_string()
}

pub fn playlist_songs(id: &str, page_token: Option<&str>) -> String {
    format!(
        "{PLAYLIST_SONGS_PREFIX}{id}:{}",
        page_token.unwrap_or_default()
    )
}

pub fn album_songs(query: &str) -> String {
    format!("{ALBUM_SONGS_PREFIX}{}", query.trim().to_lowercase())
}

pub fn artist_songs(query: &str) -> String {
    format!("{ARTIST_SONGS_PREFIX}{}", query.trim().to_lowercase())
}

pub fn recommendations() -> String {
    RECOMMENDATIONS_KEY.to_string()
}
