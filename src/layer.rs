use crate::message::QuickwitLogMessage;
use crate::visitor::{LogVisitor, TargetFieldVisitor};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing_core::Event;
use tracing_core::Subscriber;
use tracing_subscriber::layer::Context as TracingContext;
use tracing_subscriber::Layer;

#[cfg(feature = "testing-extras")]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "testing-extras")]
use std::sync::Arc;
#[cfg(feature = "testing-extras")]
use tokio::sync::Notify;

pub struct QuickwitLoggingLayer {
    // TODO: Maybe use single producer?
    sender: mpsc::Sender<QuickwitLogMessage>,
    target_field: String,
    // TODO: Consider `&' static` instead of `String`.
    field_to_index: HashMap<String, String>,
    on_index_missing: Box<dyn Fn() + Send + Sync + 'static>,
    #[cfg(feature = "testing-extras")]
    emitted_events_count: Arc<AtomicUsize>,
    #[cfg(feature = "testing-extras")]
    expected_emitted_events_count: usize,
    #[cfg(feature = "testing-extras")]
    emitted_all: Arc<Notify>,
}

impl QuickwitLoggingLayer {
    pub(crate) fn new(
        sender: mpsc::Sender<QuickwitLogMessage>,
        target_field: String,
        field_to_index: HashMap<String, String>,
        on_index_missing: Box<dyn Fn() + Send + Sync + 'static>,
        #[cfg(feature = "testing-extras")] expected_emitted_events_count: usize,
        #[cfg(feature = "testing-extras")] emitted_all: Arc<Notify>,
    ) -> Self {
        #[cfg(feature = "testing-extras")]
        let emitted_events_count = Arc::new(AtomicUsize::new(0));
        Self {
            sender,
            target_field,
            field_to_index,
            on_index_missing,
            #[cfg(feature = "testing-extras")]
            expected_emitted_events_count,
            #[cfg(feature = "testing-extras")]
            emitted_all,
            #[cfg(feature = "testing-extras")]
            emitted_events_count,
        }
    }

    #[cfg(feature = "testing-extras")]
    fn increment_emitted_events_count(&self) {
        self.emitted_events_count.fetch_add(1, Ordering::SeqCst);
    }

    #[cfg(feature = "testing-extras")]
    fn notify_if_emitted_expected_events_count(&self) {
        let count = self.emitted_events_count.load(Ordering::SeqCst);
        if count >= self.expected_emitted_events_count {
            self.emitted_all.notify_one();
        }
    }
}

impl<S: Subscriber> Layer<S> for QuickwitLoggingLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: TracingContext<'_, S>) {
        #[cfg(feature = "testing-extras")]
        self.increment_emitted_events_count();
        #[cfg(feature = "testing-extras")]
        self.notify_if_emitted_expected_events_count();
        let mut visitor = LogVisitor::new();
        let maybe_marker_field = event
            .fields()
            .find(|field| field.name() == self.target_field);
        if maybe_marker_field.is_none() {
            return;
        }
        let marker_field = maybe_marker_field.unwrap().name();
        let mut target_field_visitor = TargetFieldVisitor::new(marker_field);
        event.record(&mut target_field_visitor);
        if target_field_visitor.target_value.is_none() {
            return;
        }
        let target_value = target_field_visitor.target_value.unwrap();
        let maybe_index_id = self.field_to_index.get(&target_value);
        if maybe_index_id.is_none() {
            (self.on_index_missing)();
            return;
        }
        let index_id = maybe_index_id.unwrap().to_owned();
        event.record(&mut visitor);
        let log_message = QuickwitLogMessage {
            index_id,
            log: visitor.log,
        };
        // TODO: Let the client configure sending strategy (blocking or non-blocking, timeout,
        // `on_error` callback etc.).
        let _ = self.sender.try_send(log_message);
    }
}
