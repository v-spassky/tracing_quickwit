use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct QuickwitLogMessage {
    pub(crate) index_id: String,
    pub(crate) log: serde_json::Map<String, serde_json::Value>,
}
