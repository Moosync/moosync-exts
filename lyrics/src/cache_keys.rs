pub const LYRICS_ID_PREFIX: &str = "lyrics:id:";
pub const LYRICS_TRACK_PREFIX: &str = "lyrics:track:";

pub fn by_id(id: &str) -> String {
    format!("{LYRICS_ID_PREFIX}{id}")
}

pub fn by_track(title: &str, artist: &str) -> String {
    format!(
        "{LYRICS_TRACK_PREFIX}{}:{}",
        title.trim().to_lowercase(),
        artist.trim().to_lowercase()
    )
}
