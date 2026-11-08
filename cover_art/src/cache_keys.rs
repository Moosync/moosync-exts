pub const COVER_SONG_PREFIX: &str = "cover:song:";
pub const COVER_ALBUM_PREFIX: &str = "cover:album:";
pub const COVER_ARTIST_PREFIX: &str = "cover:artist:";

pub fn song(title: &str, artist: &str) -> String {
    format!(
        "{COVER_SONG_PREFIX}{}:{}",
        title.trim().to_lowercase(),
        artist.trim().to_lowercase()
    )
}

pub fn album(album: &str, artist: &str) -> String {
    format!(
        "{COVER_ALBUM_PREFIX}{}:{}",
        album.trim().to_lowercase(),
        artist.trim().to_lowercase()
    )
}

pub fn artist(artist: &str) -> String {
    format!("{COVER_ARTIST_PREFIX}{}", artist.trim().to_lowercase())
}
