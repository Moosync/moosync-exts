use futures::executor::block_on;
use moosync_edk::{
    ExtensionProviderScope, MoosyncError, QueryableSong, Result, SearchResult, Song, SongType,
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
    rb: RadioBrowserAPI,
}

impl RadioExtension {
    fn new() -> Self {
        if let Ok(rb) = block_on(RadioBrowserAPI::new()) {
            Self { rb }
        } else {
            error!("Failed to initialize radiobrowser instance");
            panic!("Failed to initialize radiobrowser instance")
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
                song: QueryableSong {
                    _id: Some(format!("radio-{}", s.serveruuid.unwrap_or(s.url.clone()))),
                    title: Some(s.name),
                    bitrate: Some(s.bitrate as f64),
                    codec: Some(s.codec),
                    duration: Some(0f64),
                    type_: SongType::URL,
                    url: Some(s.url),
                    playback_url: Some(s.url_resolved),
                    song_cover_path_low: Some(s.favicon),
                    ..Default::default()
                },
                ..Default::default()
            })
            .collect()
    }
}

impl PlayerEvents for RadioExtension {}
impl Provider for RadioExtension {
    fn get_provider_scopes(&self) -> Result<Vec<ExtensionProviderScope>> {
        Ok(vec![ExtensionProviderScope::Search])
    }

    fn search(&self, term: String) -> Result<SearchResult> {
        let data = self.parse_search_input(term);
        let mut builder = self.rb.get_stations();
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
