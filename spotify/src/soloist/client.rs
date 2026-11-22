use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SoloistWsCommand<'a> {
    #[serde(rename = "type")]
    pub msg_type: &'a str,
    pub command: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<u32>,
}

impl<'a> SoloistWsCommand<'a> {
    pub fn new(command: &'a str) -> Self {
        Self {
            msg_type: "command",
            command,
            uri: None,
            position_ms: None,
            volume: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoloistPosition {
    #[serde(default)]
    pub position_ms: u64,
    #[serde(default)]
    pub timestamp_ms: Option<u64>,
    #[serde(default)]
    pub speed: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoloistTrackItem {
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoloistContext {
    #[serde(default)]
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoloistOptions {
    #[serde(default)]
    pub shuffling: Option<bool>,
    #[serde(default)]
    pub repeating_context: Option<bool>,
    #[serde(default)]
    pub repeating_track: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SoloistWsEvent {
    AuthState {
        #[serde(default)]
        logged_in: bool,
        #[serde(default)]
        device_name: Option<String>,
        #[serde(default)]
        is_active: Option<bool>,
    },
    PlaybackState {
        #[serde(default)]
        device_name: Option<String>,
        #[serde(default)]
        is_active: Option<bool>,
        #[serde(default)]
        logged_in: Option<bool>,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        item: Option<SoloistTrackItem>,
        #[serde(default)]
        context: Option<SoloistContext>,
        #[serde(default)]
        position: Option<SoloistPosition>,
        #[serde(default)]
        volume: Option<u32>,
        #[serde(default)]
        options: Option<SoloistOptions>,
    },
    PlaybackChanged {
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        is_active: Option<bool>,
        #[serde(default)]
        position: Option<SoloistPosition>,
        #[serde(default)]
        item: Option<SoloistTrackItem>,
    },
    PositionSync {
        position: SoloistPosition,
    },
    DeviceChanged {
        #[serde(default)]
        device_name: Option<String>,
        #[serde(default)]
        is_active: Option<bool>,
    },
    TrackChanged {
        #[serde(default)]
        item: Option<SoloistTrackItem>,
        #[serde(default)]
        position: Option<SoloistPosition>,
    },
    ContextChanged {
        #[serde(default)]
        context: Option<SoloistContext>,
    },
    OptionsChanged {
        #[serde(default)]
        options: Option<SoloistOptions>,
    },
    QueueChanged {},
    VolumeChanged {
        #[serde(default)]
        volume: Option<u32>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoloistSnapshot {
    #[serde(default)]
    pub device_name: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub logged_in: Option<bool>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub item: Option<SoloistTrackItem>,
    #[serde(default)]
    pub context: Option<SoloistContext>,
    #[serde(default)]
    pub position: Option<SoloistPosition>,
    #[serde(default)]
    pub volume: Option<u32>,
    #[serde(default)]
    pub options: Option<SoloistOptions>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SoloistWsResponse {
    Event(SoloistWsEvent),
    Snapshot(SoloistSnapshot),
}
