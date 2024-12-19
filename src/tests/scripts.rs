use crate::tests::environment::TestEnvironment;
use serde_json::json;

#[tokio::test]
async fn how_to_use_test_environment() {
    let env = TestEnvironment::builder()
        .with_quickwit_subscriber_channel_capacity(1)
        .with_expected_events_count(1)
        .with_quickwit_port(9011)
        .with_marker_field("some_marker_field")
        .with_marker_to_index_mapping("marker_field_value", "some_index_id")
        .build()
        .await;

    tracing::info!(
        some_marker_field = "marker_field_value",
        metric1 = 2145.43,
        metric2 = "done",
    );

    env.quickwit_server
        .wait_until_processed_expected_events_count()
        .await;

    let expected_requests = vec![
        json!({"some_marker_field": "marker_field_value", "metric1": "2145.43", "metric2": "done"}),
    ];
    assert_eq!(env.quickwit_server.accepted_requests(), expected_requests);
}
