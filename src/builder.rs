use crate::defaults::DEFAULT_LOGGING_BUFFER_SIZE;
use crate::layer::QuickwitLoggingLayer;
use crate::message::QuickwitLogMessage;
use crate::ndjson;
use reqwest::Client;
use std::collections::HashMap;
use std::future::Future;
use tokio::sync::mpsc;
use url::Url;

pub struct QuickwitLoggingLayerBuilder {
    quickwit_url: Url,
    target_field: String,
    // TODO: Consider `&'static str`.
    field_to_index: HashMap<String, String>,
    batch_size: usize,
}

impl QuickwitLoggingLayerBuilder {
    // TODO: Check that `quickwit_url` is reachable and warn if it isn't.
    pub fn new(quickwit_url: impl Into<Url>) -> Self {
        Self {
            quickwit_url: quickwit_url.into(),
            target_field: String::new(),
            field_to_index: HashMap::new(),
            batch_size: DEFAULT_LOGGING_BUFFER_SIZE,
        }
    }

    pub fn marker_field(mut self, field: impl Into<String>) -> Self {
        self.target_field = field.into();
        self
    }

    pub fn map_marker_to_index<S: Into<String>>(mut self, field_value: S, index_id: S) -> Self {
        self.field_to_index
            .insert(field_value.into(), index_id.into());
        self
    }

    // TODO: Figure out a way to accept hardcoded `i32` if it is valid `usize` (`i32` isn't convertible to `usize` so
    // the client can't write `.with_batch_size(100)`.
    pub fn with_batch_size(mut self, batch_size: impl Into<usize>) -> Self {
        self.batch_size = batch_size.into();
        self
    }

    pub fn build(self) -> (QuickwitLoggingLayer, impl Future<Output = impl Send> + Send) {
        let http_client = Client::new();
        // TODO: Capacity should be configurable.
        let (sender, mut receiver) = mpsc::channel(500);
        let index_names = self.field_to_index.values().cloned().collect::<Vec<_>>();
        let background_task = async move {
            let mut buffers = HashMap::new();
            for index_name in index_names {
                buffers.insert(index_name, Vec::with_capacity(self.batch_size));
            }
            while let Some(QuickwitLogMessage { index_id, log }) = receiver.recv().await {
                // TODO: This `.unwrap()` is bad (can break the whole task).
                let buffer = buffers.get_mut(&index_id).unwrap();
                (*buffer).push(log);
                if buffer.len() >= self.batch_size {
                    // TODO: Reuse `ndjson_body`.
                    let mut ndjson_body = Vec::new();
                    for log in buffer.iter() {
                        ndjson::serialize(&mut ndjson_body, log).unwrap();
                    }
                    // TODO: Provide a user-specified `on_error()` callback here.
                    let _response = http_client
                        .post(format!("{}api/v1/{}/ingest", self.quickwit_url, index_id))
                        .body(ndjson_body)
                        .send()
                        .await;
                    buffer.clear();
                }
            }
            for (index_id, buffer) in buffers.iter_mut() {
                let mut ndjson_body = Vec::new();
                for log in buffer.iter() {
                    ndjson::serialize(&mut ndjson_body, log).unwrap();
                }
                let _response = http_client
                    .post(format!("{}api/v1/{}/ingest", self.quickwit_url, index_id))
                    .body(ndjson_body)
                    .send()
                    .await;
                buffer.clear();
            }
        };
        let layer = QuickwitLoggingLayer::new(sender, self.target_field, self.field_to_index);
        (layer, background_task)
    }
}
