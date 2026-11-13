//! The HTTP acceptance tests (api spec, sections 1 to 2).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use axum::body::Body;
use axum::extract::Request;
use axum::http::{Method, StatusCode};
use riehub_server::http::{build_router, RouterInput};
use riehub_server::routes::{RouteClass, RouteSpec};
use riehub_store::Store;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;

fn get(uri: &str) -> Request {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn test_router(table: &'static [RouteSpec], input: RouterInput) -> axum::Router {
    build_router(input, table)
}

#[tokio::test]
async fn healthz_is_200() {
    let router = test_router(riehub_server::routes::TABLE, RouterInput::default());
    let response = router.oneshot(get("/healthz")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn readyz_is_503_when_store_is_down() {
    let router = test_router(riehub_server::routes::TABLE, RouterInput::default());
    let response = router.oneshot(get("/readyz")).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn responses_carry_security_headers() {
    let router = test_router(riehub_server::routes::TABLE, RouterInput::default());
    let response = router.oneshot(get("/healthz")).await.unwrap();
    let headers = response.headers();
    assert!(
        headers.contains_key("strict-transport-security"),
        "HSTS is missing"
    );
    assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
    assert_eq!(headers.get("referrer-policy").unwrap(), "no-referrer");
    assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
    assert!(
        headers.contains_key("x-request-id"),
        "every response carries x-request-id"
    );
}

#[tokio::test]
async fn oversized_body_is_413() {
    let router = test_router(riehub_server::routes::TABLE, RouterInput::default());
    let request = Request::builder()
        .method(Method::POST)
        .uri("/healthz")
        .header("content-length", "33554433")
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn oversized_headers_are_rejected() {
    let router = test_router(riehub_server::routes::TABLE, RouterInput::default());
    let big = "x".repeat(70_000);
    let request = Request::builder()
        .uri("/healthz")
        .header("x-big", big)
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

static SLOW_TABLE: &[RouteSpec] = &[RouteSpec {
    method: &Method::GET,
    path: "/slow",
    class: RouteClass::Public,
    timeout: None,
    handler: |_state, _req| {
        Box::pin(async {
            tokio::time::sleep(Duration::from_millis(200)).await;
            axum::response::Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap()
        })
    },
}];

#[tokio::test]
async fn slow_request_times_out() {
    let router = test_router(
        SLOW_TABLE,
        RouterInput {
            timeout: Duration::from_millis(20),
            ..Default::default()
        },
    );
    let response = router.oneshot(get("/slow")).await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

struct Capture(Arc<std::sync::Mutex<Vec<serde_json::Value>>>);

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Capture {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        struct Visitor(serde_json::Map<String, serde_json::Value>);
        impl tracing::field::Visit for Visitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.0
                    .insert(field.name().to_owned(), format!("{value:?}").into());
            }
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                self.0.insert(field.name().to_owned(), value.into());
            }
            fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
                self.0.insert(field.name().to_owned(), value.into());
            }
            fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
                self.0.insert(field.name().to_owned(), value.into());
            }
        }
        let mut visitor = Visitor(serde_json::Map::new());
        event.record(&mut visitor);
        self.0
            .lock()
            .unwrap()
            .push(serde_json::Value::Object(visitor.0));
    }
}

#[tokio::test]
async fn access_log_has_only_allowlisted_fields() {
    let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
    // The global subscriber collects the access log events of every
    // test in this binary; this test matches its own by request id.
    let _ = tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(Capture(captured.clone())),
    );

    let router = test_router(riehub_server::routes::TABLE, RouterInput::default());
    let response = router.oneshot(get("/healthz")).await.unwrap();
    let my_id = response
        .headers()
        .get("x-request-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();

    let events = captured.lock().unwrap();
    let mine: Vec<&serde_json::Value> = events
        .iter()
        .filter(|e| e["request_id"] == my_id.as_str())
        .collect();
    assert_eq!(mine.len(), 1, "one request makes one access log event");
    let keys = mine[0]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let allowlist = [
        "request_id",
        "method",
        "route_template",
        "status",
        "duration_ms",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(keys, allowlist, "only the allowlisted fields may appear");
    assert_eq!(mine[0]["method"], "GET");
    assert_eq!(mine[0]["route_template"], "/healthz");
    assert_eq!(mine[0]["status"], 200);
}

#[tokio::test]
async fn unknown_route_is_404_problem_json() {
    let router = test_router(riehub_server::routes::TABLE, RouterInput::default());
    let response = router
        .oneshot(get("/api/v1/definitely/not/here"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/problem+json"
    );
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], 404);
    assert!(json["type"].is_string());
    assert!(json["title"].is_string());
    assert!(json["detail"].is_string());
    assert!(json["request_id"].is_string());
}

#[tokio::test]
async fn index_html_is_served_with_csp_and_no_store() {
    let router = test_router(riehub_server::routes::TABLE, RouterInput::default());
    let response = router.oneshot(get("/")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let headers = response.headers().clone();
    assert_eq!(
        headers.get("content-security-policy").map(|v| v.to_str().unwrap()),
        Some(
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'"
        ),
        "every html response carries the CSP of security spec, section 7"
    );
    assert_eq!(
        headers.get("cache-control").map(|v| v.to_str().unwrap()),
        Some("no-store, no-cache"),
        "the index is never cached"
    );
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.starts_with("text/html"),
        "content-type is {content_type}"
    );
    let body = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("<!doctype html>"), "the served index is html");
}

/// The built app must run under the CSP (web spec, section 1): no
/// inline script and no inline style. A plain filesystem scan of
/// `web/dist/index.html`, whatever build is in place. The placeholder
/// satisfies it, the real Vite build must too.
#[test]
fn built_app_has_no_inline_script_or_style() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let dist = std::path::Path::new(manifest).join("../../web/dist");
    let index = dist.join("index.html");
    let html = std::fs::read_to_string(&index).unwrap_or_default();
    assert!(
        !html.is_empty(),
        "web/dist/index.html must exist for the scan"
    );
    assert!(
        !html.contains("<style"),
        "the app must not carry an inline style element"
    );
    assert!(
        !html.contains("style="),
        "the app must not carry inline style attributes"
    );
    let mut rest = html.as_str();
    while let Some(start) = rest.find("<script") {
        let tag = &rest[start..];
        let end = tag.find('>').map(|e| e + 1).unwrap_or(tag.len());
        let tag = &tag[..end];
        assert!(
            tag.contains("src="),
            "every script needs a src attribute, found: {tag}"
        );
        rest = &rest[start + tag.len()..];
    }
    let assets = std::fs::read_dir(dist.join("assets")).expect("built assets exist");
    for asset in assets {
        let asset = asset.expect("asset entry is readable").path();
        let content = std::fs::read_to_string(&asset).unwrap_or_default();
        assert!(
            !content.contains("<style") && !content.contains("style="),
            "{} must not contain inline styles",
            asset.display()
        );
    }
}

#[test]
fn every_route_has_a_class() {
    for spec in riehub_server::routes::TABLE {
        match spec.class {
            RouteClass::Public
            | RouteClass::Self_
            | RouteClass::AdminMeta
            | RouteClass::Key(_)
            | RouteClass::Hook => {}
        }
        assert!(!spec.path.is_empty());
    }
}

#[tokio::test]
async fn serve_writes_system_start_audit_event() {
    let dir = std::env::temp_dir().join(format!(
        "riehub-serve-audit-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let url = format!("sqlite://{}/riehub.db", dir.display());
    let store = Arc::new(riehub_store::sqlite::SqliteStore::open(&url).await.unwrap());
    store.migrate().await.unwrap();
    let mac = riehub_audit::AuditChainMac::new(riehub_crypto::Key32::new([8u8; 32]));
    let audit = riehub_audit::AuditLog::new(store.clone(), mac);

    riehub_server::serve::announce_start(&audit).await.unwrap();

    let rows = store
        .audit_read(1, 100, &riehub_store::audit::AuditFilter::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, "system.start");
}

#[tokio::test]
async fn graceful_shutdown_finishes_inflight_request() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = test_router(
        SLOW_TABLE,
        RouterInput {
            timeout: Duration::from_secs(10),
            ..Default::default()
        },
    );
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .unwrap();
    });

    // Wait for the listener, then open one request that sleeps.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let client = tokio::spawn(async move {
        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        tokio::io::AsyncWriteExt::write_all(
            &mut stream,
            b"GET /slow HTTP/1.1\r\nHost: hub\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();
        let mut buf = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut buf)
            .await
            .unwrap();
        String::from_utf8(buf).unwrap()
    });

    // The request is in flight when shutdown arrives.
    tokio::time::sleep(Duration::from_millis(30)).await;
    shutdown_tx.send(()).unwrap();

    let response = client.await.unwrap();
    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "the in-flight request must finish: {response:?}"
    );
    server.await.unwrap();
}
