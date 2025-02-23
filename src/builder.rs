use crate::defaults::DEFAULT_LOGGING_BUFFER_SIZE;
use crate::layer::QuickwitLoggingLayer;
use crate::message::QuickwitLogMessage;
use crate::ndjson;
use reqwest::Client;
use std::collections::HashMap;
use std::future::Future;
use tokio::sync::mpsc;
use url::Url;

#[cfg(feature = "testing-extras")]
use std::sync::Arc;
#[cfg(feature = "testing-extras")]
use tokio::sync::Notify;

pub struct QuickwitLoggingLayerBuilder {
    quickwit_url: Url,
    target_field: String,
    // TODO: Consider `&'static str`.
    field_to_index: HashMap<String, String>,
    batch_size: usize,
    on_index_missing: Box<dyn Fn() + Send + Sync + 'static>,
    on_ingest_failed: Box<dyn Fn(reqwest::Error) + Send + Sync + 'static>,
    #[cfg(feature = "testing-extras")]
    expected_emitted_events_count: usize,
    #[cfg(feature = "testing-extras")]
    emitted_all: Arc<Notify>,
}

impl QuickwitLoggingLayerBuilder {
    // TODO: Check that `quickwit_url` is reachable and warn if it isn't.
    pub fn new(quickwit_url: impl Into<Url>) -> Self {
        Self {
            quickwit_url: quickwit_url.into(),
            target_field: String::new(),
            field_to_index: HashMap::new(),
            batch_size: DEFAULT_LOGGING_BUFFER_SIZE,
            on_index_missing: Box::new(|| ()),
            on_ingest_failed: Box::new(|_err| ()),
            #[cfg(feature = "testing-extras")]
            expected_emitted_events_count: 0,
            #[cfg(feature = "testing-extras")]
            emitted_all: Arc::new(Notify::new()),
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

    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    pub fn on_index_missing(mut self, callback: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_index_missing = Box::new(callback);
        self
    }

    pub fn on_ingest_failed(
        mut self,
        callback: impl Fn(reqwest::Error) + Send + Sync + 'static,
    ) -> Self {
        self.on_ingest_failed = Box::new(callback);
        self
    }

    #[cfg(feature = "testing-extras")]
    pub fn with_expected_emitted_events_count(mut self, count: usize) -> Self {
        self.expected_emitted_events_count = count;
        self
    }

    #[cfg(feature = "testing-extras")]
    pub fn on_emitted_all(mut self, notifier: Arc<Notify>) -> Self {
        self.emitted_all = notifier;
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
                let buffer = match buffers.get_mut(&index_id) {
                    Some(buffer) => buffer,
                    None => buffers
                        .entry(index_id.clone())
                        .or_insert(Vec::with_capacity(self.batch_size)),
                };
                buffer.push(log);
                if buffer.len() >= self.batch_size {
                    // TODO: Reuse `ndjson_body`.
                    let mut ndjson_body = Vec::new();
                    for log in buffer.iter() {
                        ndjson::serialize(&mut ndjson_body, log).unwrap();
                    }
                    let response = http_client
                        .post(format!("{}api/v1/{}/ingest", self.quickwit_url, index_id))
                        .body(ndjson_body)
                        .send()
                        .await;
                    if let Err(err) = response {
                        (self.on_ingest_failed)(err);
                    }
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
        let layer = QuickwitLoggingLayer::new(
            sender,
            self.target_field,
            self.field_to_index,
            self.on_index_missing,
            #[cfg(feature = "testing-extras")]
            self.expected_emitted_events_count,
            #[cfg(feature = "testing-extras")]
            self.emitted_all,
        );
        (layer, background_task)
    }
}
