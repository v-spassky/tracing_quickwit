use crate::message::QuickwitLogMessage;
use crate::visitor::{LogVisitor, TargetFieldVisitor};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tracing_core::Event;
use tracing_core::Subscriber;
use tracing_subscriber::layer::Context as TracingContext;
use tracing_subscriber::Layer;

pub struct QuickwitLoggingLayer {
    // TODO: Maybe use single producer?
    sender: mpsc::Sender<QuickwitLogMessage>,
    target_field: String,
    // TODO: Consider `&' static` instead of `String`.
    field_to_index: HashMap<String, String>,
}

impl QuickwitLoggingLayer {
    pub(crate) fn new(
        sender: mpsc::Sender<QuickwitLogMessage>,
        target_field: String,
        field_to_index: HashMap<String, String>,
    ) -> Self {
        Self {
            sender,
            target_field,
            field_to_index,
        }
    }

    fn should_log_event(&self, event: &Event<'_>) -> Option<String> {
        let mut visitor = TargetFieldVisitor::new(&self.target_field);
        event.record(&mut visitor);
        visitor
            .target_value
            .and_then(|value| self.field_to_index.get(&value).cloned())
    }
}

impl<S: Subscriber> Layer<S> for QuickwitLoggingLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: TracingContext<'_, S>) {
        if let Some(index_id) = self.should_log_event(event) {
            let mut visitor = LogVisitor::new();
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
}
