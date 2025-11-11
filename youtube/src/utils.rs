use moosync_edk::MoosyncError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub limit: u32,
    pub offset: u32,
    pub token: Option<String>,
    pub is_first: bool,
    pub is_valid: bool,
}

impl Pagination {
    pub fn new_limit(limit: u32, offset: u32) -> Self {
        Pagination {
            limit,
            offset,
            is_first: true,
            is_valid: true,
            ..Default::default()
        }
    }

    pub fn new_token(token: Option<String>) -> Self {
        Pagination {
            token,
            is_first: true,
            is_valid: true,
            ..Default::default()
        }
    }

    pub fn next_page(&self) -> Self {
        Pagination {
            limit: self.limit,
            offset: self.offset + self.limit.max(1),
            token: self.token.clone(),
            is_first: false,
            is_valid: true,
        }
    }

    pub fn next_page_wtoken(&self, token: Option<String>) -> Self {
        Pagination {
            limit: self.limit,
            offset: self.offset + self.limit,
            token,
            is_first: false,
            is_valid: true,
        }
    }

    pub fn invalidate(&mut self) {
        self.is_valid = false;
    }
}

pub fn to_moosync_string_error<E: std::fmt::Debug>(e: E) -> MoosyncError {
    MoosyncError::String(format!("{:?}", e))
}

pub fn sanitize_id(id: &str) -> &str {
    return id.strip_prefix("moosync.youtube:").unwrap_or(id);
}
