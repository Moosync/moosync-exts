use std::str::FromStr;

use rusty_ytdl::{
    reqwest::Url,
    search::{Channel, PlaylistSearchOptions, SearchOptions, SearchType, YouTube},
};

use moosync_edk::{
    Album, Artist, InnerSong, MoosyncError, MoosyncResult, Playlist, SearchResult, Song, SongType,
};

pub type Result<T> = MoosyncResult<T>;

use crate::utils::Pagination;

pub struct YoutubeScraper {
    youtube: YouTube,
}

impl Default for YoutubeScraper {
    fn default() -> YoutubeScraper {
        YoutubeScraper {
            youtube: YouTube::new().unwrap(),
        }
    }
}

impl YoutubeScraper {
    fn parse_song(&self, v: &rusty_ytdl::search::Video) -> Song {
        Song {
            song: Some(InnerSong {
                id: Some(format!("youtube:{}", v.id.clone())),
                deviceno: None,
                title: Some(v.title.clone()),
                duration: Some((v.duration / 1000) as f64),
                r#type: SongType::Url.into(),
                url: Some(v.id.clone()),
                song_cover_path_high: v.thumbnails.first().map(|d| d.url.clone()),
                song_cover_path_low: v.thumbnails.get(1).map(|d| d.url.clone()),
                playback_url: Some(format!("extension://moosync.youtubedl/{}", v.id)),
                provider_extension: Some("youtube".into()),
                ..Default::default()
            }),
            album: Some(Album {
                album_name: Some("Misc".to_string()),
                ..Default::default()
            }),
            artists: vec![Artist {
                artist_id: Some(format!("youtube-artist:{}", v.channel.id)),
                artist_name: Some(v.channel.name.clone()),
                ..Default::default()
            }],
            genre: vec![],
        }
    }

    fn parse_video_info(&self, v: &rusty_ytdl::VideoInfo) -> Song {
        let details = &v.video_details;
        Song {
            song: Some(InnerSong {
                id: Some(format!("youtube:{}", details.video_id.clone())),
                deviceno: None,
                title: Some(details.title.clone()),
                duration: Some(details.length_seconds.parse().unwrap_or_default()),
                r#type: SongType::Url.into(),
                url: Some(details.video_id.clone()),
                song_cover_path_high: details.thumbnails.first().map(|d| d.url.clone()),
                song_cover_path_low: details.thumbnails.get(1).map(|d| d.url.clone()),
                playback_url: Some(details.video_id.clone()),
                provider_extension: Some("youtube".into()),
                ..Default::default()
            }),
            album: Some(Album {
                album_name: Some("Misc".to_string()),
                ..Default::default()
            }),
            artists: vec![Artist {
                artist_id: Some(format!("youtube-artist:{}", details.channel_id)),
                artist_name: Some(details.owner_channel_name.clone()),
                ..Default::default()
            }],
            genre: vec![],
        }
    }

    fn parse_playlist(&self, playlist: &rusty_ytdl::search::Playlist) -> Playlist {
        Playlist {
            playlist_id: Some(format!("youtube-playlist:{}", playlist.id)),
            playlist_name: playlist.name.clone(),
            playlist_coverpath: playlist.thumbnails.first().map(|v| v.url.clone()),
            playlist_song_count: playlist.videos.capacity() as f64,
            extension: Some("youtube".into()),
            ..Default::default()
        }
    }

    fn parse_artist(&self, artist: &Channel) -> Artist {
        Artist {
            artist_id: Some(format!("youtube-artist:{}", artist.id)),
            artist_name: Some(artist.name.clone()),
            artist_coverpath: artist.icon.first().map(|v| v.url.clone()),
            ..Default::default()
        }
    }

    pub async fn get_playlist_content(&self, id: String, _: Pagination) -> Result<Vec<Song>> {
        let mut playlist = rusty_ytdl::search::Playlist::get(id, None)
            .await
            .map_err(|e| MoosyncError::String(e.to_string()))?;
        playlist.fetch(None).await;
        let res = playlist.videos.iter().map(|v| self.parse_song(v)).collect();

        Ok(res)
    }

    pub async fn get_video_by_id(&self, id: String) -> Result<Song> {
        let video = rusty_ytdl::Video::new(id).map_err(|e| MoosyncError::String(e.to_string()))?;
        let info = video
            .get_basic_info()
            .await
            .map_err(|e| MoosyncError::String(e.to_string()))?;
        Ok(self.parse_video_info(&info))
    }

    pub async fn get_playlist_from_url(&self, url: String) -> Result<Playlist> {
        let res = rusty_ytdl::search::Playlist::get(
            url,
            Some(&PlaylistSearchOptions {
                limit: 1,
                ..Default::default()
            }),
        )
        .await
        .map_err(|e| MoosyncError::String(e.to_string()))?;

        return Ok(self.parse_playlist(&res));
    }

    pub async fn search_yt(
        &self,
        query: impl Into<String>,
        search_type: SearchType,
    ) -> Result<SearchResult> {
        let res = self
            .youtube
            .search(
                query,
                Some(&SearchOptions {
                    limit: 100,
                    search_type,
                    safe_search: false,
                }),
            )
            .await
            .map_err(|e| MoosyncError::String(e.to_string()))?;

        let mut songs: Vec<Song> = vec![];
        let mut playlists: Vec<Playlist> = vec![];
        let mut artists: Vec<Artist> = vec![];
        for item in res {
            match item {
                rusty_ytdl::search::SearchResult::Video(v) => songs.push(self.parse_song(&v)),
                rusty_ytdl::search::SearchResult::Playlist(p) => {
                    playlists.push(self.parse_playlist(&p))
                }
                rusty_ytdl::search::SearchResult::Channel(a) => artists.push(self.parse_artist(&a)),
            }
        }
        Ok(SearchResult {
            songs,
            artists,
            playlists,
            albums: vec![],
            genres: vec![],
        })
    }

    pub async fn get_video_url(&self, mut id: String) -> Result<String> {
        if id.starts_with("http") {
            let url = Url::from_str(&id).unwrap();
            let query = url.query_pairs().find(|(k, _)| k == "v");
            if let Some((_, v)) = query {
                id = v.to_string();
            }
        }
        let video = rusty_ytdl::Video::new(id).map_err(|e| MoosyncError::String(e.to_string()))?;
        let info = video
            .get_info()
            .await
            .map_err(|e| MoosyncError::String(e.to_string()))?;

        moosync_edk::info!("Got formats {:?}", info.formats);

        let best_format = info
            .formats
            .into_iter()
            .filter(|format| format.has_audio && !format.has_video)
            .max_by(|a, b| a.bitrate.cmp(&b.bitrate));

        moosync_edk::info!("chose formats {:?}", best_format);

        match best_format {
            Some(f) => Ok(f.url.clone()),
            None => Err(MoosyncError::String("Unable to find URL".into())),
        }
    }

    pub async fn get_suggestions(&self) -> Result<Vec<Song>> {
        let songs = self
            .youtube
            .search(
                "music video",
                Some(&SearchOptions {
                    limit: 100,
                    search_type: SearchType::Video,
                    safe_search: false,
                }),
            )
            .await
            .map_err(|e| MoosyncError::String(e.to_string()))?;

        Ok(songs
            .into_iter()
            .filter_map(|s| {
                if let rusty_ytdl::search::SearchResult::Video(v) = s {
                    Some(self.parse_song(&v))
                } else {
                    None
                }
            })
            .collect())
    }
}
