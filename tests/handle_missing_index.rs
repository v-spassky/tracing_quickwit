pub mod common;

use common::environment::TestEnvironment;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn handle_missing_index() {
    let missed = Arc::new(AtomicBool::new(false));
    let missed_clone = Arc::clone(&missed);
    let env = TestEnvironment::builder()
        .with_quickwit_subscriber_channel_capacity(1)
        .with_expected_emitted_events_count(1)
        .with_expected_recieved_events_count(0)
        .with_quickwit_port(9025)
        .with_marker_field("some_marker_field")
        .with_marker_to_index_mapping("marker_field_value", "some_index_id")
        .on_index_missing(move || missed_clone.store(true, Ordering::Relaxed))
        .build()
        .await;

    tracing::info!(
        some_marker_field = "WRONG_marker_field_value",
        metric1 = 2145.43,
        metric2 = "done",
    );
    env.wait_until_all_events_emitted().await;

    assert_eq!(
        env.quickwit_server.accepted_requests(),
        Vec::<serde_json::Value>::new(),
    );
    assert_eq!(missed.load(Ordering::Relaxed), true);
}
