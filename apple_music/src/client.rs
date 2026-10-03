use std::sync::Mutex;
use std::time::Duration;

use futures::executor::block_on;
use moosync_edk::{Album, Artist, MoosyncResult, Playlist, SearchResult, Song, error, info, warn};
use ramdl::stream_info::StreamInfo;
use ramdl::AppleMusicDownloader;

use crate::utils::{
    album_from_ramdl, album_from_search, artist_from_search, playlist_from_ramdl,
    playlist_from_search, song_from_ramdl, song_from_search,
};

pub struct AppleMusicClient {
    downloader: Mutex<AppleMusicDownloader>,
}

impl AppleMusicClient {
    pub fn new(media_user_token: Option<String>) -> Self {
        let token = media_user_token.unwrap_or_default();
        let downloader = match block_on(AppleMusicDownloader::new_with_media_user_token(&token)) {
            Ok(dl) => dl,
            Err(e) => {
                error!("Failed to initialize Apple Music downloader: {:?}", e);
                AppleMusicDownloader::default()
            }
        };
        Self {
            downloader: Mutex::new(downloader),
        }
    }

    pub fn set_user_token(&self, token: &str) -> MoosyncResult<()> {
        let mut downloader = self.downloader.lock().unwrap();
        *downloader = block_on(AppleMusicDownloader::new_with_media_user_token(token))
            .map_err(|e| format!("Failed to re-initialize with token: {:?}", e))?;
        Ok(())
    }

    pub fn search(&self, query: String) -> MoosyncResult<SearchResult> {
        let downloader = self.downloader.lock().unwrap();
        let res = block_on(downloader.search(&query))
            .map_err(|e| format!("Search failed: {:?}", e))?;

        let songs: Vec<Song> = res.songs.iter().map(song_from_search).collect();
        let albums: Vec<Album> = res.albums.iter().map(album_from_search).collect();
        let artists: Vec<Artist> = res.artists.iter().map(artist_from_search).collect();
        let playlists: Vec<Playlist> = res.playlists.iter().map(playlist_from_search).collect();

        Ok(SearchResult {
            songs,
            albums,
            artists,
            playlists,
            genres: vec![],
        })
    }

    pub fn get_song(&self, id: &str) -> MoosyncResult<Option<Song>> {
        let downloader = self.downloader.lock().unwrap();
        match block_on(downloader.get_songs(id)) {
            Ok(song) => Ok(Some(song_from_ramdl(&song))),
            Err(e) => {
                warn!("Failed to fetch song {}: {:?}", id, e);
                Ok(None)
            }
        }
    }

    pub fn get_album(&self, id: &str) -> MoosyncResult<Option<Album>> {
        let downloader = self.downloader.lock().unwrap();
        match block_on(downloader.get_album(id)) {
            Ok(album) => Ok(Some(album_from_ramdl(&album))),
            Err(e) => {
                warn!("Failed to fetch album {}: {:?}", id, e);
                Ok(None)
            }
        }
    }

    pub fn get_album_songs(&self, id: &str) -> MoosyncResult<Vec<Song>> {
        let downloader = self.downloader.lock().unwrap();
        let tracks = block_on(downloader.get_album_tracks(id))
            .map_err(|e| format!("Failed to fetch album tracks: {:?}", e))?;
        Ok(tracks.iter().map(song_from_ramdl).collect())
    }

    pub fn get_playlist(&self, id: &str) -> MoosyncResult<Option<Playlist>> {
        let downloader = self.downloader.lock().unwrap();
        match block_on(downloader.get_playlist(id)) {
            Ok(playlist) => Ok(Some(playlist_from_ramdl(&playlist))),
            Err(e) => {
                warn!("Failed to fetch playlist {}: {:?}", id, e);
                Ok(None)
            }
        }
    }

    pub fn get_playlist_songs(&self, id: &str) -> MoosyncResult<Vec<Song>> {
        let downloader = self.downloader.lock().unwrap();
        let tracks = block_on(downloader.get_playlist_tracks(id))
            .map_err(|e| format!("Failed to fetch playlist tracks: {:?}", e))?;
        Ok(tracks.iter().map(song_from_ramdl).collect())
    }

    pub fn get_artist_songs(&self, id: &str) -> MoosyncResult<Vec<Song>> {
        let downloader = self.downloader.lock().unwrap();
        let songs = block_on(downloader.get_artist_top_songs(id))
            .map_err(|e| format!("Failed to fetch artist top songs: {:?}", e))?;
        Ok(songs.iter().map(song_from_ramdl).collect())
    }

    pub fn get_recommendations(&self) -> MoosyncResult<Vec<Song>> {
        let downloader = self.downloader.lock().unwrap();
        let songs = block_on(downloader.get_charts())
            .map_err(|e| format!("Failed to fetch recommendations: {:?}", e))?;
        Ok(songs.iter().map(song_from_ramdl).collect())
    }

    pub fn get_playback_details(&self, song_id: &str) -> MoosyncResult<(String, Option<String>, Option<Duration>)> {
        let downloader = self.downloader.lock().unwrap();
        info!("Resolving playback details for song: {}", song_id);

        // 1. Try actual WebPlayback endpoint and DRM decryption keys:
        match block_on(downloader.get_webplayback(song_id)) {
            Ok(webplayback) => {
                let stream_info = block_on(StreamInfo::new_with_webplayback(&webplayback)).ok();
                let drm_key = if let Some(ref si) = stream_info {
                    match block_on(downloader.get_decryption_key(si, song_id)) {
                        Ok(k) => {
                            info!("Successfully acquired Widevine DRM decryption key: {}", k);
                            Some(k)
                        }
                        Err(e) => {
                            warn!("Failed to acquire DRM key: {:?}", e);
                            None
                        }
                    }
                } else {
                    None
                };

                let stream_url = stream_info
                    .map(|si| si.stream_url)
                    .or_else(|| {
                        webplayback
                            .song_list
                            .first()
                            .and_then(|s| s.assets.iter().find(|t| t.flavor == "28:ctrp256"))
                            .map(|a| a.url.clone())
                    });

                let duration = webplayback
                    .song_list
                    .first()
                    .and_then(|s| s.assets.iter().find(|t| t.flavor == "28:ctrp256"))
                    .map(|a| Duration::from_millis(a.metadata.duration));

                if let Some(url) = stream_url {
                    return Ok((url, drm_key, duration));
                }
            }
            Err(e) => {
                warn!("Webplayback resolution failed for {}: {:?}. Trying catalog preview.", song_id, e);
            }
        }

        // 2. Fallback to catalog preview stream URL:
        let song = block_on(downloader.get_songs(song_id))
            .map_err(|e| format!("Failed to get song {}: {:?}", song_id, e))?;
        let preview_url = song
            .attributes
            .previews
            .first()
            .map(|p| p.url.clone())
            .ok_or_else(|| format!("No playback or preview URL available for song {}", song_id))?;
        let duration = Some(Duration::from_millis(song.attributes.duration_in_millis));

        Ok((preview_url, None, duration))
    }
}
