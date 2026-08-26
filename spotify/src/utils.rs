use std::collections::HashMap;

use moosync_edk::{
    Album, Artist, InnerSong, MoosyncError, MoosyncResult, Playlist, PreferenceData, Song, SongType,
};
use serde::de::DeserializeOwned;

pub fn cast_prefdata<T: DeserializeOwned>(prefs: PreferenceData) -> MoosyncResult<Option<T>> {
    let value = prefs
        .value
        .map(google_value_to_serde)
        .unwrap_or(serde_json::Value::Null);
    let res: Option<T> =
        serde_json::from_value(value).map_err(|e| MoosyncError::String(e.to_string()))?;
    Ok(res)
}

use rspotify::model::{
    FullAlbum, FullTrack, Image, SimplifiedAlbum, SimplifiedArtist, SimplifiedPlaylist,
    SimplifiedTrack,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub limit: u32,
    pub offset: u32,
    pub token: Option<String>,
    pub is_first: bool,
    pub is_valid: bool,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: Default::default(),
            token: Default::default(),
            is_first: Default::default(),
            is_valid: Default::default(),
        }
    }
}

impl Pagination {
    pub fn new_limit(limit: u32, offset: u32) -> Self {
        Pagination {
            limit,
            offset,
            is_first: true,
            is_valid: true,
            ..Default::default()
        }
    }

    pub fn new_token(token: Option<String>) -> Self {
        Pagination {
            token,
            is_first: true,
            is_valid: true,
            ..Default::default()
        }
    }

    pub fn next_page(&self) -> Self {
        Pagination {
            limit: self.limit,
            offset: self.offset + self.limit.max(1),
            token: self.token.clone(),
            is_first: false,
            is_valid: true,
        }
    }

    pub fn next_page_wtoken(&self, token: Option<String>) -> Self {
        Pagination {
            limit: self.limit,
            offset: self.offset + self.limit,
            token,
            is_first: false,
            is_valid: true,
        }
    }

    pub fn invalidate(&mut self) {
        self.is_valid = false;
    }
}

impl TryFrom<Pagination> for serde_json::Value {
    type Error = serde_json::Error;

    fn try_from(value: Pagination) -> Result<Self, Self::Error> {
        serde_json::to_value(value)
    }
}

impl TryFrom<Pagination> for String {
    type Error = MoosyncError;

    fn try_from(value: Pagination) -> Result<Self, Self::Error> {
        let s = serde_json::to_string(&value).map_err(|e| MoosyncError::String(e.to_string()))?;
        Ok(s)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ArtistExtraInfo {
    pub artist_id: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SpotifyExtraInfo {
    pub spotify: ArtistExtraInfo,
}

pub fn parse_playlist(playlist: SimplifiedPlaylist) -> Playlist {
    Playlist {
        playlist_id: Some(playlist.id.to_string()),
        playlist_name: playlist.name,
        playlist_coverpath: playlist.images.first().map(|i| i.url.clone()),
        playlist_song_count: playlist.tracks.total as f64,
        extension: Some("spotify".to_string()),
        ..Default::default()
    }
}

pub fn parse_artist(artist: SimplifiedArtist, images: Option<Vec<Image>>) -> Artist {
    Artist {
        artist_id: Some(
            artist
                .id
                .clone()
                .map(|id| id.to_string())
                .unwrap_or_default(),
        ),
        artist_name: Some(artist.name),
        artist_coverpath: images.and_then(|i| i.first().map(|im| im.url.clone())),
        ..Default::default()
    }
}

pub fn parse_album(album: SimplifiedAlbum) -> Album {
    Album {
        album_id: Some(
            album
                .id
                .clone()
                .map(|id| id.to_string())
                .unwrap_or_default(),
        ),
        album_name: Some(album.name),
        album_artist: album.artists.first().map(|a| a.name.clone()),
        album_coverpath_high: album.images.first().map(|i| i.url.clone()),
        album_coverpath_low: album.images.last().map(|i| i.url.clone()),
        ..Default::default()
    }
}

pub fn parse_track(item: FullTrack) -> Song {
    let id = item.id.clone().map(|i| i.to_string()).unwrap_or_default();
    Song {
        song: Some(InnerSong {
            id: Some(id.clone()),
            title: Some(item.name),
            duration: Some(moosync_edk::duration_to_proto(
                item.duration.to_std().unwrap_or(std::time::Duration::from_secs(0))
            )),
            r#type: SongType::Spotify as i32,
            url: Some(id.clone()),
            song_cover_path_high: item.album.images.first().map(|i| i.url.clone()),
            playback_url: Some(id),
            track_no: Some(item.disc_number as f64),
            ..Default::default()
        }),
        album: if item.album.id.is_some() {
            Some(parse_album(item.album))
        } else {
            None
        },
        artists: item
            .artists
            .into_iter()
            .map(|a| parse_artist(a, None))
            .collect(),
        ..Default::default()
    }
}

pub fn get_full_track_from_simplified(track: SimplifiedTrack) -> FullTrack {
    FullTrack {
        album: track.album.unwrap_or_default(),
        artists: track.artists,
        available_markets: track.available_markets.unwrap_or_default(),
        disc_number: track.disc_number,
        duration: track.duration,
        explicit: track.explicit,
        external_ids: HashMap::new(),
        external_urls: track.external_urls,
        href: track.href,
        id: track.id,
        is_local: track.is_local,
        is_playable: track.is_playable,
        linked_from: track.linked_from,
        restrictions: track.restrictions,
        name: track.name,
        popularity: 0,
        preview_url: track.preview_url,
        track_number: track.track_number,
        r#type: rspotify::model::Type::Track,
    }
}

pub fn simplify_full_album(album: FullAlbum) -> SimplifiedAlbum {
    let album_type = format!("{:?}", album.album_type).to_lowercase();
    SimplifiedAlbum {
        album_group: None,
        album_type: Some(album_type),
        artists: album.artists,
        available_markets: album.available_markets.unwrap_or_default(),
        external_urls: album.external_urls,
        href: Some(album.href),
        id: Some(album.id),
        images: album.images,
        name: album.name,
        release_date: Some(album.release_date),
        release_date_precision: None,
        restrictions: None,
    }
}

pub fn parse_next_page_token(token_raw: Option<String>) -> Pagination {
    token_raw
        .map(|token| serde_json::from_str(&token).unwrap_or_default())
        .unwrap_or_default()
}

pub fn google_value_to_serde(
    v: moosync_edk::extensions_proto::struct_proto::google::protobuf::Value,
) -> serde_json::Value {
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

pub fn google_struct_to_serde(
    s: moosync_edk::extensions_proto::struct_proto::google::protobuf::Struct,
) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in s.fields {
        map.insert(k, google_value_to_serde(v));
    }
    serde_json::Value::Object(map)
}

pub fn serde_to_google_value(
    v: serde_json::Value,
) -> moosync_edk::extensions_proto::struct_proto::google::protobuf::Value {
    use moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind;
    use moosync_edk::extensions_proto::struct_proto::google::protobuf::{
        ListValue, Value as ProtoValue,
    };

    let kind = match v {
        serde_json::Value::Null => Some(Kind::NullValue(0)),
        serde_json::Value::Bool(b) => Some(Kind::BoolValue(b)),
        serde_json::Value::Number(n) => Some(Kind::NumberValue(n.as_f64().unwrap_or(0.0))),
        serde_json::Value::String(s) => Some(Kind::StringValue(s)),
        serde_json::Value::Array(a) => Some(Kind::ListValue(ListValue {
            values: a.into_iter().map(serde_to_google_value).collect(),
        })),
        serde_json::Value::Object(o) => Some(Kind::StructValue(serde_to_google_struct(
            serde_json::Value::Object(o),
        ))),
    };

    ProtoValue { kind }
}

pub fn serde_to_google_struct(
    v: serde_json::Value,
) -> moosync_edk::extensions_proto::struct_proto::google::protobuf::Struct {
    use moosync_edk::extensions_proto::struct_proto::google::protobuf::Struct;

    let fields = match v {
        serde_json::Value::Object(o) => o
            .into_iter()
            .map(|(k, v)| (k, serde_to_google_value(v)))
            .collect(),
        _ => Default::default(),
    };

    Struct { fields }
}
