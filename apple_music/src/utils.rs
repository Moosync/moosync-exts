#![allow(dead_code)]

use moosync_edk::{
    api::duration_to_proto,
    types::{Album, Artist, InnerSong, Playlist, PreferenceData, Song, SongType},
    MoosyncError, MoosyncResult,
};
use moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind;
use moosync_edk::extensions_proto::struct_proto::google::protobuf::{
    ListValue, Struct as ProtoStruct, Value as ProtoValue,
};
use ramdl::api::{albums, playlists, search, songs};
use regex::Regex;
use serde::de::DeserializeOwned;
use std::sync::LazyLock;
use std::time::Duration;

pub fn google_value_to_serde(v: ProtoValue) -> serde_json::Value {
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

pub fn google_struct_to_serde(s: ProtoStruct) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in s.fields {
        map.insert(k, google_value_to_serde(v));
    }
    serde_json::Value::Object(map)
}

pub fn serde_to_google_value(v: serde_json::Value) -> ProtoValue {
    let kind = match v {
        serde_json::Value::Null => Some(Kind::NullValue(0)),
        serde_json::Value::Bool(b) => Some(Kind::BoolValue(b)),
        serde_json::Value::Number(n) => Some(Kind::NumberValue(n.as_f64().unwrap_or(0.0))),
        serde_json::Value::String(s) => Some(Kind::StringValue(s)),
        serde_json::Value::Array(a) => Some(Kind::ListValue(ListValue {
            values: a.into_iter().map(serde_to_google_value).collect(),
        })),
        serde_json::Value::Object(o) => {
            let fields = o
                .into_iter()
                .map(|(k, v)| (k, serde_to_google_value(v)))
                .collect();
            Some(Kind::StructValue(ProtoStruct { fields }))
        }
    };
    ProtoValue { kind }
}

pub fn cast_prefdata<T: DeserializeOwned>(prefs: PreferenceData) -> MoosyncResult<Option<T>> {
    let value = prefs
        .value
        .map(google_value_to_serde)
        .unwrap_or(serde_json::Value::Null);
    let res: Option<T> =
        serde_json::from_value(value).map_err(|e| MoosyncError::String(e.to_string()))?;
    Ok(res)
}

static SONG_URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://music\.apple\.com/[a-z]{2}/(?:album/[^/]+/|song/[^/]+/)?(?:\d+\?i=(\d+)|(\d+))"#).unwrap()
});

static PLAYLIST_URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://music\.apple\.com/[a-z]{2}/playlist/[^/]+/(pl\.[a-zA-Z0-9]+|pl\.u-[a-zA-Z0-9]+)"#).unwrap()
});

static ALBUM_URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://music\.apple\.com/[a-z]{2}/album/[^/]+/(\d+)(?:\?.*)?$"#).unwrap()
});

static ARTIST_URL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https?://music\.apple\.com/[a-z]{2}/artist/[^/]+/(\d+)"#).unwrap()
});

pub fn extract_song_id(url: &str) -> Option<String> {
    let caps = SONG_URL_REGEX.captures(url)?;
    caps.get(1)
        .or_else(|| caps.get(2))
        .map(|m| m.as_str().to_string())
}

pub fn extract_playlist_id(url: &str) -> Option<String> {
    let caps = PLAYLIST_URL_REGEX.captures(url)?;
    caps.get(1).map(|m| m.as_str().to_string())
}

pub fn extract_album_id(url: &str) -> Option<String> {
    if url.contains("?i=") {
        return None;
    }
    let caps = ALBUM_URL_REGEX.captures(url)?;
    caps.get(1).map(|m| m.as_str().to_string())
}

pub fn extract_artist_id(url: &str) -> Option<String> {
    let caps = ARTIST_URL_REGEX.captures(url)?;
    caps.get(1).map(|m| m.as_str().to_string())
}

pub fn format_artwork_url(template: &str, size: u32) -> String {
    template
        .replace("{w}", &size.to_string())
        .replace("{h}", &size.to_string())
        .replace("{f}", "jpg")
        .replace("{c}", "")
}

pub fn song_from_ramdl(song: &songs::Songs) -> Song {
    let id = format!("applemusic:{}", song.id);
    let attr = &song.attributes;
    let cover = format_artwork_url(&attr.artwork.url, 600);
    let preview_url = attr.previews.first().map(|p| p.url.clone());
    let duration = Some(duration_to_proto(Duration::from_millis(attr.duration_in_millis)));

    let album = attr.album_name.as_ref().map(|name| Album {
        album_name: Some(name.clone()),
        album_artist: Some(attr.artist_name.clone()),
        album_coverpath_high: Some(cover.clone()),
        ..Default::default()
    });

    let artist = Artist {
        artist_name: Some(attr.artist_name.clone()),
        artist_coverpath: Some(cover.clone()),
        ..Default::default()
    };

    Song {
        song: Some(InnerSong {
            id: Some(id.clone()),
            title: Some(attr.name.clone()),
            duration,
            r#type: SongType::Url as i32,
            url: Some(attr.url.clone()),
            playback_url: preview_url.clone().or_else(|| Some(attr.url.clone())),
            song_cover_path_high: Some(cover),
            track_no: attr.track_number.map(|n| n as f64),
            date: attr.release_date.clone(),
            ..Default::default()
        }),
        album,
        artists: vec![artist],
        ..Default::default()
    }
}

pub fn song_from_search(song: &search::Song) -> Song {
    let id = format!("applemusic:{}", song.id);
    let attr = &song.attributes;
    let cover = format_artwork_url(&attr.artwork.url, 600);
    let preview_url = attr.previews.first().map(|p| p.url.clone());
    let duration = duration_to_proto(Duration::from_millis(attr.duration_in_millis as u64));

    let album = Album {
        album_name: Some(attr.album_name.clone()),
        album_artist: Some(attr.artist_name.clone()),
        album_coverpath_high: Some(cover.clone()),
        ..Default::default()
    };

    let artist = Artist {
        artist_name: Some(attr.artist_name.clone()),
        artist_coverpath: Some(cover.clone()),
        ..Default::default()
    };

    Song {
        song: Some(InnerSong {
            id: Some(id.clone()),
            title: Some(attr.name.clone()),
            duration: Some(duration),
            r#type: SongType::Url as i32,
            url: Some(attr.url.clone()),
            playback_url: preview_url.clone().or_else(|| Some(attr.url.clone())),
            song_cover_path_high: Some(cover),
            track_no: Some(attr.track_number as f64),
            date: Some(attr.release_date.clone()),
            ..Default::default()
        }),
        album: Some(album),
        artists: vec![artist],
        ..Default::default()
    }
}

pub fn album_from_ramdl(album: &albums::Albums) -> Album {
    let id = format!("applemusic:album:{}", album.id);
    let attr = album.attributes.as_ref();
    let name = attr.map(|a| a.name.clone()).unwrap_or_default();
    let artist = attr.map(|a| a.artist_name.clone());
    let cover = attr.map(|a| format_artwork_url(&a.artwork.url, 600));
    let count = attr.map(|a| a.track_count as f64).unwrap_or(0.0);
    let year = attr.and_then(|a| a.release_date.clone());

    Album {
        album_id: Some(id),
        album_name: Some(name),
        album_artist: artist,
        album_coverpath_high: cover,
        album_song_count: count,
        year,
        extension: Some("moosync.apple.music".into()),
        ..Default::default()
    }
}

pub fn album_from_search(album: &search::Album) -> Album {
    let id = format!("applemusic:album:{}", album.id);
    let attr = &album.attributes;
    let cover = format_artwork_url(&attr.artwork.url, 600);

    Album {
        album_id: Some(id),
        album_name: Some(attr.name.clone()),
        album_artist: Some(attr.artist_name.clone()),
        album_coverpath_high: Some(cover),
        album_song_count: attr.track_count as f64,
        year: Some(attr.release_date.clone()),
        extension: Some("moosync.apple.music".into()),
        ..Default::default()
    }
}

pub fn artist_from_search(artist: &search::Artist) -> Artist {
    let id = format!("applemusic:artist:{}", artist.id);
    let cover = artist.attributes.artwork.as_ref().map(|a| format_artwork_url(&a.url, 600));

    Artist {
        artist_id: Some(id),
        artist_name: Some(artist.attributes.name.clone()),
        artist_coverpath: cover,
        extension: Some("moosync.apple.music".into()),
        ..Default::default()
    }
}

pub fn playlist_from_ramdl(playlist: &playlists::Playlists) -> Playlist {
    let id = format!("applemusic:playlist:{}", playlist.id);
    let attr = &playlist.attributes;
    let cover = attr.artwork.as_ref().map(|a| format_artwork_url(&a.url, 600));
    let desc = attr
        .description
        .as_ref()
        .and_then(|d| d.short.clone().or_else(|| d.standard.clone()));

    Playlist {
        playlist_id: Some(id),
        playlist_name: attr.name.clone(),
        playlist_coverpath: cover,
        playlist_desc: desc,
        extension: Some("moosync.apple.music".into()),
        ..Default::default()
    }
}

pub fn playlist_from_search(playlist: &search::Playlist) -> Playlist {
    let id = format!("applemusic:playlist:{}", playlist.id);
    let attr = &playlist.attributes;
    let cover = attr.artwork.as_ref().map(|a| format_artwork_url(&a.url, 600));
    let desc = attr.description.as_ref().and_then(|d| d.short.clone().or_else(|| d.standard.clone()));

    Playlist {
        playlist_id: Some(id),
        playlist_name: attr.name.clone(),
        playlist_coverpath: cover,
        playlist_desc: desc,
        extension: Some("moosync.apple.music".into()),
        ..Default::default()
    }
}
