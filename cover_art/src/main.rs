// Moosync
// Copyright (C) 2024, 2025  Moosync <support@moosync.app>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use futures::executor::block_on;
use moosync_edk::{
    api::{
        get_cache, set_cache, Accounts, ContextMenu, DatabaseEvents, Extension, PlayerEvents,
        PlayerProvider, PreferenceEvents, Provider,
    },
    handler::register_extension,
    info,
    types::{
        requested_cover_art_request::Item, Album, Artist, RequestedCoverArtRequest,
        RequestedCoverArtResponse, Song,
    },
    MoosyncResult,
};
use reqwest::Client;
use serde::Deserialize;
use tracing;
use url::Url;

mod cache_keys;

struct CoverArtExtension {
    client: Client,
}

impl Extension for CoverArtExtension {}
impl PlayerProvider for CoverArtExtension {}
impl PlayerEvents for CoverArtExtension {}
impl PreferenceEvents for CoverArtExtension {}
impl DatabaseEvents for CoverArtExtension {}
impl Accounts for CoverArtExtension {}
impl ContextMenu for CoverArtExtension {}

#[derive(Deserialize, Debug)]
struct DeezerSongAlbum {
    cover_xl: Option<String>,
    cover_big: Option<String>,
    cover_medium: Option<String>,
}

#[derive(Deserialize, Debug)]
struct DeezerSongItem {
    album: Option<DeezerSongAlbum>,
}

#[derive(Deserialize, Debug)]
struct DeezerSongSearchResponse {
    data: Vec<DeezerSongItem>,
}

#[derive(Deserialize, Debug)]
struct DeezerAlbumItem {
    cover_xl: Option<String>,
    cover_big: Option<String>,
    cover_medium: Option<String>,
}

#[derive(Deserialize, Debug)]
struct DeezerAlbumSearchResponse {
    data: Vec<DeezerAlbumItem>,
}

#[derive(Deserialize, Debug)]
struct DeezerArtistItem {
    picture_xl: Option<String>,
    picture_big: Option<String>,
    picture_medium: Option<String>,
}

#[derive(Deserialize, Debug)]
struct DeezerArtistSearchResponse {
    data: Vec<DeezerArtistItem>,
}

#[derive(Deserialize, Debug)]
struct ItunesResultItem {
    #[serde(rename = "artworkUrl100")]
    artwork_url_100: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ItunesSearchResponse {
    results: Vec<ItunesResultItem>,
}

#[derive(Deserialize, Debug)]
struct MusicBrainzReleaseItem {
    id: String,
}

#[derive(Deserialize, Debug)]
struct MusicBrainzSearchResponse {
    releases: Vec<MusicBrainzReleaseItem>,
}

impl CoverArtExtension {
    #[tracing::instrument(level = "debug", skip_all)]
    fn fetch_song_deezer(&self, song: &Song, urls: &mut Vec<String>) {
        let Some(inner) = &song.song else { return; };
        let Some(title) = &inner.title else { return; };
        if title.is_empty() { return; }

        let artist_name = song
            .artists
            .first()
            .and_then(|a| a.artist_name.as_deref())
            .unwrap_or_default();

        let query = format!("{title} {artist_name}");
        let Ok(mut search_url) = Url::parse("https://api.deezer.com/search") else { return; };
        search_url
            .query_pairs_mut()
            .append_pair("q", query.trim())
            .append_pair("limit", "3");

        info!("Querying Deezer song cover: {search_url}");
        let Ok(resp) = block_on(self.client.get(search_url.as_str()).send()) else { return; };
        if !resp.status().is_success() { return; }

        let Ok(parsed) = block_on(resp.json::<DeezerSongSearchResponse>()) else { return; };
        for item in parsed.data {
            if let Some(album) = item.album {
                if let Some(url) = album.cover_xl {
                    urls.push(url);
                } else if let Some(url) = album.cover_big {
                    urls.push(url);
                } else if let Some(url) = album.cover_medium {
                    urls.push(url);
                }
            }
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    fn fetch_song_itunes(&self, song: &Song, urls: &mut Vec<String>) {
        let Some(inner) = &song.song else { return; };
        let Some(title) = &inner.title else { return; };
        if title.is_empty() { return; }

        let artist_name = song
            .artists
            .first()
            .and_then(|a| a.artist_name.as_deref())
            .unwrap_or_default();

        let query = format!("{title} {artist_name}");
        let Ok(mut search_url) = Url::parse("https://itunes.apple.com/search") else { return; };
        search_url
            .query_pairs_mut()
            .append_pair("term", query.trim())
            .append_pair("entity", "musicTrack")
            .append_pair("limit", "3");

        info!("Querying iTunes song cover: {search_url}");
        let Ok(resp) = block_on(self.client.get(search_url.as_str()).send()) else { return; };
        if !resp.status().is_success() { return; }

        let Ok(parsed) = block_on(resp.json::<ItunesSearchResponse>()) else { return; };
        for item in parsed.results {
            if let Some(artwork_100) = item.artwork_url_100 {
                let high_res = artwork_100.replace("100x100bb", "1000x1000bb");
                urls.push(high_res);
                urls.push(artwork_100);
            }
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    fn fetch_album_deezer(&self, album: &Album, urls: &mut Vec<String>) {
        let Some(name) = &album.album_name else { return; };
        if name.is_empty() { return; }

        let artist = album.album_artist.as_deref().unwrap_or_default();
        let query = format!("{name} {artist}");
        let Ok(mut search_url) = Url::parse("https://api.deezer.com/search/album") else { return; };
        search_url
            .query_pairs_mut()
            .append_pair("q", query.trim())
            .append_pair("limit", "3");

        info!("Querying Deezer album cover: {search_url}");
        let Ok(resp) = block_on(self.client.get(search_url.as_str()).send()) else { return; };
        if !resp.status().is_success() { return; }

        let Ok(parsed) = block_on(resp.json::<DeezerAlbumSearchResponse>()) else { return; };
        for item in parsed.data {
            if let Some(url) = item.cover_xl {
                urls.push(url);
            } else if let Some(url) = item.cover_big {
                urls.push(url);
            } else if let Some(url) = item.cover_medium {
                urls.push(url);
            }
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    fn fetch_album_itunes(&self, album: &Album, urls: &mut Vec<String>) {
        let Some(name) = &album.album_name else { return; };
        if name.is_empty() { return; }

        let artist = album.album_artist.as_deref().unwrap_or_default();
        let query = format!("{name} {artist}");
        let Ok(mut search_url) = Url::parse("https://itunes.apple.com/search") else { return; };
        search_url
            .query_pairs_mut()
            .append_pair("term", query.trim())
            .append_pair("entity", "album")
            .append_pair("limit", "3");

        info!("Querying iTunes album cover: {search_url}");
        let Ok(resp) = block_on(self.client.get(search_url.as_str()).send()) else { return; };
        if !resp.status().is_success() { return; }

        let Ok(parsed) = block_on(resp.json::<ItunesSearchResponse>()) else { return; };
        for item in parsed.results {
            if let Some(artwork_100) = item.artwork_url_100 {
                let high_res = artwork_100.replace("100x100bb", "1000x1000bb");
                urls.push(high_res);
                urls.push(artwork_100);
            }
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    fn fetch_album_musicbrainz(&self, album: &Album, urls: &mut Vec<String>) {
        let Some(name) = &album.album_name else { return; };
        if name.is_empty() { return; }

        let artist = album.album_artist.as_deref().unwrap_or_default();
        let query = format!("release:\"{name}\" AND artist:\"{artist}\"");
        let Ok(mut search_url) = Url::parse("https://musicbrainz.org/ws/2/release/") else { return; };
        search_url
            .query_pairs_mut()
            .append_pair("query", query.trim())
            .append_pair("fmt", "json")
            .append_pair("limit", "3");

        info!("Querying MusicBrainz album cover: {search_url}");
        let Ok(resp) = block_on(
            self.client
                .get(search_url.as_str())
                .header("User-Agent", "Moosync/1.0 ( support@moosync.app )")
                .send(),
        ) else { return; };
        if !resp.status().is_success() { return; }

        let Ok(parsed) = block_on(resp.json::<MusicBrainzSearchResponse>()) else { return; };
        for item in parsed.releases {
            urls.push(format!("https://coverartarchive.org/release/{}/front-500", item.id));
            urls.push(format!("https://coverartarchive.org/release/{}/front", item.id));
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    fn fetch_artist_deezer(&self, artist: &Artist, urls: &mut Vec<String>) {
        let Some(name) = &artist.artist_name else { return; };
        if name.is_empty() { return; }

        let Ok(mut search_url) = Url::parse("https://api.deezer.com/search/artist") else { return; };
        search_url
            .query_pairs_mut()
            .append_pair("q", name.trim())
            .append_pair("limit", "3");

        info!("Querying Deezer artist picture: {search_url}");
        let Ok(resp) = block_on(self.client.get(search_url.as_str()).send()) else { return; };
        if !resp.status().is_success() { return; }

        let Ok(parsed) = block_on(resp.json::<DeezerArtistSearchResponse>()) else { return; };
        for item in parsed.data {
            if let Some(url) = item.picture_xl {
                urls.push(url);
            } else if let Some(url) = item.picture_big {
                urls.push(url);
            } else if let Some(url) = item.picture_medium {
                urls.push(url);
            }
        }
    }
}

impl Provider for CoverArtExtension {
    #[tracing::instrument(level = "debug", skip_all)]
    fn requested_cover_art(
        &self,
        req: RequestedCoverArtRequest,
    ) -> MoosyncResult<RequestedCoverArtResponse> {
        let cache_key = match &req.item {
            Some(Item::Song(song)) => {
                let title = song
                    .song
                    .as_ref()
                    .and_then(|s| s.title.as_deref())
                    .unwrap_or_default();
                let artist = song
                    .artists
                    .first()
                    .and_then(|a| a.artist_name.as_deref())
                    .unwrap_or_default();
                Some(cache_keys::song(title, artist))
            }
            Some(Item::Album(album)) => {
                let name = album.album_name.as_deref().unwrap_or_default();
                let artist = album.album_artist.as_deref().unwrap_or_default();
                Some(cache_keys::album(name, artist))
            }
            Some(Item::Artist(artist)) => {
                let name = artist.artist_name.as_deref().unwrap_or_default();
                Some(cache_keys::artist(name))
            }
            None => None,
        };

        if let Some(ref key) = cache_key {
            if let Ok(Some(cached)) = get_cache::<RequestedCoverArtResponse>(key) {
                return Ok(cached);
            }
        }

        let mut urls = Vec::new();
        match req.item {
            Some(Item::Song(song)) => {
                self.fetch_song_deezer(&song, &mut urls);
                self.fetch_song_itunes(&song, &mut urls);
            }
            Some(Item::Album(album)) => {
                self.fetch_album_deezer(&album, &mut urls);
                self.fetch_album_itunes(&album, &mut urls);
                self.fetch_album_musicbrainz(&album, &mut urls);
            }
            Some(Item::Artist(artist)) => {
                self.fetch_artist_deezer(&artist, &mut urls);
            }
            None => {}
        }

        if let Some(ref key) = cache_key {
            if !urls.is_empty() {
                let _ = set_cache(
                    key,
                    &RequestedCoverArtResponse {
                        cover_urls: urls.clone(),
                    },
                );
            }
        }

        Ok(RequestedCoverArtResponse { cover_urls: urls })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    let extension = CoverArtExtension {
        client: Client::new(),
    };
    let _ = register_extension(Box::new(extension));
}

fn main() {}
