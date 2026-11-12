//! The privacy canary (security spec, section 8): a fresh canary goes
//! through every surface that exists today, and nothing may ever see
//! it again on disk or in the logs.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::type_complexity)]

mod common;

use common::TestServer;

fn fresh_canary() -> String {
    format!(
        "RIEHUB-CANARY-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

#[tokio::test]
async fn canary_never_reaches_disk_or_logs() {
    let mut server = TestServer::start();
    let canary = fresh_canary();

    // The canary travels in the body, a header, the query string and
    // the path of requests to the routes that exist.
    let cases: Vec<(&str, &str, Vec<(&str, &str)>, Option<String>)> = vec![
        ("GET", "/healthz?canary={c}", vec![], None),
        ("GET", "/healthz", vec![("x-canary", "{c}")], None),
        ("GET", "/readyz?c={c}", vec![], None),
        ("POST", "/healthz", vec![], Some("{c}".to_owned())),
        ("GET", "/{c}", vec![], None),
        ("GET", "/readyz/{c}", vec![], None),
    ];
    for (method, target, headers, body) in cases {
        let target = target.replace("{c}", &canary);
        let headers: Vec<(String, String)> = headers
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.replace("{c}", &canary)))
            .collect();
        let header_refs: Vec<(&str, &str)> = headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let body = body.map(|b| b.replace("{c}", &canary));
        let body_ref = body.as_deref();
        let (status, _text) = server
            .request(method, &target, &header_refs, body_ref)
            .unwrap();
        // The canary never changes what the server answers.
        assert!(
            status == 200 || status == 404 || status == 405,
            "unexpected status {status} for {method} {target}"
        );
    }

    server.stop();
    let hits = server.scan_for(&canary);
    assert!(
        hits.is_empty(),
        "the canary leaked to disk or logs: {hits:?}"
    );
}

#[tokio::test]
async fn scanner_finds_a_planted_canary() {
    let server = TestServer::start();
    let canary = fresh_canary();

    // Plant the canary in a file under the data directory: the scanner
    // must be able to fail.
    std::fs::write(server.data_dir.join("planted.txt"), &canary).unwrap();

    let hits = server.scan_for(&canary);
    assert!(
        hits.iter()
            .any(|hit| hit.where_.contains("planted.txt") && hit.encoding == "raw"),
        "the scanner must report the planted canary: {hits:?}"
    );
}
