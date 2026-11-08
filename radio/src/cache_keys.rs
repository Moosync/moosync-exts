pub const SEARCH_PREFIX: &str = "search:";

pub fn search(query: &str) -> String {
    format!("{SEARCH_PREFIX}{}", query.trim().to_lowercase())
}
