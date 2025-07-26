use moosync_edk::{QueryableAlbum, QueryableArtist, QueryablePlaylist, QueryableSong};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct KoelSong {
    pub id: Option<String>,
    pub title: Option<String>,
    pub lyrics: Option<String>,
    pub album_id: Option<String>,
    pub album_name: Option<String>,
    pub album_cover: Option<String>,
    pub artist_id: Option<String>,
    pub artist_name: Option<String>,
    pub track: Option<u32>,
    pub length: Option<f64>,
    pub genre: Option<String>,
    pub year: Option<serde_json::Value>,
}

/// Parse next_page_token from Option<String> using JSON, with a default value if parsing fails.
pub fn parse_next_page_token_json(token: &Option<String>, default: u64) -> u64 {
    token
        .as_ref()
        .and_then(|v| serde_json::from_str::<serde_json::Value>(v).ok())
        .and_then(|val| val.as_u64())
        .unwrap_or(default)
}

/// Generate a next_page_token as JSON string, or None if there are no more pages.
pub fn make_next_page_token_json(page: u64, has_more: bool) -> Option<Value> {
    if has_more {
        Some(serde_json::json!(page + 1))
    } else {
        None
    }
}

/// Parse a vector of QueryableAlbum from a serde_json::Value (usually from API response)
pub fn parse_albums(value: &Value) -> Vec<QueryableAlbum> {
    value
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|ka| QueryableAlbum {
            album_id: ka.get("id").and_then(|v| v.as_str().map(|s| s.to_string())),
            album_name: ka
                .get("name")
                .and_then(|v| v.as_str().map(|s| s.to_string())),
            album_coverpath_high: ka
                .get("cover")
                .and_then(|v| v.as_str().map(|s| s.to_string())),
            year: ka.get("year").map(|v| v.to_string()),
            ..Default::default()
        })
        .collect()
}

/// Parse a vector of QueryableArtist from a serde_json::Value (usually from API response)
pub fn parse_artists(value: &Value) -> Vec<QueryableArtist> {
    value
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|ka| QueryableArtist {
            artist_id: ka.get("id").and_then(|v| v.as_str().map(|s| s.to_string())),
            artist_name: ka
                .get("name")
                .and_then(|v| v.as_str().map(|s| s.to_string())),
            artist_coverpath: ka
                .get("image")
                .and_then(|v| v.as_str().map(|s| s.to_string())),
            ..Default::default()
        })
        .collect()
}

/// Parse a vector of QueryablePlaylist from a serde_json::Value (usually from API response)
pub fn parse_playlists(value: &Value) -> Vec<QueryablePlaylist> {
    value
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|pl| QueryablePlaylist {
            playlist_id: pl.get("id").map(|v| v.to_string().replace('"', "")),
            playlist_name: pl
                .get("name")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default(),
            playlist_coverpath: None,
            playlist_song_count: 0.0,
            playlist_desc: None,
            playlist_path: None,
            extension: Some("koel".to_string()),
            icon: None,
            library_item: Some(true),
            ..Default::default()
        })
        .collect()
}

/// Parse a vector of QueryableSong from a serde_json::Value (for search results)
pub fn parse_queryable_songs(value: &Value) -> Vec<QueryableSong> {
    value
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|ks| QueryableSong {
            _id: ks.get("id").and_then(|v| v.as_str().map(|s| s.to_string())),
            title: ks
                .get("title")
                .and_then(|v| v.as_str().map(|s| s.to_string())),
            lyrics: ks
                .get("lyrics")
                .and_then(|v| v.as_str().map(|s| s.to_string())),
            duration: ks.get("length").and_then(|v| v.as_f64()),
            track_no: ks.get("track").and_then(|v| v.as_u64().map(|t| t as f64)),
            year: ks.get("year").map(|v| v.to_string()),
            provider_extension: Some("koel".to_string()),
            song_cover_path_high: ks
                .get("album_cover")
                .and_then(|v| v.as_str().map(|s| s.to_string())),
            show_in_library: Some(true),
            playback_url: ks
                .get("id")
                .and_then(|id| id.as_str())
                .map(|id| format!("extension://moosync.koel/{id}")),
            ..Default::default()
        })
        .collect()
}
