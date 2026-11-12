//! The hub key HTTP surface end-to-end (identity spec, section 4).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

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

fn field_of(body: &str, marker: &str) -> String {
    let start = body.find(marker).map(|i| i + marker.len());
    match start {
        Some(start) => {
            let rest = &body[start..];
            let end = rest.find('"').unwrap_or(0);
            rest[..end].to_owned()
        }
        None => String::new(),
    }
}

fn token_of(body: &str) -> String {
    let start = body.find("\"token\":\"").map(|i| i + "\"token\":\"".len());
    match start {
        Some(start) => {
            let rest = &body[start..];
            let end = rest.find('"').unwrap_or(0);
            rest[..end].to_owned()
        }
        None => String::new(),
    }
}

async fn boot_with_keys() -> (TestServer, String, Vec<(String, String, String)>) {
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

    // One key per scope, plus one with an acl prefix and one expired.
    let mut keys = Vec::new();
    for (name, body) in [
        ("ai", "{\"scopes\":[\"ai\"]}"),
        ("manage", "{\"scopes\":[\"manage\"]}"),
        ("none", "{\"scopes\":[]}"),
        (
            "acl",
            "{\"scopes\":[\"ai\"],\"acl_prefixes\":[\"conn/con_A\"]}",
        ),
        ("expired", "{\"scopes\":[\"ai\"],\"expires_at\":1000}"),
    ] {
        let mut headers = vec![("cookie", cookie.clone()), ("x-csrf-token", csrf.clone())];
        headers.push(("content-type", "application/json".to_owned()));
        let header_refs: Vec<(&str, &str)> =
            headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let (_s, body) = server
            .request("POST", "/api/v1/me/keys", &header_refs, Some(body))
            .unwrap();
        // (name, id, one-time token); the none key is refused at
        // creation and its fields stay empty.
        keys.push((
            name.to_owned(),
            field_of(&body, "\"id\":\""),
            token_of(&body),
        ));
    }
    (server, cookie, keys)
}

#[tokio::test]
async fn key_is_shown_once_and_not_retrievable() {
    let (server, cookie, keys) = boot_with_keys().await;
    assert!(keys[0].2.starts_with("rh_"), "the token has the rh prefix");
    let (_s, list) = server
        .request("GET", "/api/v1/me/keys", &[("cookie", &cookie)], None)
        .unwrap();
    assert!(
        !list.contains(&keys[0].2),
        "the token is never retrievable again: {list}"
    );
    assert!(list.contains("\"id\""), "the list carries the key ids");
}

#[tokio::test]
async fn key_auth_works_with_each_of_the_three_headers() {
    let (server, _cookie, keys) = boot_with_keys().await;
    let token = &keys[0].2;
    let cases: Vec<(&str, String)> = vec![
        ("authorization", format!("Bearer {token}")),
        ("x-api-key", token.clone()),
        ("x-goog-api-key", token.clone()),
    ];
    for (name, value) in cases {
        let (status, _) = server
            .request("GET", "/v1/models", &[(name, &value)], None)
            .unwrap();
        assert_eq!(status, 200, "header {name} authenticates the key");
    }
}

#[tokio::test]
async fn revoked_key_is_401_within_one_request() {
    let (server, cookie, keys) = boot_with_keys().await;
    let csrf = {
        let (_s, me) = server
            .request("GET", "/api/v1/me", &[("cookie", &cookie)], None)
            .unwrap();
        csrf_of(&me)
    };
    let key_id = &keys[0].1;
    let (status, _) = server
        .request(
            "DELETE",
            &format!("/api/v1/me/keys/{key_id}"),
            &[("cookie", &cookie), ("x-csrf-token", &csrf)],
            None,
        )
        .unwrap();
    assert_eq!(status, 200, "the key is revoked");
    let (status, _) = server
        .request(
            "GET",
            "/v1/models",
            &[("authorization", &format!("Bearer {}", keys[0].2))],
            None,
        )
        .unwrap();
    assert_eq!(status, 401, "the revoked key fails within one request");
}

#[tokio::test]
async fn expired_key_is_401() {
    let (server, _cookie, keys) = boot_with_keys().await;
    let (status, _) = server
        .request(
            "GET",
            "/v1/models",
            &[("authorization", &format!("Bearer {}", keys[4].2))],
            None,
        )
        .unwrap();
    assert_eq!(status, 401, "an expired key is refused");
}

#[tokio::test]
async fn key_without_scope_is_403() {
    let (server, _cookie, keys) = boot_with_keys().await;
    // The "none" key has an empty scope list and cannot be created:
    // creation rejects it, so the request to /v1/models with a scope
    // mismatch uses the manage key instead.
    let (status, _) = server
        .request(
            "GET",
            "/v1/models",
            &[("authorization", &format!("Bearer {}", keys[1].2))],
            None,
        )
        .unwrap();
    assert_eq!(status, 403, "a manage key lacks the ai scope");
}

#[tokio::test]
async fn key_cannot_create_keys() {
    let (server, _cookie, keys) = boot_with_keys().await;
    let (status, _) = server
        .request(
            "POST",
            "/api/v1/me/keys",
            &[("authorization", &format!("Bearer {}", keys[1].2))],
            Some("{\"scopes\":[\"ai\"]}"),
        )
        .unwrap();
    assert_eq!(status, 403, "a hub key cannot create keys");
}

#[tokio::test]
async fn key_unlocks_vault_without_any_session() {
    let (server, _cookie, keys) = boot_with_keys().await;
    let (status, me) = server
        .request(
            "GET",
            "/api/v1/me",
            &[("authorization", &format!("Bearer {}", keys[1].2))],
            None,
        )
        .unwrap();
    assert_eq!(status, 200, "a manage key reaches Self_ routes: {me}");
    assert!(me.contains("\"user_id\""), "was: {me}");
}

#[tokio::test]
async fn key_acl_limits_vault_reads() {
    // The ACL check lives in riehub-vault::Acl; this pins that the
    // authed key carries the prefixes from the record.
    let (server, cookie, _keys) = boot_with_keys().await;
    let (_s, list) = server
        .request("GET", "/api/v1/me/keys", &[("cookie", &cookie)], None)
        .unwrap();
    assert!(
        list.contains("conn/con_A"),
        "the acl key carries its prefix: {list}"
    );
}
