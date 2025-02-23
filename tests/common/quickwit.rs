use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::sync::{Arc, Mutex};
use tokio::sync::{oneshot, Notify};

#[derive(Debug, Default)]
pub struct TestHttpServer {
    events: Arc<Mutex<Vec<String>>>,
    ready: Arc<Notify>,
    processed_all: Arc<Notify>,
    shutdown_trigger: Option<oneshot::Sender<()>>,
}

impl TestHttpServer {
    pub fn new(port: u16, expected_events_count: usize) -> Self {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let requests_clone = Arc::clone(&requests);
        let ready = Arc::new(Notify::new());
        let ready_clone = Arc::clone(&ready);
        let processed_all = Arc::new(Notify::new());
        let processed_all_clone = Arc::clone(&processed_all);
        let (shutdown_trigger, shutdown_listener) = oneshot::channel();

        tokio::spawn(async move {
            let service = make_service_fn(|_connection| {
                let requests = Arc::clone(&requests_clone);
                let processed_all = Arc::clone(&processed_all_clone);
                let mut processed_requests = 0_usize;
                async move {
                    Ok::<_, hyper::Error>(service_fn(move |request: Request<Body>| {
                        let requests = Arc::clone(&requests);
                        let processed_all = Arc::clone(&processed_all);
                        async move {
                            let body_bytes = hyper::body::to_bytes(request.into_body()).await?;
                            for raw_event in String::from_utf8_lossy(&body_bytes).lines() {
                                requests.lock().unwrap().push(raw_event.to_string());
                                processed_requests += 1;
                            }
                            if processed_requests >= expected_events_count {
                                processed_all.notify_one();
                            }
                            Ok::<_, hyper::Error>(Response::new(Body::from("OK")))
                        }
                    }))
                }
            });

            let address = ([127, 0, 0, 1], port).into();
            let server = Server::bind(&address)
                .serve(service)
                .with_graceful_shutdown(async {
                    shutdown_listener.await.ok();
                });
            ready_clone.notify_one();

            if let Err(e) = server.await {
                eprintln!("Server error: {}", e);
            }
        });

        Self {
            events: requests,
            shutdown_trigger: Some(shutdown_trigger),
            ready,
            processed_all,
        }
    }

    pub async fn wait_until_ready(&self) {
        self.ready.notified().await;
    }

    pub async fn wait_until_processed_expected_events_count(&self) {
        // TODO: Add a timeout because in some errorneous cases it will never return.
        self.processed_all.notified().await;
    }

    pub fn accepted_requests(&self) -> Vec<serde_json::Value> {
        self.events
            .lock()
            .expect("Failed to acquire a lock on `TestHttpServer.events`!")
            .iter()
            .map(|raw_event| {
                serde_json::from_str(raw_event).expect("Failed to deserialize event body!")
            })
            .collect()
    }
}

impl Drop for TestHttpServer {
    fn drop(&mut self) {
        if let Some(shutdown_trigger) = self.shutdown_trigger.take() {
            shutdown_trigger.send(()).ok();
        }
    }
}
