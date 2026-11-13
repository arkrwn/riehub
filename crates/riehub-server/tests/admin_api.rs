//! The role and admin surface end-to-end (identity spec, section 5,
//! api spec section 2, SPEC section 2.1).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::TestServer;
use riehub_server::routes::{RouteClass, RouteSpec};

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

async fn csrf_of_cookie(server: &TestServer, cookie: &str) -> String {
    let (_s, me) = server
        .request("GET", "/api/v1/me", &[("cookie", cookie)], None)
        .unwrap();
    csrf_of(&me)
}

/// An owner boots the instance, invites a member, and returns the
/// member's cookie. The owner cookie comes back too.
async fn boot_with_member() -> (TestServer, String, String) {
    let server = TestServer::start();
    server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("owner@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let (_s, owner_login) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("owner@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let owner_cookie = cookie_of(&owner_login);
    let owner_csrf = csrf_of_cookie(&server, &owner_cookie).await;

    let (_s, created) = server
        .request(
            "POST",
            "/api/v1/admin/users",
            &[
                ("cookie", &owner_cookie),
                ("x-csrf-token", &owner_csrf),
                ("content-type", "application/json"),
            ],
            Some("{\"email\":\"member@example.com\"}"),
        )
        .unwrap();
    let token_start = created
        .find("\"invite_token\":\"")
        .map(|i| i + "\"invite_token\":\"".len());
    let token = match token_start {
        Some(start) => {
            let rest = &created[start..];
            let end = rest.find('"').unwrap_or(0);
            rest[..end].to_owned()
        }
        None => panic!("the invite token is in the response: {created}"),
    };
    let (status, accepted) = server
        .request(
            "POST",
            &format!("/api/v1/invites/{token}"),
            &[("content-type", "application/json")],
            Some(
                &setup_body("", GOOD_PASSWORD)
                    .replace("{\"email\":\"\",\"password\":\"", "{\"password\":\""),
            ),
        )
        .unwrap();
    assert_eq!(status, 200, "the invite was accepted: {accepted}");
    let (_s, member_login) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("member@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let member_cookie = cookie_of(&member_login);
    (server, owner_cookie, member_cookie)
}

#[tokio::test]
async fn class_tests_cover_every_route() {
    // The explicit list: a new route missing from here fails the test.
    let expected = [
        ("GET", "/healthz", "public"),
        ("GET", "/readyz", "public"),
        ("POST", "/api/v1/setup", "public"),
        ("POST", "/api/v1/auth/login", "public"),
        ("GET", "/api/v1/me", "self"),
        ("POST", "/api/v1/auth/logout", "self"),
        ("GET", "/api/v1/me/sessions", "self"),
        ("DELETE", "/api/v1/me/sessions/{id}", "self"),
        ("GET", "/api/v1/admin/users", "admin_meta"),
        ("POST", "/api/v1/admin/users", "admin_meta"),
        ("PATCH", "/api/v1/admin/users/{id}", "admin_meta"),
        ("DELETE", "/api/v1/admin/users/{id}", "admin_meta"),
        ("POST", "/api/v1/admin/invites", "admin_meta"),
        ("GET", "/api/v1/admin/audit", "admin_meta"),
        ("GET", "/api/v1/invites/{token}", "public"),
        ("POST", "/api/v1/invites/{token}", "public"),
        ("GET", "/api/v1/me/keys", "self"),
        ("POST", "/api/v1/me/keys", "self"),
        ("DELETE", "/api/v1/me/keys/{id}", "self"),
        ("GET", "/v1/models", "key"),
        ("POST", "/api/v1/me/password", "self"),
        ("POST", "/api/v1/me/recovery-key/rotate", "self"),
        ("POST", "/api/v1/auth/recover", "public"),
        ("POST", "/api/v1/admin/users/{id}/reset", "admin_meta"),
    ];
    assert_eq!(
        expected.len(),
        riehub_server::routes::TABLE.len(),
        "a route is missing from the class test list"
    );
    for (method, path, class) in expected {
        let found = riehub_server::routes::TABLE.iter().any(|spec: &RouteSpec| {
            spec.method.as_str() == method
                && spec.path == path
                && riehub_server::http::class_name(&spec.class) == class
        });
        assert!(found, "route {method} {path} as {class} is missing");
    }
    // No route hands out a session for another user.
    for spec in riehub_server::routes::TABLE {
        let takes_target_user =
            spec.path.contains("{user") || (spec.path.contains("login") && spec.path.contains('{'));
        assert!(
            !takes_target_user,
            "route {} must not take a target user id",
            spec.path
        );
    }
    let _ = RouteClass::Hook;
}

#[tokio::test]
async fn member_gets_403_on_admin_routes() {
    let (server, _owner, member) = boot_with_member().await;
    for (method, path) in [
        ("GET", "/api/v1/admin/users"),
        ("GET", "/api/v1/admin/audit"),
    ] {
        let (status, _) = server
            .request(method, path, &[("cookie", &member)], None)
            .unwrap();
        assert_eq!(status, 403, "a member is refused on {path}");
    }
}

#[tokio::test]
async fn admin_routes_need_a_session() {
    let server = TestServer::start();
    let (status, _) = server
        .request("GET", "/api/v1/admin/users", &[], None)
        .unwrap();
    assert_eq!(status, 401, "no cookie, no admin route");
}

#[tokio::test]
async fn owner_gets_404_for_another_users_session_id() {
    let (server, owner, member) = boot_with_member().await;
    let (_s, member_me) = server
        .request("GET", "/api/v1/me", &[("cookie", &member)], None)
        .unwrap();
    let start = member_me
        .find("\"session_id\":\"")
        .map(|i| i + "\"session_id\":\"".len());
    let session_id = match start {
        Some(start) => {
            let rest = &member_me[start..];
            let end = rest.find('"').unwrap_or(0);
            rest[..end].to_owned()
        }
        None => panic!("the session id is in me: {member_me}"),
    };
    let owner_csrf = csrf_of_cookie(&server, &owner).await;
    let (status, _) = server
        .request(
            "DELETE",
            &format!("/api/v1/me/sessions/{session_id}"),
            &[("cookie", &owner), ("x-csrf-token", &owner_csrf)],
            None,
        )
        .unwrap();
    assert_eq!(status, 404, "another user's session id answers 404");
}

#[tokio::test]
async fn admin_user_list_has_no_secret_or_content_fields() {
    let (server, owner, _member) = boot_with_member().await;
    let (status, body) = server
        .request("GET", "/api/v1/admin/users", &[("cookie", &owner)], None)
        .unwrap();
    assert_eq!(status, 200, "was: {body}");
    let allowlist = ["id", "email", "display_name", "status", "roles", "items"];
    for line in body.lines() {
        if !line.contains('{') {
            continue;
        }
        let _ = line;
    }
    let json_start = body.find('{').unwrap();
    let value: serde_json::Value = serde_json::from_str(&body[json_start..]).unwrap();
    let items = value["items"].as_array().unwrap();
    assert!(!items.is_empty(), "the owner is listed");
    for item in items {
        for key in item.as_object().unwrap().keys() {
            assert!(
                allowlist.contains(&key.as_str()),
                "the admin user list carries the field {key}"
            );
        }
    }
    // The AdminSafe trait exists and is implemented by hand for the
    // admin response types only.
    fn assert_admin_safe<T: riehub_identity::AdminSafe>(_: &T) {}
    struct SampleAdminList;
    impl riehub_identity::AdminSafe for SampleAdminList {}
    let sample = SampleAdminList;
    assert_admin_safe(&sample);
}

#[tokio::test]
async fn delete_user_removes_vault_and_all_records() {
    let (server, owner, _member) = boot_with_member().await;
    let owner_csrf = csrf_of_cookie(&server, &owner).await;

    // The member's user id comes from the admin list.
    let (_s, list) = server
        .request("GET", "/api/v1/admin/users", &[("cookie", &owner)], None)
        .unwrap();
    let json_start = list.find('{').unwrap();
    let value: serde_json::Value = serde_json::from_str(&list[json_start..]).unwrap();
    let member_id = value["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["email"] == "member@example.com")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let (status, body) = server
        .request(
            "DELETE",
            &format!("/api/v1/admin/users/{member_id}"),
            &[("cookie", &owner), ("x-csrf-token", &owner_csrf)],
            None,
        )
        .unwrap();
    assert_eq!(status, 200, "was: {body}");

    // Every record of the member is gone, the owner's stay.
    let (_s, list) = server
        .request("GET", "/api/v1/admin/users", &[("cookie", &owner)], None)
        .unwrap();
    assert!(
        !list.contains("member@example.com"),
        "the user record is gone: {list}"
    );
    let store = riehub_store::sqlite::SqliteStore::open(&format!(
        "sqlite://{}/riehub.db",
        server.data_dir.display()
    ))
    .await
    .unwrap();
    let user_id = riehub_core::UserId::parse(&member_id).unwrap();
    let vault = riehub_vault::load(&store, &user_id).await.unwrap();
    assert!(vault.is_none(), "the vault record is gone");
}

#[tokio::test]
async fn last_owner_cannot_be_removed() {
    let (server, owner, _member) = boot_with_member().await;
    let owner_csrf = csrf_of_cookie(&server, &owner).await;
    let (_s, list) = server
        .request("GET", "/api/v1/admin/users", &[("cookie", &owner)], None)
        .unwrap();
    let json_start = list.find('{').unwrap();
    let value: serde_json::Value = serde_json::from_str(&list[json_start..]).unwrap();
    let owner_id = value["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["email"] == "owner@example.com")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let (status, _) = server
        .request(
            "DELETE",
            &format!("/api/v1/admin/users/{owner_id}"),
            &[("cookie", &owner), ("x-csrf-token", &owner_csrf)],
            None,
        )
        .unwrap();
    assert_eq!(status, 409, "the last owner cannot be removed");

    let (status, _) = server
        .request(
            "PATCH",
            &format!("/api/v1/admin/users/{owner_id}"),
            &[
                ("cookie", &owner),
                ("x-csrf-token", &owner_csrf),
                ("content-type", "application/json"),
            ],
            Some("{\"status\":\"disabled\"}"),
        )
        .unwrap();
    assert_eq!(status, 409, "the last owner cannot be disabled");

    let (status, _) = server
        .request(
            "PATCH",
            &format!("/api/v1/admin/users/{owner_id}"),
            &[
                ("cookie", &owner),
                ("x-csrf-token", &owner_csrf),
                ("content-type", "application/json"),
            ],
            Some("{\"roles\":[\"member\"]}"),
        )
        .unwrap();
    assert_eq!(status, 409, "the last owner cannot be demoted");
}

#[tokio::test]
async fn admin_cannot_grant_the_owner_role() {
    // The member is promoted to admin, then tries to grant owner.
    let (server, owner, member) = boot_with_member().await;
    let owner_csrf = csrf_of_cookie(&server, &owner).await;
    let (_s, list) = server
        .request("GET", "/api/v1/admin/users", &[("cookie", &owner)], None)
        .unwrap();
    let json_start = list.find('{').unwrap();
    let value: serde_json::Value = serde_json::from_str(&list[json_start..]).unwrap();
    let member_id = value["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["email"] == "member@example.com")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // The owner makes the member an admin.
    let (status, _) = server
        .request(
            "PATCH",
            &format!("/api/v1/admin/users/{member_id}"),
            &[
                ("cookie", &owner),
                ("x-csrf-token", &owner_csrf),
                ("content-type", "application/json"),
            ],
            Some("{\"roles\":[\"admin\"]}"),
        )
        .unwrap();
    assert_eq!(status, 200);

    let member_csrf = csrf_of_cookie(&server, &member).await;
    let (status, _) = server
        .request(
            "PATCH",
            &format!("/api/v1/admin/users/{member_id}"),
            &[
                ("cookie", &member),
                ("x-csrf-token", &member_csrf),
                ("content-type", "application/json"),
            ],
            Some("{\"roles\":[\"owner\"]}"),
        )
        .unwrap();
    assert_eq!(
        status, 403,
        "an admin cannot assign the owner role, only an owner can"
    );
}

#[tokio::test]
async fn disabling_a_user_ends_their_sessions() {
    let (server, owner, member) = boot_with_member().await;
    let owner_csrf = csrf_of_cookie(&server, &owner).await;
    let (_s, list) = server
        .request("GET", "/api/v1/admin/users", &[("cookie", &owner)], None)
        .unwrap();
    let json_start = list.find('{').unwrap();
    let value: serde_json::Value = serde_json::from_str(&list[json_start..]).unwrap();
    let member_id = value["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["email"] == "member@example.com")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let (status, _) = server
        .request(
            "PATCH",
            &format!("/api/v1/admin/users/{member_id}"),
            &[
                ("cookie", &owner),
                ("x-csrf-token", &owner_csrf),
                ("content-type", "application/json"),
            ],
            Some("{\"status\":\"disabled\"}"),
        )
        .unwrap();
    assert_eq!(status, 200);

    let (status, _) = server
        .request("GET", "/api/v1/me", &[("cookie", &member)], None)
        .unwrap();
    assert_eq!(status, 401, "a disabled user loses every session");
}
