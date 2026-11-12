//! The web session HTTP surface end-to-end (identity spec, section 3).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::TestServer;

const GOOD_PASSWORD: &str = "a-very-good-passphrase-42";

fn setup_body(email: &str, password: &str) -> String {
    format!("{{\"email\":\"{email}\",\"password\":\"{password}\"}}")
}

fn login_body(email: &str, password: &str) -> String {
    format!("{{\"email\":\"{email}\",\"password\":\"{password}\"}}")
}

fn cookie_of(headers: &str) -> String {
    for line in headers.lines() {
        if let Some(rest) = line.strip_prefix("set-cookie:") {
            return rest.trim().split(';').next().unwrap_or("").to_owned();
        }
    }
    String::new()
}

fn csrf_of(body: &str) -> String {
    let start = body
        .find("\"csrf_token\":\"")
        .map(|i| i + "\"csrf_token\":\"".len());
    match start {
        Some(start) => {
            let rest = &body[start..];
            let end = rest.find('"').unwrap_or(0);
            rest[..end].to_owned()
        }
        None => String::new(),
    }
}

#[tokio::test]
async fn login_sets_host_cookie_with_flags() {
    let server = TestServer::start();
    server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let (_status, body) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    // Dev over plain http: the cookie is named `rs` without the
    // `__Host-` prefix and without Secure, but HttpOnly and
    // SameSite=Strict stay.
    assert!(body.contains("set-cookie: rs="), "was: {body}");
    assert!(body.contains("HttpOnly"), "was: {body}");
    assert!(body.contains("SameSite=Strict"), "was: {body}");
    assert!(body.contains("Path=/"), "was: {body}");
    assert!(!body.to_lowercase().contains("secure"), "was: {body}");
}

#[tokio::test]
async fn session_reaches_me_and_unlocks_vault() {
    let server = TestServer::start();
    server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let (_status, login) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let cookie = cookie_of(&login);
    let csrf = csrf_of(&login);

    let (status, me) = server
        .request("GET", "/api/v1/me", &[("cookie", &cookie)], None)
        .unwrap();
    assert_eq!(status, 200, "was: {me}");
    assert!(me.contains("\"user_id\""), "was: {me}");
    assert!(me.contains(&csrf), "me hands back the same csrf token");

    // The vault is unlocked through the session: the user can read
    // its own session list, which touches the store as its owner.
    let (status, sessions) = server
        .request("GET", "/api/v1/me/sessions", &[("cookie", &cookie)], None)
        .unwrap();
    assert_eq!(status, 200, "was: {sessions}");
    assert!(sessions.contains("\"items\""), "was: {sessions}");
}

#[tokio::test]
async fn logout_removes_session_and_slot() {
    let server = TestServer::start();
    server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let (_s, login) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let cookie = cookie_of(&login);
    let csrf = csrf_of(&login);

    let (status, body) = server
        .request(
            "POST",
            "/api/v1/auth/logout",
            &[("cookie", &cookie), ("x-csrf-token", &csrf)],
            None,
        )
        .unwrap();
    assert_eq!(status, 200, "was: {body}");

    // The cookie no longer reaches anything.
    let (status, _) = server
        .request("GET", "/api/v1/me", &[("cookie", &cookie)], None)
        .unwrap();
    assert_eq!(status, 401, "the session is gone");
}

#[tokio::test]
async fn post_without_csrf_token_is_403() {
    let server = TestServer::start();
    server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let (_s, login) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let cookie = cookie_of(&login);

    // A cookie-carrying POST without the header is refused.
    let (status, _) = server
        .request("POST", "/api/v1/auth/logout", &[("cookie", &cookie)], None)
        .unwrap();
    assert_eq!(status, 403, "csrf is required");
    // So is a wrong token.
    let (status, _) = server
        .request(
            "POST",
            "/api/v1/auth/logout",
            &[("cookie", &cookie), ("x-csrf-token", "deadbeef")],
            None,
        )
        .unwrap();
    assert_eq!(status, 403, "a wrong csrf token is refused");
}

#[tokio::test]
async fn eleventh_session_ends_the_oldest() {
    let server = TestServer::start();
    server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let mut cookies = Vec::new();
    for _ in 0..11 {
        let (_s, login) = server
            .request(
                "POST",
                "/api/v1/auth/login",
                &[("content-type", "application/json")],
                Some(&login_body("u@example.com", GOOD_PASSWORD)),
            )
            .unwrap();
        cookies.push(cookie_of(&login));
    }
    // The first cookie is gone, the last still works.
    let (status, _) = server
        .request("GET", "/api/v1/me", &[("cookie", &cookies[0])], None)
        .unwrap();
    assert_eq!(status, 401, "the oldest session ended");
    let (status, _) = server
        .request(
            "GET",
            "/api/v1/me",
            &[("cookie", cookies.last().unwrap())],
            None,
        )
        .unwrap();
    assert_eq!(status, 200, "the newest session lives");
}

#[tokio::test]
async fn session_works_on_a_second_server_instance_sharing_the_store() {
    // Two servers on one data directory, started one after the other.
    let dir = std::env::temp_dir().join(format!(
        "riehub-two-nodes-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let first = TestServer::start_on(dir.clone());
    first
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let (_s, login) = first
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let cookie = cookie_of(&login);
    drop(first);

    let second = TestServer::start_on(dir);
    let (status, me) = second
        .request("GET", "/api/v1/me", &[("cookie", &cookie)], None)
        .unwrap();
    assert_eq!(status, 200, "any node serves the session: {me}");
}
