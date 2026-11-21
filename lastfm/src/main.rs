use std::sync::Mutex;

use client::{Builder, Client};
use moosync_edk::{
    api::{
        self, Accounts, ContextMenu, DatabaseEvents, Extension, PlayerEvents, PlayerProvider,
        PreferenceEvents, Provider,
    },
    error,
    handler::register_extension,
    info,
    types::{
        ExtensionAccountDetail, GetCurrentSongRequest, GetSecureRequest, OauthCallbackRequest,
        OauthCallbackResponse, OpenExternalUrlRequest, PerformAccountLoginRequest,
        PerformAccountLoginResponse, PreferenceData, RegisterOauthRequest, ScrobbleRequest,
        SetAccountRequest, SetSecureRequest, SongChangedRequest,
    },
    warn, MoosyncResult,
};

mod client;

struct LastFMExtension {
    client: Mutex<Client>,
}

impl LastFMExtension {
    fn sync_account(&self) {
        let client = self.client.lock().unwrap();
        let username = client.get_username();
        let _ = api::set_account(SetAccountRequest {
            account: Some(ExtensionAccountDetail {
                id: "lastfm".into(),
                package_name: "lastfm".into(),
                name: "Last.fm".into(),
                bg_color: "red".into(),
                icon: "".into(),
                logged_in: username.is_some(),
                username,
            }),
        });
    }

    fn fetch_session(&self) -> MoosyncResult<()> {
        if let Ok(resp) = api::get_secure(GetSecureRequest {
            data: Some(PreferenceData {
                key: "session".to_string(),
                value: None,
            }),
        }) {
            if let Some(session) = resp.data.and_then(|d| d.value) {
                if let Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::StringValue(session_str)) = session.kind {
                     if let Ok(parsed_session) = serde_json::from_str(&session_str) {
                         {
                             let mut client = self.client.lock().unwrap();
                             client.set_session(parsed_session);
                         }
                         self.sync_account();
                     } else {
                         error!("Failed to parse existing sessions {:?}", session_str);
                     }
                 }
            } else {
                warn!("Session could not be retrieved");
            }
        }

        Ok(())
    }
}

impl LastFMExtension {
    pub fn new() -> Self {
        Self {
            client: Mutex::new(
                Builder::new()
                    .with_api_key("85ac9a982f718673f1cdbb9d8d6fba19")
                    .with_secret("f075c733fe5c8bb55fbb338f2ac739b0")
                    .build(),
            ),
        }
    }
}

impl PlayerEvents for LastFMExtension {
    fn song_changed(&self, _: SongChangedRequest) -> MoosyncResult<()> {
        let client = self.client.lock().unwrap();
        if let Ok(resp) = api::get_current_song(GetCurrentSongRequest {}) {
            if let Some(current_song) = resp.song {
                client.set_now_playing(current_song);
            }
        }

        Ok(())
    }
}
impl Provider for LastFMExtension {
    fn scrobble(&self, req: ScrobbleRequest) -> MoosyncResult<()> {
        let client = self.client.lock().unwrap();
        if let Some(song) = req.song {
            client.scrobble(song);
        }
        Ok(())
    }
}
impl PlayerProvider for LastFMExtension {}
impl DatabaseEvents for LastFMExtension {}
impl PreferenceEvents for LastFMExtension {}
impl ContextMenu for LastFMExtension {}
impl Extension for LastFMExtension {}
impl Accounts for LastFMExtension {
    fn perform_account_login(
        &self,
        args: PerformAccountLoginRequest,
    ) -> MoosyncResult<PerformAccountLoginResponse> {
        info!("Performing account login {}", args.login_status);
        if args.login_status {
            let res = {
                let client = self.client.lock().unwrap();
                client.login()
            };
            info!("opening url {}", res);
            api::open_external_url(OpenExternalUrlRequest { url: res.clone() })?;
            Ok(PerformAccountLoginResponse {
                url: res,
                device_code: None,
            })
        } else {
            {
                let mut client = self.client.lock().unwrap();
                client.logout();
            }
            api::set_secure(SetSecureRequest {
                data: Some(PreferenceData {
                    key: "session".to_string(),
                    value: None,
                }),
            })?;
            self.sync_account();
            Ok(PerformAccountLoginResponse {
                url: String::new(),
                device_code: None,
            })
        }
    }

    fn oauth_callback(&self, req: OauthCallbackRequest) -> MoosyncResult<OauthCallbackResponse> {
        info!("Got oauth callback {:?}", req);
        {
            let mut client = self.client.lock().unwrap();
            match client.authorize(req.callback_uri) {
                Ok(s) => {
                    if let Err(e) = api::set_secure(SetSecureRequest {
                        data: Some(PreferenceData {
                            key: "session".to_string(),
                            value: Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::Value {
                                 kind: Some(moosync_edk::extensions_proto::struct_proto::google::protobuf::value::Kind::StringValue(serde_json::to_string(&s).unwrap())),
                            }),
                        }),
                    }) {
                        error!("Failed to set lastfm token in secure store {}", e);
                    }
                }
                Err(e) => {
                    error!("Error while authorizing: {}", e);
                    return Err(e);
                }
            }
        }

        self.sync_account();
        Ok(OauthCallbackResponse {
            extra_content: None,
        })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    info!("Initializing LastFMExtension");

    let extension = LastFMExtension::new();
    extension.fetch_session().unwrap();
    extension.sync_account();

    register_extension(Box::new(extension)).unwrap();

    if let Err(e) = api::register_oauth(RegisterOauthRequest {
        url: "lastfmcallback".to_string(),
    }) {
        error!("Failed to register oauth callback {:?}", e);
    }

    if let Err(e) = api::register_oauth(RegisterOauthRequest {
        url: "lastfm".to_string(),
    }) {
        error!("Failed to register oauth callback {:?}", e);
    }

    info!("Initialized SampleExtension");
}

fn main() {}
