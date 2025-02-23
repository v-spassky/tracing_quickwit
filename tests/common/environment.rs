use crate::common::quickwit::TestHttpServer;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Notify;
use tracing_quickwit::QuickwitLoggingLayerBuilder;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use url::Url;

pub struct TestEnvironment {
    pub quickwit_server: TestHttpServer,
    all_emitted: Arc<Notify>,
}

impl TestEnvironment {
    pub fn builder() -> TestEnvironmentBuilder<'static> {
        TestEnvironmentBuilder {
            quickwit_subscriber_channel_capacity: 1,
            expected_events_count: 0,
            emitted_events_count: 0,
            quickwit_port: 9011,
            marker_field: "task",
            marker_to_index_mapping: HashMap::new(),
            on_index_missing: Box::new(|| ()),
            on_ingest_failed: Box::new(|_err| ()),
        }
    }

    pub async fn wait_until_all_events_emitted(&self) {
        self.all_emitted.notified().await;
    }
}

pub struct TestEnvironmentBuilder<'mf> {
    quickwit_subscriber_channel_capacity: usize,
    on_index_missing: Box<dyn Fn() + Send + Sync + 'static>,
    on_ingest_failed: Box<dyn Fn(reqwest::Error) + Send + Sync + 'static>,
    expected_events_count: usize,
    emitted_events_count: usize,
    quickwit_port: u16,
    marker_field: &'mf str,
    marker_to_index_mapping: HashMap<&'mf str, &'mf str>,
}

impl<'mf> TestEnvironmentBuilder<'mf> {
    pub fn with_quickwit_subscriber_channel_capacity(mut self, capacity: usize) -> Self {
        self.quickwit_subscriber_channel_capacity = capacity;
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

    pub fn with_expected_emitted_events_count(mut self, count: usize) -> Self {
        self.emitted_events_count = count;
        self
    }

    pub fn with_expected_recieved_events_count(mut self, count: usize) -> Self {
        self.expected_events_count = count;
        self
    }

    pub fn with_quickwit_port(mut self, port: u16) -> Self {
        self.quickwit_port = port;
        self
    }

    pub fn with_marker_field(mut self, field_name: &'mf str) -> Self {
        self.marker_field = field_name;
        self
    }

    pub fn with_marker_to_index_mapping(
        mut self,
        marker_name: &'mf str,
        index_name: &'mf str,
    ) -> Self {
        self.marker_to_index_mapping.insert(marker_name, index_name);
        self
    }

    pub async fn build(self) -> TestEnvironment {
        let mut quickwit_url =
            Url::from_str("http://127.0.0.1").expect("Failed to parse fake Quickwit server URL!");
        quickwit_url
            .set_port(Some(self.quickwit_port))
            .expect("Failed to set the port for Quickwit URL!");
        let all_emitted = Arc::new(Notify::new());
        let all_emitted_clone = Arc::clone(&all_emitted);
        let mut quickqit_layer_builder = QuickwitLoggingLayerBuilder::new(quickwit_url)
            .marker_field(self.marker_field)
            .with_batch_size(self.quickwit_subscriber_channel_capacity)
            .with_expected_emitted_events_count(self.emitted_events_count)
            .on_index_missing(self.on_index_missing)
            .on_ingest_failed(self.on_ingest_failed)
            .on_emitted_all(all_emitted_clone);
        for (marker_name, index_name) in self.marker_to_index_mapping {
            quickqit_layer_builder =
                quickqit_layer_builder.map_marker_to_index(marker_name, index_name);
        }
        let (quickwit_logging_layer, quickwit_background_client_task) =
            quickqit_layer_builder.build();

        let background_task_ready = Arc::new(Notify::new());
        let notify_background_task_ready_clone = background_task_ready.clone();
        tokio::spawn(async move {
            // TODO: This probably notifies too early?
            notify_background_task_ready_clone.notify_one();
            quickwit_background_client_task.await;
        });
        background_task_ready.notified().await;

        tracing_subscriber::registry()
            .with(quickwit_logging_layer)
            .try_init()
            .ok();

        let quickwit_server = TestHttpServer::new(self.quickwit_port, self.expected_events_count);
        quickwit_server.wait_until_ready().await;

        TestEnvironment {
            quickwit_server,
            all_emitted,
        }
    }
}
