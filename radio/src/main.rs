use futures::executor::block_on;
use moosync_edk::{
    ExtensionProviderScope, InnerSong, MoosyncError, MoosyncResult, SearchResult, Song, SongType,
    api::{
        Accounts, ContextMenu, DatabaseEvents, Extension, PlayerEvents, PreferenceEvents, Provider,
    },
    error,
    handler::register_extension,
    info,
};
use radiobrowser::{ApiStation, RadioBrowserAPI};
use regex::Regex;

#[derive(Debug)]
struct ParsedSearchTokens {
    country: Option<String>,
    country_code: Option<String>,
    tags: Option<Vec<String>>,
    language: Option<String>,
    name: String,
}

struct RadioExtension {
    rb: Option<RadioBrowserAPI>,
}

impl RadioExtension {
    fn new() -> Self {
        match block_on(RadioBrowserAPI::new()) {
            Ok(rb) => Self { rb: Some(rb) },
            Err(e) => {
                error!("Failed to initialize radiobrowser instance: {:?}", e);
                // Continue without radiobrowser instance, search will fail gracefully
                Self { rb: None }
            }
        }
    }

    fn parse_search_input(&self, input: String) -> ParsedSearchTokens {
        let re = Regex::new(
                r#"^"?(?:country:(?P<country>\S+)(?:[\s"]+|$))?(?:country_code:(?P<country_code>\S+)(?:[\s"]+|$))?(?:tags:(?P<tags>\S+)(?:[\s"]+|$))?(?:language:(?P<language>\S+)(?:[\s"]+|$))?(?P<others>.*?)"?$"#
            ).unwrap();

        let caps = re
            .captures(&input)
            .expect("Input didn't match expected format");

        let country = caps
            .name("country")
            .map(|m| m.as_str().trim_end_matches('"').to_string());
        let country_code = caps
            .name("country_code")
            .map(|m| m.as_str().trim_end_matches('"').to_string());
        let tags = caps.name("tags").map(|m| {
            m.as_str()
                .trim_end_matches('"')
                .split(',')
                .map(|s| s.to_string())
                .collect()
        });
        let language = caps
            .name("language")
            .map(|m| m.as_str().trim_end_matches('"').to_string());
        let name = caps
            .name("others")
            .map_or("".to_string(), |m| m.as_str().to_string());

        let ret = ParsedSearchTokens {
            country,
            country_code,
            tags,
            language,
            name,
        };
        ret
    }

    fn stations_to_songs(&self, stations: Vec<ApiStation>) -> Vec<Song> {
        stations
            .into_iter()
            .map(|s| Song {
                song: Some(InnerSong {
                    id: Some(format!("radio-{}", s.serveruuid.unwrap_or(s.url.clone()))),
                    title: Some(s.name),
                    bitrate: Some(s.bitrate as f64),
                    codec: Some(s.codec),
                    duration: Some(moosync_edk::duration_to_proto(std::time::Duration::from_secs(0))),
                    r#type: SongType::Url.into(),
                    url: Some(s.url),
                    playback_url: Some(s.url_resolved),
                    song_cover_path_low: Some(s.favicon),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .collect()
    }
}

impl PlayerEvents for RadioExtension {}
impl Provider for RadioExtension {
    fn get_provider_scopes(&self) -> MoosyncResult<Vec<ExtensionProviderScope>> {
        Ok(vec![ExtensionProviderScope::Search])
    }

    fn search(
        &self,
        req: moosync_edk::api::RequestedSearchResultRequest,
    ) -> MoosyncResult<SearchResult> {
        let rb = self
            .rb
            .as_ref()
            .ok_or(MoosyncError::String("Radio browser not initialized".into()))?;

        let data = self.parse_search_input(req.query);
        let mut builder = rb.get_stations();
        if let Some(country) = data.country {
            builder = builder.country(capitalize_first(&country));
        }

        if let Some(country_code) = data.country_code {
            builder = builder.countrycode(country_code.to_uppercase());
        }

        if let Some(language) = data.language {
            builder = builder.language(language);
        }

        if let Some(tags) = data.tags {
            builder = builder.tag_list(tags.iter().map(|s| s.as_str()).collect());
        }

        if !data.name.is_empty() {
            builder = builder.name(data.name);
        }

        builder = builder.limit("50");

        let resp = block_on(builder.send()).map_err(|e| MoosyncError::String(e.to_string()))?;
        Ok(SearchResult {
            songs: self.stations_to_songs(resp),
            ..Default::default()
        })
    }

    fn get_song_from_url(
        &self,
        req: moosync_edk::api::RequestedSongFromUrlRequest,
    ) -> MoosyncResult<Option<Song>> {
        info!("Got URL {}", req.url);
        Ok(None)
    }
}
impl DatabaseEvents for RadioExtension {}
impl PreferenceEvents for RadioExtension {}
impl ContextMenu for RadioExtension {}
impl Accounts for RadioExtension {}
impl Extension for RadioExtension {}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn init() {
    info!("Initializing RadioExtension");

    register_extension(Box::new(RadioExtension::new())).unwrap();

    info!("Initialized RadioExtension");
}

fn main() {}
