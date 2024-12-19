use crate::tests::quickwit::TestHttpServer;
use crate::QuickwitLoggingLayerBuilder;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Notify;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use url::Url;

pub(crate) struct TestEnvironment {
    pub(crate) quickwit_server: TestHttpServer,
}

impl TestEnvironment {
    pub(crate) fn builder() -> TestEnvironmentBuilder<'static> {
        TestEnvironmentBuilder {
            quickwit_subscriber_channel_capacity: 1,
            expected_events_count: 0,
            quickwit_port: 9011,
            marker_field: "task",
            marker_to_index_mapping: HashMap::new(),
        }
    }
}

pub(crate) struct TestEnvironmentBuilder<'mf> {
    quickwit_subscriber_channel_capacity: usize,
    expected_events_count: usize,
    quickwit_port: u16,
    marker_field: &'mf str,
    marker_to_index_mapping: HashMap<&'mf str, &'mf str>,
}

impl<'mf> TestEnvironmentBuilder<'mf> {
    pub(crate) fn with_quickwit_subscriber_channel_capacity(mut self, capacity: usize) -> Self {
        self.quickwit_subscriber_channel_capacity = capacity;
        self
    }

    pub(crate) fn with_expected_events_count(mut self, count: usize) -> Self {
        self.expected_events_count = count;
        self
    }

    pub(crate) fn with_quickwit_port(mut self, port: u16) -> Self {
        self.quickwit_port = port;
        self
    }

    pub(crate) fn with_marker_field(mut self, field_name: &'mf str) -> Self {
        self.marker_field = field_name;
        self
    }

    pub(crate) fn with_marker_to_index_mapping(
        mut self,
        marker_name: &'mf str,
        index_name: &'mf str,
    ) -> Self {
        self.marker_to_index_mapping.insert(marker_name, index_name);
        self
    }

    pub(crate) async fn build(self) -> TestEnvironment {
        let mut quickwit_url =
            Url::from_str("http://127.0.0.1").expect("Failed to parse fake Quickwit server URL!");
        quickwit_url
            .set_port(Some(self.quickwit_port))
            .expect("Failed to set the port for Quickwit URL!");
        let mut quickqit_layer_builder = QuickwitLoggingLayerBuilder::new(quickwit_url)
            .marker_field(self.marker_field)
            .with_batch_size(self.quickwit_subscriber_channel_capacity);
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
            .init();

        let quickwit_server = TestHttpServer::new(self.quickwit_port, self.expected_events_count);
        quickwit_server.wait_until_ready().await;

        TestEnvironment { quickwit_server }
    }
}
