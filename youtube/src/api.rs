use moosync_edk::{
    Album, Artist, ExtensionAccountDetail, InnerSong, Playlist, PreferenceData, info,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use url::Url;

/// Struct to manage YouTube OAuth authentication and API calls.
pub struct YoutubeAuth {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    access_token: Option<String>,
    refresh_token: Option<String>,
    client: Client,
    state: Option<String>,
    pub account_details: ExtensionAccountDetail,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub expires_in: Option<u64>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub token_type: Option<String>,
}

impl From<OAuthTokenResponse>
    for moosync_edk::extensions_proto::struct_proto::google::protobuf::Value
{
    fn from(token: OAuthTokenResponse) -> Self {
        let mut fields = std::collections::HashMap::new();
        fields.insert(
            "access_token".to_string(),
            moosync_edk::extensions_proto::struct_proto::google::protobuf::Value {
                kind: Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::StringValue(
                    token.access_token,
                )),
            },
        );
        if let Some(expires_in) = token.expires_in {
            fields.insert(
                "expires_in".to_string(),
                moosync_edk::extensions_proto::struct_proto::google::protobuf::Value {
                    kind: Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::NumberValue(
                        expires_in as f64,
                    )),
                },
            );
        }
        if let Some(refresh_token) = token.refresh_token {
            fields.insert(
                "refresh_token".to_string(),
                moosync_edk::extensions_proto::struct_proto::google::protobuf::Value {
                    kind: Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::StringValue(
                        refresh_token,
                    )),
                },
            );
        }
        if let Some(scope) = token.scope {
            fields.insert(
                "scope".to_string(),
                moosync_edk::extensions_proto::struct_proto::google::protobuf::Value {
                    kind: Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::StringValue(
                        scope,
                    )),
                },
            );
        }
        if let Some(token_type) = token.token_type {
            fields.insert(
                "token_type".to_string(),
                moosync_edk::extensions_proto::struct_proto::google::protobuf::Value {
                    kind: Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::StringValue(
                        token_type,
                    )),
                },
            );
        }
        moosync_edk::extensions_proto::struct_proto::google::protobuf::Value {
            kind: Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::StructValue(
                moosync_edk::extensions_proto::struct_proto::google::protobuf::Struct { fields },
            )),
        }
    }
}

impl TryFrom<moosync_edk::extensions_proto::struct_proto::google::protobuf::Value>
    for OAuthTokenResponse
{
    type Error = &'static str;

    fn try_from(
        val: moosync_edk::extensions_proto::struct_proto::google::protobuf::Value,
    ) -> Result<Self, Self::Error> {
        if let Some(
            moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::StructValue(
                s,
            ),
        ) = val.kind
        {
            let access_token = s
                .fields
                .get("access_token")
                .and_then(|v| match &v.kind {
                    Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::StringValue(s)) => {
                        Some(s.clone())
                    }
                    _ => None,
                })
                .ok_or("Missing access_token")?;

            let expires_in = s.fields.get("expires_in").and_then(|v| match &v.kind {
                Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::NumberValue(n)) => Some(*n as u64),
                _ => None,
            });

            let refresh_token = s.fields.get("refresh_token").and_then(|v| match &v.kind {
                Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::StringValue(s)) => Some(s.clone()),
                _ => None,
            });

            let scope = s.fields.get("scope").and_then(|v| match &v.kind {
                Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::StringValue(s)) => Some(s.clone()),
                _ => None,
            });

            let token_type = s.fields.get("token_type").and_then(|v| match &v.kind {
                Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::StringValue(s)) => Some(s.clone()),
                _ => None,
            });

            Ok(OAuthTokenResponse {
                access_token,
                expires_in,
                refresh_token,
                scope,
                token_type,
            })
        } else {
            Err("Expected StructValue")
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct PlaylistListResponse {
    pub items: Option<Vec<PlaylistItem>>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PlaylistItem {
    pub id: Option<String>,
    pub snippet: Option<PlaylistSnippet>,
    // Add more fields as needed
}

impl From<PlaylistItem> for Playlist {
    fn from(item: PlaylistItem) -> Self {
        let coverpath = item
            .snippet
            .as_ref()
            .and_then(|s| s.thumbnails.as_ref())
            .and_then(|thumbs| {
                thumbs
                    .default
                    .as_ref()
                    .map(|t| t.url.clone())
                    .or_else(|| thumbs.high.as_ref().map(|t| t.url.clone()))
                    .or_else(|| thumbs.medium.as_ref().map(|t| t.url.clone()))
            });
        Playlist {
            playlist_id: item
                .id
                .as_ref()
                .map(|id| format!("youtube-playlist:{}", id)),
            playlist_name: item
                .snippet
                .as_ref()
                .and_then(|s| s.title.clone())
                .unwrap_or_default(),
            playlist_coverpath: coverpath,
            playlist_song_count: 0.0, // Not available from PlaylistItem directly
            extension: Some("youtube".into()),
            ..Default::default()
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct PlaylistSnippet {
    pub title: Option<String>,
    pub thumbnails: Option<PlaylistThumbnails>,
}

#[derive(Deserialize, Debug)]
pub struct PlaylistThumbnails {
    pub default: Option<ThumbnailInfo>,
    pub medium: Option<ThumbnailInfo>,
    pub high: Option<ThumbnailInfo>,
}

#[derive(Deserialize, Debug)]
pub struct ThumbnailInfo {
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct PlaylistItemsListResponse {
    pub items: Option<Vec<PlaylistVideoItem>>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PlaylistVideoItem {
    pub snippet: Option<PlaylistVideoSnippet>,
    #[serde(rename = "contentDetails")]
    pub content_details: Option<PlaylistVideoContentDetails>,
}

#[derive(Deserialize, Debug)]
pub struct PlaylistVideoSnippet {
    pub title: Option<String>,
    #[serde(rename = "resourceId")]
    pub resource_id: Option<PlaylistVideoResourceId>,
    pub thumbnails: Option<PlaylistThumbnails>,
    #[serde(rename = "channelTitle")]
    pub channel_title: Option<String>,
    #[serde(rename = "videoOwnerChannelTitle")]
    pub video_owner_channel_title: Option<String>,
    #[serde(rename = "videoOwnerChannelId")]
    pub video_owner_channel_id: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PlaylistVideoResourceId {
    #[serde(rename = "videoId")]
    pub video_id: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PlaylistVideoContentDetails {
    #[serde(rename = "videoId")]
    pub video_id: Option<String>,
}

impl From<PlaylistVideoItem> for moosync_edk::Song {
    fn from(item: PlaylistVideoItem) -> Self {
        let (video_id, title, thumbnails, channel_title) = if let Some(snippet) = &item.snippet {
            (
                snippet
                    .resource_id
                    .as_ref()
                    .and_then(|r| r.video_id.clone())
                    .or_else(|| {
                        item.content_details
                            .as_ref()
                            .and_then(|c| c.video_id.clone())
                    }),
                snippet.title.clone(),
                snippet.thumbnails.as_ref(),
                snippet
                    .channel_title
                    .clone()
                    .or(snippet.video_owner_channel_title.clone()),
            )
        } else {
            (
                item.content_details
                    .as_ref()
                    .and_then(|c| c.video_id.clone()),
                None,
                None,
                None,
            )
        };

        moosync_edk::Song {
            song: Some(InnerSong {
                id: video_id.as_ref().map(|id| format!("youtube:{}", id)),
                deviceno: None,
                title,
                duration: None, // Not available from playlistItems API
                r#type: moosync_edk::SongType::Url.into(),
                url: video_id.clone(),
                song_cover_path_high: thumbnails
                    .and_then(|thumbs| thumbs.high.as_ref().map(|t| t.url.clone()))
                    .or_else(|| {
                        thumbnails.and_then(|thumbs| thumbs.default.as_ref().map(|t| t.url.clone()))
                    }),
                song_cover_path_low: thumbnails
                    .and_then(|thumbs| thumbs.medium.as_ref().map(|t| t.url.clone())),
                playback_url: video_id
                    .as_ref()
                    .map(|id| format!("extension://moosync.youtubedl/{}", id)),
                provider_extension: Some("youtube".into()),
                ..Default::default()
            }),
            album: Some(Album {
                album_name: Some("Misc".to_string()),
                ..Default::default()
            }),
            artists: vec![Artist {
                artist_id: None,
                artist_name: channel_title,
                ..Default::default()
            }],
            genre: vec![],
        }
    }
}

impl YoutubeAuth {
    pub fn new(client_id: &str, client_secret: &str, redirect_uri: &str) -> Self {
        YoutubeAuth {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            redirect_uri: redirect_uri.to_string(),
            access_token: None,
            refresh_token: None,
            client: Client::new(),
            state: None,
            account_details: ExtensionAccountDetail {
                id: "youtube".into(),
                package_name: "youtube".into(),
                name: "Youtube".into(),
                bg_color: "".into(),
                icon: "".into(),
                logged_in: false,
                username: None,
            },
        }
    }

    /// Generates the URL to which the user should be redirected to authorize the app.
    pub fn get_authorization_url(&mut self, state: &str) -> String {
        let mut url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
        self.state = Some(state.to_string());
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("response_type", "code")
            .append_pair(
                "scope",
                "https://www.googleapis.com/auth/youtube.readonly openid profile email",
            )
            .append_pair("access_type", "offline")
            .append_pair("state", state)
            .append_pair("prompt", "consent");
        url.into()
    }

    /// Exchanges the authorization code for access and refresh tokens.
    pub async fn exchange_code_for_token(&mut self, code: &str) -> Result<(), Box<dyn Error>> {
        let code = self.validate_and_extract_code(code)?;
        let params = [
            ("code", &code),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("redirect_uri", &self.redirect_uri),
            ("grant_type", &"authorization_code".to_owned()),
        ];

        let resp = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(format!("Failed to get token: {:?}", resp.text().await?).into());
        }

        let token_response: OAuthTokenResponse = resp.json().await?;
        self.access_token = Some(token_response.access_token.clone());
        self.refresh_token = token_response.refresh_token.clone();

        moosync_edk::api::extension_api::set_secure(PreferenceData {
            key: "tokens".to_string(),
            value: Some(token_response.into()),
            ..Default::default()
        })
        .unwrap();

        // Fetch user info and store user_id and username
        self.fetch_and_update_user_info().await?;

        Ok(())
    }

    /// Validates the state and extracts the code from the OAuth redirect query string.
    pub fn validate_and_extract_code(&self, query: &str) -> Result<String, &'static str> {
        let query = query.strip_prefix('?').unwrap_or(query);

        let mut code = None;
        let mut state = None;

        for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
            match key.as_ref() {
                "code" => code = Some(value.into_owned()),
                "state" => state = Some(value.into_owned()),
                _ => {}
            }
        }

        let expected_state = self.state.clone();
        if expected_state.is_none() {
            return Err("Missing expected state");
        }

        match (code, state) {
            (Some(code), Some(state_val)) if state_val == expected_state.unwrap() => Ok(code),
            (Some(_), Some(_)) => Err("Invalid state parameter"),
            _ => Err("Missing code or state parameter"),
        }
    }

    /// Loads tokens from disk if available and updates user info.
    pub async fn load_tokens(&mut self) -> Result<(), Box<dyn Error>> {
        match moosync_edk::api::extension_api::get_secure(PreferenceData {
            key: "tokens".into(),
            ..Default::default()
        }) {
            Ok(data) => {
                if let Some(data_val) = data.value {
                    match OAuthTokenResponse::try_from(data_val) {
                        Ok(token_response) => {
                            self.access_token = Some(token_response.access_token);
                            self.refresh_token = token_response.refresh_token;
                            // Always refresh the access token before fetching user info
                            self.refresh_access_token().await?;
                            self.fetch_and_update_user_info().await?;
                        }
                        Err(e) => {
                            moosync_edk::error!("Failed to parse token_response {:?}", e);
                        }
                    }
                } else {
                    moosync_edk::error!("token_response not found");
                }
            }
            Err(e) => {
                moosync_edk::error!("GetSecure failed: {:?}", e);
            }
        }
        Ok(())
    }

    /// Logs out by removing the stored tokens.
    pub fn logout(&mut self) -> Result<(), Box<dyn Error>> {
        self.access_token = None;
        self.refresh_token = None;
        self.account_details.logged_in = false;
        self.account_details.username = None;
        moosync_edk::api::extension_api::set_secure(PreferenceData {
            key: "tokens".to_string(),
            value: None,
            ..Default::default()
        })
        .unwrap();
        Ok(())
    }

    /// Refreshes the access token using the refresh token.
    pub async fn refresh_access_token(&mut self) -> Result<(), Box<dyn Error>> {
        let refresh_token = match &self.refresh_token {
            Some(token) => token,
            None => return Err("No refresh token available".into()),
        };

        let params = [
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("refresh_token", refresh_token),
            ("grant_type", &"refresh_token".to_owned()),
        ];

        let resp = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(format!("Failed to refresh token: {:?}", resp.text().await?).into());
        }

        let token_response: OAuthTokenResponse = resp.json().await?;
        self.access_token = Some(token_response.access_token.clone());

        moosync_edk::api::extension_api::set_secure(PreferenceData {
            key: "tokens".to_string(),
            value: Some(token_response.into()),
            ..Default::default()
        })
        .unwrap();

        Ok(())
    }

    /// Gets a list of all the user's playlists, paginated, as Playlist.
    pub async fn get_all_playlists(&self) -> Result<Vec<Playlist>, Box<dyn Error>> {
        if self.access_token.is_none() {
            return Err("Not authenticated".into());
        }
        let access_token = match &self.access_token {
            Some(token) => token,
            None => return Err("Not authenticated".into()),
        };

        let mut all_playlists = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut req = self
                .client
                .get("https://www.googleapis.com/youtube/v3/playlists")
                .query(&[
                    ("part", "snippet,contentDetails"),
                    ("mine", "true"),
                    ("maxResults", "50"),
                ])
                .bearer_auth(access_token);

            if let Some(ref token) = page_token {
                req = req.query(&[("pageToken", token)]);
            }

            let resp = req.send().await?;
            if !resp.status().is_success() {
                return Err(format!("Failed to fetch playlists: {:?}", resp.text().await?).into());
            }

            let playlists: PlaylistListResponse = resp.json().await?;
            if let Some(items) = playlists.items {
                all_playlists.extend(items.into_iter().map(Playlist::from));
            }

            if let Some(next_token) = playlists.next_page_token {
                page_token = Some(next_token);
            } else {
                break;
            }
        }

        Ok(all_playlists)
    }

    /// Fetches videos from a playlist, paginated. Returns (Vec<Song>, Option<nextPageToken>)
    pub async fn get_playlist_content(
        &self,
        playlist_id: &str,
        next_page_token: Option<String>,
    ) -> Result<(Vec<moosync_edk::Song>, Option<String>), Box<dyn Error>> {
        if self.access_token.is_none() {
            return Err("Not authenticated".into());
        }
        let access_token = match &self.access_token {
            Some(token) => token,
            None => return Err("Not authenticated".into()),
        };

        let mut req = self
            .client
            .get("https://www.googleapis.com/youtube/v3/playlistItems")
            .query(&[
                ("part", "snippet,contentDetails"),
                ("playlistId", playlist_id),
                ("maxResults", "50"),
            ])
            .bearer_auth(access_token);

        if let Some(ref token) = next_page_token {
            req = req.query(&[("pageToken", token)]);
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(
                format!("Failed to fetch playlist videos: {:?}", resp.text().await?).into(),
            );
        }

        let playlist_items: PlaylistItemsListResponse = resp.json().await?;
        let songs = playlist_items
            .items
            .unwrap_or_default()
            .into_iter()
            .map(moosync_edk::Song::from)
            .collect();

        Ok((songs, playlist_items.next_page_token))
    }

    /// Fetch user info and update the struct's account_details.
    pub async fn fetch_and_update_user_info(&mut self) -> Result<(), Box<dyn Error>> {
        if let Some(access_token) = &self.access_token {
            info!("Using access token: {:?}", access_token);
            let user_resp = self
                .client
                .get("https://www.googleapis.com/oauth2/v2/userinfo")
                .bearer_auth(access_token)
                .send()
                .await?;

            let status = user_resp.status();
            info!("Got response {:?}", status);
            if status.is_success() {
                #[derive(Deserialize)]
                struct UserInfo {
                    name: Option<String>,
                }
                let user_info: UserInfo = user_resp.json().await?;
                self.account_details.username = user_info.name;
                self.account_details.logged_in = true;
            } else {
                let text = user_resp.text().await?;
                moosync_edk::error!("Failed to fetch user info: {}", text);
            }
        }
        Ok(())
    }
}
