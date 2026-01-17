use moosync_edk::extensions_proto::struct_proto::google::protobuf::{Value, value};

pub fn google_value_to_string(val: Value) -> Option<String> {
    match val.kind {
        Some(value::Kind::StringValue(s)) => Some(s),
        _ => None,
    }
}

pub struct Pagination {
    pub page: i32,
}

pub fn to_moosync_string_error(e: impl std::fmt::Display) -> moosync_edk::MoosyncError {
    moosync_edk::MoosyncError::String(e.to_string())
}

pub fn sanitize_id(id: &str) -> &str {
    return id.strip_prefix("moosync.youtube:").unwrap_or(id);
}
