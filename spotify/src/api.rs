use std::collections::{HashMap, HashSet};
use std::time::Instant;

use chrono::{DateTime, TimeDelta, Utc};
use futures::StreamExt;
use moosync_edk::{MoosyncError, MoosyncResult, info};
use oauth2::{
    AuthUrl, AuthorizationCode, Client, ClientId, ClientSecret, CsrfToken, EmptyExtraTokenFields,
    EndpointNotSet, EndpointSet, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl,
    RevocationErrorResponseType, Scope, StandardErrorResponse, StandardRevocableToken,
    StandardTokenIntrospectionResponse, StandardTokenResponse, TokenResponse, TokenUrl,
    basic::{BasicClient, BasicErrorResponseType, BasicTokenType},
};
use rspotify::{
    AuthCodePkceSpotify, Token,
    clients::OAuthClient,
    model::SubscriptionLevel,
    model::{
        AlbumId, ArtistId, PlaylistId, PlaylistTracksRef, SearchType, SimplifiedArtist,
        SimplifiedPlaylist, TrackId,
    },
    prelude::BaseClient,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use url::Url;

use moosync_edk::{
    Album, Artist, InnerSong, Playlist, PreferenceData, SearchResult, Song, SongType,
};
use regex::Regex;

use crate::utils::{self, Pagination};

pub type OAuth2Client = Client<
    StandardErrorResponse<BasicErrorResponseType>,
    StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, BasicTokenType>,
    StandardRevocableToken,
    StandardErrorResponse<RevocationErrorResponseType>,
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
>;

#[derive(Debug, Clone, Default)]
pub(crate) struct SpotifyConfig {
    pub(crate) client_secret: String,
    pub(crate) client_id: String,
    pub(crate) redirect_uri: &'static str,
    pub(crate) scopes: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenHolder {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub expires_at: i64,
    pub scope: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct ApiClient {
    api_client: AuthCodePkceSpotify,
    config: SpotifyConfig,
    verifier: Option<(OAuth2Client, PkceCodeVerifier, CsrfToken)>,
    tokens: Option<TokenHolder>,
    is_premium: bool,
}

impl ApiClient {
    pub fn new(config: SpotifyConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    pub async fn set_tokens(&mut self, tokens: Option<TokenHolder>) {
        self.tokens = tokens.clone();
        if let Some(token_holder) = tokens {
            let mut api_client_tokens = self.api_client.token.lock().await.unwrap();
            let expires_at_dt = DateTime::<Utc>::from_timestamp_millis(token_holder.expires_at)
                .ok_or("Invalid timestamp")
                .unwrap();
            *api_client_tokens = Some(Token {
                access_token: token_holder.access_token,
                expires_in: TimeDelta::seconds(token_holder.expires_in as i64),
                expires_at: Some(expires_at_dt),
                refresh_token: Some(token_holder.refresh_token),
                scopes: HashSet::from_iter(self.config.scopes.iter().map(|v| v.to_string())),
            });
        }
    }

    pub fn get_tokens(&self) -> Option<TokenHolder> {
        self.tokens.clone()
    }

    fn get_oauth_client(&self) -> OAuth2Client {
        let auth_url = AuthUrl::new("https://accounts.spotify.com/authorize".to_string())
            .expect("Invalid auth URL");
        let token_url = TokenUrl::new("https://accounts.spotify.com/api/token".to_string())
            .expect("Invalid token URL");

        let mut client = BasicClient::new(ClientId::new(self.config.client_id.clone()))
            .set_auth_uri(auth_url)
            .set_token_uri(token_url)
            .set_redirect_uri(
                RedirectUrl::new(self.config.redirect_uri.to_string())
                    .expect("Invalid redirect URI"),
            );

        if !self.config.client_secret.is_empty() {
            client = client.set_client_secret(ClientSecret::new(self.config.client_secret.clone()));
        };

        client
    }

    pub fn login(&mut self) -> MoosyncResult<String> {
        let client = self.get_oauth_client();
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let builder = client
            .authorize_url(CsrfToken::new_random)
            .add_scopes(self.config.scopes.iter().map(|s| Scope::new(s.to_string())))
            .set_pkce_challenge(pkce_challenge);

        let (auth_url, csrf_token) = builder.url();

        self.verifier = Some((client, pkce_verifier, csrf_token));

        Ok(auth_url.to_string())
    }

    pub fn logout(&mut self) {
        self.tokens = None;
        self.verifier = None;
        self.api_client = AuthCodePkceSpotify::default();
    }

    pub async fn authorize(&mut self, code: String) -> MoosyncResult<()> {
        let (client, verifier, _) = self.verifier.take().ok_or("OAuth flow not initiated")?;

        let code_val = if code.contains("?") || code.contains("code=") || code.starts_with("http") {
            let url_str = if !code.starts_with("http") {
                format!("https://moosync.app/{}", code.trim_start_matches('/'))
            } else {
                code
            };
            let url = Url::parse(&url_str).map_err(|e| e.to_string())?;
            url.query_pairs()
                .find(|(k, _)| k == "code")
                .map(|(_, v)| v.to_string())
                .ok_or("Code not found in URL")?
        } else {
            code
        };

        info!("Got code {}", code_val);

        let http_client = reqwest::ClientBuilder::new()
            .build()
            .map_err(|e| e.to_string())?;

        let res = client
            .exchange_code(AuthorizationCode::new(code_val))
            .set_pkce_verifier(verifier)
            .request_async(&http_client)
            .await
            .map_err(|e| format!("Token exchange failed: {}", e))?;

        let refresh_token = res
            .refresh_token()
            .map(|r| r.secret().clone())
            .unwrap_or_default();

        let expires_in_sec = res.expires_in().map(|d| d.as_secs()).unwrap_or(3600);

        let now_sec = moosync_edk::api::extension_api::get_system_time();

        let expires_at_ms = (now_sec + expires_in_sec) * 1000;

        let token_holder = TokenHolder {
            access_token: res.access_token().secret().clone(),
            refresh_token: refresh_token.clone(),
            expires_in: expires_in_sec,
            expires_at: expires_at_ms as i64,
            scope: None,
        };

        self.tokens = Some(token_holder.clone());
        info!("Got tokens {:?}", self.tokens);

        let expires_at_dt = DateTime::<Utc>::from_timestamp_millis(token_holder.expires_at)
            .ok_or("Invalid timestamp")?;

        self.api_client = AuthCodePkceSpotify::from_token(Token {
            access_token: token_holder.access_token,
            expires_in: TimeDelta::seconds(token_holder.expires_in as i64),
            expires_at: Some(expires_at_dt),
            refresh_token: Some(token_holder.refresh_token),
            scopes: HashSet::from_iter(self.config.scopes.iter().map(|v| v.to_string())),
        });
        self.api_client.config.token_refreshing = true;

        Ok(())
    }

    pub async fn get_username(&mut self) -> MoosyncResult<Option<String>> {
        info!("Fetching user details for spotify");
        let user = self
            .api_client
            .current_user()
            .await
            .map_err(|e| MoosyncError::String(e.to_string()))?;
        info!("Got user {:?}", user);
        if let Some(subscription) = user.product {
            if subscription == SubscriptionLevel::Premium {
                self.is_premium = true
            }
        }

        Ok(user.display_name)
    }

    pub fn key(&self) -> String {
        "spotify".into()
    }

    pub fn match_id(&self, id: String) -> bool {
        id.starts_with("moosync.spotify:")
    }

    pub async fn match_url(&self, url: String) -> MoosyncResult<bool> {
        let re = Regex::new(
            r"^(https:\/\/open\.spotify\.com\/(track|embed)\/|spotify:track:)([a-zA-Z0-9]+)(.*)$",
        )
        .unwrap();
        if re.is_match(url.as_str()) {
            return Ok(true);
        }

        let re = Regex::new(
            r"^(https:\/\/open\.spotify\.com\/playlist\/|spotify:playlist:)([a-zA-Z0-9]+)(.*)$",
        )
        .unwrap();
        if re.is_match(url.as_str()) {
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn fetch_user_playlists(
        &self,
        pagination: Pagination,
    ) -> MoosyncResult<(Vec<Playlist>, Pagination)> {
        let mut ret = vec![];
        let playlists = self
            .api_client
            .current_user_playlists_manual(Some(pagination.limit), Some(pagination.offset))
            .await
            .map_err(|e| format!("Failed to fetch playlists: {}", e))?;

        for playlist in playlists.items {
            ret.push(utils::parse_playlist(playlist));
        }

        Ok((ret, pagination.next_page()))
    }

    pub async fn get_playlist_content(
        &self,
        playlist: Playlist,
        pagination: Pagination,
    ) -> MoosyncResult<(Vec<Song>, Pagination)> {
        let mut ret = vec![];
        let playlist_id_str = playlist.playlist_id.ok_or("Playlist ID cannot be none")?;

        let playlist_id = playlist_id_str
            .strip_prefix("moosync.spotify:")
            .unwrap_or(&playlist_id_str);

        let items = self
            .api_client
            .playlist_items_manual(
                PlaylistId::from_id_or_uri(playlist_id).map_err(|e| e.to_string())?,
                None,
                None,
                Some(pagination.limit),
                Some(pagination.offset),
            )
            .await
            .map_err(|e| format!("Failed to fetch playlist items: {}", e))?;

        for i in items.items {
            if i.is_local {
                continue;
            }

            match i.track {
                Some(rspotify::model::PlayableItem::Track(t)) => {
                    ret.push(utils::parse_track(t));
                }
                _ => continue,
            }
        }
        Ok((ret, pagination.next_page()))
    }

    pub async fn search(&self, term: String) -> MoosyncResult<SearchResult> {
        let mut ret = SearchResult {
            songs: vec![],
            albums: vec![],
            artists: vec![],
            playlists: vec![],
            ..Default::default()
        };

        // Helper macro to reduce repetition, similar to the original implementation
        macro_rules! search_and_parse {
            ($type:expr, $variant:path, $parse_fn:expr, $result_vec:expr) => {
                if let Ok($variant(items)) = self
                    .api_client
                    .search(&term, $type, None, None, Some(50), Some(0))
                    .await
                {
                    for item in items.items {
                        $parse_fn(item, &mut $result_vec);
                    }
                }
            };
        }

        search_and_parse!(
            SearchType::Track,
            rspotify::model::SearchResult::Tracks,
            |item, vec: &mut Vec<Song>| vec.push(utils::parse_track(item)),
            ret.songs
        );

        search_and_parse!(
            SearchType::Playlist,
            rspotify::model::SearchResult::Playlists,
            |item, vec: &mut Vec<Playlist>| vec.push(utils::parse_playlist(item)),
            ret.playlists
        );

        search_and_parse!(
            SearchType::Artist,
            rspotify::model::SearchResult::Artists,
            |item: rspotify::model::FullArtist, vec: &mut Vec<Artist>| vec.push(
                utils::parse_artist(
                    SimplifiedArtist {
                        external_urls: item.external_urls,
                        href: Some(item.href),
                        id: Some(item.id),
                        name: item.name,
                    },
                    Some(item.images)
                )
            ),
            ret.artists
        );

        search_and_parse!(
            SearchType::Album,
            rspotify::model::SearchResult::Albums,
            |item, vec: &mut Vec<Album>| vec.push(utils::parse_album(item)),
            ret.albums
        );

        Ok(ret)
    }
    pub async fn playlist_from_url(&self, url: String) -> MoosyncResult<Playlist> {
        let playlist_id_str = Url::parse(&url)
            .map(|u| u.path().to_string())
            .unwrap_or_else(|_| url.clone());

        let playlist = self
            .api_client
            .playlist(
                PlaylistId::from_id_or_uri(&playlist_id_str).map_err(|e| e.to_string())?,
                None,
                None,
            )
            .await
            .map_err(|e| format!("Failed to fetch playlist from URL: {}", e))?;

        Ok(utils::parse_playlist(SimplifiedPlaylist {
            collaborative: playlist.collaborative,
            external_urls: playlist.external_urls,
            href: playlist.href,
            id: playlist.id,
            images: playlist.images,
            name: playlist.name,
            owner: playlist.owner,
            public: playlist.public,
            snapshot_id: playlist.snapshot_id,
            tracks: PlaylistTracksRef::default(),
        }))
    }

    pub async fn song_from_url(&self, url: String) -> MoosyncResult<Song> {
        let track_id_str = Url::parse(&url)
            .map(|u| {
                let path = u.path().to_string();
                if path.starts_with("track:") {
                    url.clone()
                } else {
                    path
                }
            })
            .unwrap_or_else(|_| url.clone());

        let track = self
            .api_client
            .track(
                TrackId::from_id_or_uri(&track_id_str).map_err(|e| e.to_string())?,
                None,
            )
            .await
            .map_err(|e| format!("Failed to fetch track from URL: {}", e))?;

        Ok(utils::parse_track(track))
    }

    pub async fn song_from_id(&self, id: String) -> MoosyncResult<Song> {
        let stripped_id = id.replacen("spotify:", "", 1);
        self.song_from_url(stripped_id.trim().to_string()).await
    }

    pub async fn get_suggestions(&self) -> MoosyncResult<Vec<Song>> {
        let mut seed_tracks = vec![];
        let mut i = 0;

        let mut top_tracks = self.api_client.current_user_top_tracks(None);

        while i < 5 {
            if let Some(Ok(track)) = top_tracks.next().await {
                if let Some(track_id) = track.id {
                    seed_tracks.push(track_id);
                    i += 1;
                }
            } else {
                break;
            }
        }

        let recom = self
            .api_client
            .recommendations(
                vec![],
                Some(vec![]),
                Some(vec![]),
                Some(seed_tracks),
                None,
                Some(100),
            )
            .await
            .map_err(|e| format!("Failed to fetch recommendations: {}", e))?;

        Ok(recom
            .tracks
            .into_iter()
            .map(|t| {
                utils::parse_track(rspotify::model::FullTrack {
                    album: t.album.unwrap_or_default(),
                    artists: t.artists,
                    disc_number: t.disc_number,
                    duration: t.duration,
                    id: t.id,
                    name: t.name,
                    track_number: t.track_number,
                    available_markets: vec![],
                    explicit: t.explicit,
                    external_urls: t.external_urls,
                    external_ids: HashMap::new(),
                    href: None,
                    is_local: false,
                    is_playable: None,
                    linked_from: None,
                    restrictions: None,
                    popularity: 0,
                    preview_url: t.preview_url,
                    r#type: rspotify::model::Type::Track,
                })
            })
            .collect())
    }

    pub async fn get_album_content(
        &self,
        album: Album,
        pagination: Pagination,
    ) -> MoosyncResult<(Vec<Song>, Pagination)> {
        let mut raw_id = album.album_id;

        // If ID is missing or doesn't look like a spotify ID, search for it
        if let Some(id) = &raw_id {
            if !self.match_id(id.clone()) {
                if let Some(album_name) = album.album_name {
                    let res = self.search(album_name).await?;
                    if let Some(found_album) = res.albums.first() {
                        raw_id = found_album.album_id.clone();
                    } else {
                        raw_id = None;
                    }
                } else {
                    raw_id = None;
                }
            }
        }

        if let Some(id) = raw_id {
            let id = id.replace("moosync.spotify:", "");
            let album_id = AlbumId::from_id_or_uri(&id).map_err(|e| e.to_string())?;

            let full_album = self
                .api_client
                .album(album_id.clone(), None)
                .await
                .map_err(|e| format!("Failed to fetch album: {}", e))?;

            let album_tracks = self
                .api_client
                .album_track_manual(
                    album_id,
                    None,
                    Some(pagination.limit),
                    Some(pagination.offset),
                )
                .await
                .map_err(|e| format!("Failed to fetch album tracks: {}", e))?;

            let songs = album_tracks
                .items
                .into_iter()
                .map(|t| {
                    let mut full_track = utils::get_full_track_from_simplified(t);
                    full_track.album = utils::simplify_full_album(full_album.clone());
                    utils::parse_track(full_track)
                })
                .collect();

            Ok((songs, pagination.next_page()))
        } else {
            Err("Could not find album ID".into())
        }
    }

    pub async fn get_artist_content(
        &self,
        artist: Artist,
        pagination: Pagination,
    ) -> MoosyncResult<(Vec<Song>, Pagination)> {
        if let Some(next_page_token) = &pagination.token {
            // Handle next page logic if token based
            // For now return empty as per original implementation placeholder
            let _tokens: Vec<&str> = next_page_token.split(';').collect();
            return Ok((vec![], pagination.next_page_wtoken(None)));
        }

        let mut raw_id = artist.artist_id;
        if let Some(id) = &raw_id {
            if !self.match_id(id.clone()) {
                if let Some(artist_name) = artist.artist_name {
                    let res = self.search(artist_name).await?;
                    if let Some(found_artist) = res.artists.first() {
                        raw_id = found_artist.artist_id.clone();
                    } else {
                        raw_id = None;
                    }
                } else {
                    raw_id = None;
                }
            }
        }

        if let Some(id) = raw_id {
            let id = id.replace("moosync.spotify:", "");
            let artist_id = ArtistId::from_id_or_uri(&id).map_err(|e| e.to_string())?;

            let mut songs = vec![];
            let mut next_page_tokens = vec![];

            let mut albums_stream = self.api_client.artist_albums(artist_id, [], None);
            let mut album_ids = vec![];

            while let Some(Ok(album)) = albums_stream.next().await {
                if let Some(id) = album.id {
                    album_ids.push(id);
                }
            }

            // Process in chunks to avoid huge requests if supported, otherwise loop
            for chunk in album_ids.chunks(20) {
                if let Ok(albums) = self.api_client.albums(chunk.to_vec(), None).await {
                    for a in albums {
                        let simplified_album = utils::simplify_full_album(a.clone());
                        let tracks = a.tracks.items;
                        let parsed = tracks.into_iter().map(|t| {
                            let mut full_track = utils::get_full_track_from_simplified(t);
                            full_track.album = simplified_album.clone();
                            utils::parse_track(full_track)
                        });
                        songs.extend(parsed);

                        if let Some(next) = a.tracks.next {
                            next_page_tokens.push(next);
                        }
                    }
                }
            }

            let next_page_token = next_page_tokens.join(";");
            Ok((songs, pagination.next_page_wtoken(Some(next_page_token))))
        } else {
            Err("Could not find artist ID".into())
        }
    }
}
