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
        get_cache, parse_lrc, set_cache, Accounts, ContextMenu, DatabaseEvents, Extension,
        PlayerEvents, PlayerProvider, PreferenceEvents, Provider,
    },
    handler::register_extension,
    info,
    types::{Lyrics, RequestedLyricsRequest, RequestedLyricsResponse, Song},
    MoosyncResult,
};
use reqwest::Client;
use scraper::{Html, Selector};
use serde::Deserialize;
use tracing;
use url::Url;

mod cache_keys;

struct LyricsExtension {
    client: Client,
}

impl Extension for LyricsExtension {}
impl PlayerProvider for LyricsExtension {}
impl PlayerEvents for LyricsExtension {}
impl PreferenceEvents for LyricsExtension {}
impl DatabaseEvents for LyricsExtension {}
impl Accounts for LyricsExtension {}
impl ContextMenu for LyricsExtension {}

#[derive(Deserialize, Debug)]
struct LrclibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

#[derive(Deserialize, Debug)]
struct GeniusSearchHit {
    result: GeniusSearchResult,
}

#[derive(Deserialize, Debug)]
struct GeniusSearchResult {
    url: String,
}

#[derive(Deserialize, Debug)]
struct GeniusSearchResponse {
    response: GeniusSearchResponseBody,
}

#[derive(Deserialize, Debug)]
struct GeniusSearchResponseBody {
    hits: Vec<GeniusSearchHit>,
}

#[tracing::instrument(level = "debug", skip_all)]
fn extract_genius_lyrics(html: &str) -> String {
    let cleaned = html.replace("<br/>", "\n").replace("<br>", "\n");
    let document = Html::parse_document(&cleaned);
    let Ok(selector) = Selector::parse("div[data-lyrics-container='true']") else {
        return String::new();
    };

    let mut output = String::new();
    for container in document.select(&selector) {
        for text in container.text() {
            output.push_str(text);
        }
        output.push('\n');
    }

    output
}

impl LyricsExtension {
    #[tracing::instrument(level = "debug", skip_all)]
    fn fetch_lrclib(&self, song: &Song) -> MoosyncResult<Option<Lyrics>> {
        let Some(inner) = &song.song else {
            return Ok(None);
        };
        let Some(title) = &inner.title else {
            return Ok(None);
        };
        if title.is_empty() {
            return Ok(None);
        }

        let artist_name = song
            .artists
            .first()
            .and_then(|artist| artist.artist_name.as_deref())
            .unwrap_or_default();
        let album_name = song
            .album
            .as_ref()
            .and_then(|album| album.album_name.as_deref())
            .unwrap_or_default();

        let Ok(mut url) = Url::parse("https://lrclib.net/api/get") else {
            return Ok(None);
        };
        url.query_pairs_mut()
            .append_pair("track_name", title)
            .append_pair("artist_name", artist_name)
            .append_pair("album_name", album_name);

        if let Some(duration) = &inner.duration {
            if duration.seconds > 0 {
                url.query_pairs_mut()
                    .append_pair("duration", &duration.seconds.to_string());
            }
        }

        info!("Querying LRCLIB: {url}");
        let resp = match block_on(
            self.client
                .get(url.as_str())
                .header("User-Agent", "Moosync/1.0")
                .send(),
        ) {
            Ok(r) if r.status().is_success() => r,
            _ => return Ok(None),
        };

        let Ok(data) = block_on(resp.json::<LrclibResponse>()) else {
            return Ok(None);
        };

        if let Some(synced) = data.synced_lyrics {
            if let Ok(parsed) = parse_lrc(&synced) {
                if !parsed.lines.is_empty() {
                    return Ok(Some(parsed));
                }
            }
        }

        if let Some(plain) = data.plain_lyrics {
            if let Ok(parsed) = parse_lrc(&plain) {
                if !parsed.lines.is_empty() {
                    return Ok(Some(parsed));
                }
            }
        }

        Ok(None)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    fn fetch_genius(&self, song: &Song) -> MoosyncResult<Option<Lyrics>> {
        let Some(inner) = &song.song else {
            return Ok(None);
        };
        let Some(title) = &inner.title else {
            return Ok(None);
        };
        if title.is_empty() {
            return Ok(None);
        }

        let artist_name = song
            .artists
            .first()
            .and_then(|artist| artist.artist_name.as_deref())
            .unwrap_or_default();

        let Ok(mut search_url) = Url::parse("https://api.genius.com/search") else {
            return Ok(None);
        };
        search_url
            .query_pairs_mut()
            .append_pair("q", &format!("{artist_name} {title}"));

        info!("Querying Genius API: {search_url}");
        let search_resp = match block_on(
            self.client
                .get(search_url.as_str())
                .header("User-Agent", "Moosync/1.0")
                .send(),
        ) {
            Ok(r) if r.status().is_success() => r,
            _ => return Ok(None),
        };

        let Ok(search_response) = block_on(search_resp.json::<GeniusSearchResponse>()) else {
            return Ok(None);
        };

        let Some(first_hit) = search_response.response.hits.first() else {
            return Ok(None);
        };

        let song_page_url = &first_hit.result.url;
        if song_page_url.is_empty() {
            return Ok(None);
        }

        let page_resp = match block_on(
            self.client
                .get(song_page_url)
                .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64)")
                .send(),
        ) {
            Ok(r) if r.status().is_success() => r,
            _ => return Ok(None),
        };

        let Ok(html_content) = block_on(page_resp.text()) else {
            return Ok(None);
        };

        let extracted_lyrics = extract_genius_lyrics(&html_content);
        if !extracted_lyrics.is_empty() {
            if let Ok(parsed) = parse_lrc(&extracted_lyrics) {
                if !parsed.lines.is_empty() {
                    return Ok(Some(parsed));
                }
            }
        }

        Ok(None)
    }
}

impl Provider for LyricsExtension {
    #[tracing::instrument(level = "debug", skip_all)]
    fn requested_lyrics(
        &self,
        req: RequestedLyricsRequest,
    ) -> MoosyncResult<RequestedLyricsResponse> {
        let Some(song) = req.song else {
            return Ok(RequestedLyricsResponse { lyrics: None });
        };

        let cache_key = if let Some(id) = song
            .song
            .as_ref()
            .and_then(|s| s.id.as_deref())
            .filter(|id| !id.is_empty())
        {
            cache_keys::by_id(id)
        } else {
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
            cache_keys::by_track(title, artist)
        };

        if let Ok(Some(cached)) = get_cache::<Lyrics>(&cache_key) {
            return Ok(RequestedLyricsResponse { lyrics: Some(cached) });
        }

        let lyrics = if let Ok(Some(lyrics)) = self.fetch_lrclib(&song) {
            Some(lyrics)
        } else if let Ok(Some(lyrics)) = self.fetch_genius(&song) {
            Some(lyrics)
        } else {
            None
        };

        if let Some(ref l) = lyrics {
            let _ = set_cache(&cache_key, l);
        }

        Ok(RequestedLyricsResponse { lyrics })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    let extension = LyricsExtension {
        client: Client::new(),
    };
    let _ = register_extension(Box::new(extension));
}

fn main() {}
