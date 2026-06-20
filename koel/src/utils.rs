use moosync_edk::extensions_proto::struct_proto::google::protobuf::Value as ProtoValue;
use moosync_edk::{Album, Artist, InnerSong, Playlist};
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

/// Parse next_page_token from Option<String>, with a default value if parsing fails.
pub fn parse_next_page_token(token: &Option<String>, default: u64) -> u64 {
    token
        .as_ref()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

/// Generate a next_page_token as String, or None if there are no more pages.
pub fn make_next_page_token(page: u64, has_more: bool) -> Option<String> {
    if has_more {
        Some((page + 1).to_string())
    } else {
        None
    }
}

/// Parse a vector of Album from a serde_json::Value (usually from API response)
pub fn parse_albums(value: &Value) -> Vec<Album> {
    value
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|ka| Album {
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

/// Parse a vector of Artist from a serde_json::Value (usually from API response)
pub fn parse_artists(value: &Value) -> Vec<Artist> {
    value
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|ka| Artist {
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

/// Parse a vector of Playlist from a serde_json::Value (usually from API response)
pub fn parse_playlists(value: &Value) -> Vec<Playlist> {
    value
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|pl| Playlist {
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

/// Parse a vector of InnerSong from a serde_json::Value (for search results)
pub fn parse_queryable_songs(value: &Value) -> Vec<InnerSong> {
    value
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|ks| InnerSong {
            id: ks.get("id").and_then(|v| v.as_str().map(|s| s.to_string())),
            title: ks
                .get("title")
                .and_then(|v| v.as_str().map(|s| s.to_string())),
            lyrics: ks
                .get("lyrics")
                .and_then(|v| v.as_str().map(|s| s.to_string())),
            duration: f64_to_duration(ks.get("length").and_then(|v| v.as_f64())),
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

pub fn f64_to_duration(secs: Option<f64>) -> Option<moosync_edk::duration_proto::google::protobuf::Duration> {
    secs.map(|s| {
        moosync_edk::duration_to_proto(std::time::Duration::from_secs_f64(s))
    })
}

pub fn google_value_to_serde(v: ProtoValue) -> serde_json::Value {
    use moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind;
    match v.kind {
        Some(Kind::NullValue(_)) => serde_json::Value::Null,
        Some(Kind::NumberValue(n)) => serde_json::Value::Number(
            serde_json::Number::from_f64(n).unwrap_or(serde_json::Number::from(0)),
        ),
        Some(Kind::StringValue(s)) => serde_json::Value::String(s),
        Some(Kind::BoolValue(b)) => serde_json::Value::Bool(b),
        Some(Kind::StructValue(s)) => google_struct_to_serde(s),
        Some(Kind::ListValue(l)) => {
            serde_json::Value::Array(l.values.into_iter().map(google_value_to_serde).collect())
        }
        None => serde_json::Value::Null,
    }
}

pub fn google_value_to_string(val: ProtoValue) -> Option<String> {
    use moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind;
    match val.kind {
        Some(Kind::StringValue(s)) => Some(s),
        _ => None,
    }
}

pub fn google_struct_to_serde(
    s: moosync_edk::extensions_proto::struct_proto::google::protobuf::Struct,
) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in s.fields {
        map.insert(k, google_value_to_serde(v));
    }
    serde_json::Value::Object(map)
}
