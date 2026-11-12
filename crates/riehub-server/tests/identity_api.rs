//! The identity HTTP surface end-to-end (identity spec, sections 1
//! and 2): setup, login, throttling, over the real binary.

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

#[tokio::test]
async fn setup_creates_owner_once() {
    let server = TestServer::start();
    let (status, body) = server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("owner@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    assert_eq!(status, 200, "was: {body}");
    assert!(
        body.contains("\"recovery_key\""),
        "the recovery key is in the body once"
    );
    assert!(body.contains("-----BEGIN"), "the key is armored");

    let (status, body) = server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("second@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    assert_eq!(status, 409, "the second setup conflicts: {body}");
}

#[tokio::test]
async fn login_ok_with_correct_password() {
    let server = TestServer::start();
    server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let (status, body) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    assert_eq!(status, 200, "was: {body}");
    assert!(body.contains("\"status\":\"ok\""), "was: {body}");
}

#[tokio::test]
async fn login_unknown_email_and_wrong_password_give_same_status_and_body() {
    let server = TestServer::start();
    server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("u@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let (s1, b1) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("ghost@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let (s2, b2) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("u@example.com", "totally-different-pass-1")),
        )
        .unwrap();
    assert_eq!(s1, s2, "the statuses match");
    assert_eq!(strip_volatile(&b1), strip_volatile(&b2), "the bodies match");
}

fn strip_volatile(body: &str) -> String {
    // Compare the status line and the json body only: header order,
    // dates and request ids differ between calls.
    let status = body.lines().next().unwrap_or("").to_owned();
    let json_start = body.find('{').unwrap_or(0);
    let json = &body[json_start..];
    let request_id_start = json
        .find("\"request_id\":\"")
        .map(|i| i + "\"request_id\":\"".len());
    match request_id_start {
        Some(start) => {
            let rest = &json[start..];
            let end = rest.find('"').unwrap_or(0);
            let masked = json.replacen(&rest[..end], "<id>", 1);
            format!("{status}\n{masked}")
        }
        None => format!("{status}\n{json}"),
    }
}

#[tokio::test]
async fn login_is_throttled_after_5_failures() {
    let server = TestServer::start();
    server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("t@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    for _ in 0..5 {
        let _ = server
            .request(
                "POST",
                "/api/v1/auth/login",
                &[("content-type", "application/json")],
                Some(&login_body("t@example.com", "totally-different-pass-1")),
            )
            .unwrap();
    }
    let (status, body) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("t@example.com", "totally-different-pass-1")),
        )
        .unwrap();
    assert_eq!(status, 429, "was: {body}");
    assert!(body.to_lowercase().contains("retry-after"), "was: {body}");
}

#[tokio::test]
async fn common_password_is_rejected() {
    let server = TestServer::start();
    let (status, body) = server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("c@example.com", "password1234")),
        )
        .unwrap();
    assert_eq!(status, 400, "a common password is rejected: {body}");
}

#[tokio::test]
async fn short_password_is_rejected() {
    let server = TestServer::start();
    let (status, body) = server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("s@example.com", "short-pass")),
        )
        .unwrap();
    assert_eq!(status, 400, "a short password is rejected: {body}");
}
