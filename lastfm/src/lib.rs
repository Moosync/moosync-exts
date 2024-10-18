use std::sync::Mutex;

use client::{Builder, Client};
use moosync_edk::{
    api::{
        extension_api::{
            self, get_current_song, get_secure, register_oauth, set_secure, update_accounts,
        },
        Accounts, DatabaseEvents, Extension, PlayerEvents, PreferenceEvents, Provider,
    },
    error,
    handler::register_extension,
    info, ExtensionAccountDetail, ExtensionProviderScope, MoosyncResult, MoosyncResult as Result,
    PreferenceData, Song,
};

mod client;

struct SampleExtension {
    client: Mutex<Client>,
}

impl SampleExtension {
    fn fetch_session(&self) -> MoosyncResult<()> {
        if let Ok(session) = get_secure(PreferenceData {
            key: "session".to_string(),
            value: None,
            default_value: None,
        }) {
            if let Ok(parsed_session) = serde_json::from_value(session.clone()) {
                let mut client = self.client.lock().unwrap();
                client.set_session(parsed_session);
                update_accounts().unwrap();
            } else {
                error!("Failed to parse existing sessions {:?}", session);
            }
        }

        Ok(())
    }
}

impl SampleExtension {
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

impl PlayerEvents for SampleExtension {
    fn on_song_changed(&self) -> MoosyncResult<()> {
        let client = self.client.lock().unwrap();
        if let Ok(Some(current_song)) = get_current_song() {
            client.set_now_playing(current_song);
        }

        Ok(())
    }
}
impl Provider for SampleExtension {
    fn get_provider_scopes(&self) -> Result<Vec<ExtensionProviderScope>> {
        Ok(vec![ExtensionProviderScope::Scrobbles])
    }

    fn scrobble(&self, song: Song) -> Result<()> {
        let client = self.client.lock().unwrap();
        client.scrobble(song);
        Ok(())
    }
}
impl DatabaseEvents for SampleExtension {}
impl PreferenceEvents for SampleExtension {}
impl Extension for SampleExtension {}
impl Accounts for SampleExtension {
    fn get_accounts(&self) -> MoosyncResult<Vec<moosync_edk::ExtensionAccountDetail>> {
        let client = self.client.lock().unwrap();
        let username = client.get_username();
        Ok(vec![ExtensionAccountDetail {
            id: "lastfm".into(),
            package_name: "lastfm".into(),
            name: "Last.fm".into(),
            bg_color: "red".into(),
            icon: "".into(),
            logged_in: username.is_some(),
            username,
        }])
    }

    fn perform_account_login(&self, args: moosync_edk::AccountLoginArgs) -> MoosyncResult<()> {
        info!("Performing account login {}", args.login_status);
        if args.login_status {
            let client = self.client.lock().unwrap();
            let res = client.login();
            info!("opening url {}", res);
            extension_api::open_external_url(res).unwrap();
        }

        Ok(())
    }

    fn oauth_callback(&self, code: String) -> MoosyncResult<()> {
        info!("Got oauth callback {}", code);
        let mut client = self.client.lock().unwrap();
        match client.authorize(code) {
            Ok(s) => {
                if let Err(e) = set_secure(PreferenceData {
                    key: "session".to_string(),
                    value: Some(serde_json::to_value(&s).unwrap()),
                    default_value: None,
                }) {
                    error!("Failed to set lastfm token in secure store {}", e);
                }
            }
            Err(e) => {
                error!("Error while authorizing: {}", e);
                return Err(e);
            }
        }

        update_accounts()
    }
}

#[no_mangle]
pub extern "C" fn init() {
    info!("Initializing SampleExtension");

    let extension = SampleExtension::new();
    extension.fetch_session().unwrap();

    register_extension(Box::new(extension)).unwrap();

    if let Err(e) = register_oauth("lastfmcallback".to_string()) {
        error!("Failed to register oauth callback {:?}", e);
    }
    info!("Initialized SampleExtension");
}
