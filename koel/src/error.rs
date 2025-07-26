use moosync_edk::MoosyncError;
use std::error::Error;
use std::fmt;

/// Custom error type for the Koel extension.
#[derive(Debug)]
pub enum KoelError {
    Http(Box<dyn Error + Send + Sync>),
    Json(Box<dyn Error + Send + Sync>),
    MissingToken,
    NoUsername,
    NoPassword,
    PlaylistError,
    PlaylistContentError,
    SearchFailure,
    Other(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for KoelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KoelError::Http(e) => write!(f, "HTTP error: {}", e),
            KoelError::Json(e) => write!(f, "JSON error: {}", e),
            KoelError::Other(e) => write!(f, "Other error: {}", e),
            KoelError::MissingToken => write!(f, "Missing auth token"),
            KoelError::NoUsername => write!(f, "Missing username for koel"),
            KoelError::NoPassword => write!(f, "Missing password for koel"),
            KoelError::PlaylistError => write!(f, "Could not fetch playlists"),
            KoelError::PlaylistContentError => write!(f, "Could not fetch playlist content"),
            KoelError::SearchFailure => write!(f, "Failed to search"),
        }
    }
}

impl std::error::Error for KoelError {}

impl From<KoelError> for MoosyncError {
    fn from(err: KoelError) -> Self {
        MoosyncError::String(err.to_string())
    }
}

// Convenience conversions for common error types
impl From<reqwest::Error> for KoelError {
    fn from(e: reqwest::Error) -> Self {
        KoelError::Http(Box::new(e))
    }
}

impl From<serde_json::Error> for KoelError {
    fn from(e: serde_json::Error) -> Self {
        KoelError::Json(Box::new(e))
    }
}

impl From<std::io::Error> for KoelError {
    fn from(e: std::io::Error) -> Self {
        KoelError::Other(Box::new(e))
    }
}

impl From<moosync_edk::MoosyncError> for KoelError {
    fn from(e: moosync_edk::MoosyncError) -> Self {
        KoelError::Other(Box::new(e))
    }
}
