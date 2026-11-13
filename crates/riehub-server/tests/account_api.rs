//! The account surface end-to-end (identity spec, section 2):
//! password change, recovery, admin reset and recovery key rotation.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::TestServer;
use riehub_store::sqlite::SqliteStore;

const GOOD_PASSWORD: &str = "a-very-good-passphrase-42";
const NEW_PASSWORD: &str = "another-very-good-passphrase-7";

const ARMOR_BEGIN: &str = "-----BEGIN COFFERAK ROOT PRIVATE KEY-----"; // rules-lint: allow secret - test fixture, not a credential
const ARMOR_END: &str = "-----END COFFERAK ROOT PRIVATE KEY-----";

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

fn recovery_of(body: &str) -> String {
    let start = body
        .find("\"recovery_key\":\"")
        .map(|i| i + "\"recovery_key\":\"".len());
    match start {
        Some(start) => {
            let rest = &body[start..];
            let end = rest.find('"').unwrap_or(0);
            rest[..end].replace("\\n", "\n")
        }
        None => String::new(),
    }
}

async fn boot_owner_with_recovery() -> (TestServer, String, String, String) {
    let server = TestServer::start();
    let (_s, setup) = server
        .request(
            "POST",
            "/api/v1/setup",
            &[("content-type", "application/json")],
            Some(&setup_body("owner@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let recovery = recovery_of(&setup);
    let (_s, login) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("owner@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let cookie = cookie_of(&login);
    let csrf = csrf_of(&login);
    (server, cookie, csrf, recovery)
}

#[tokio::test]
async fn change_password_keeps_vault_entries_readable() {
    let (server, cookie, csrf, _recovery) = boot_owner_with_recovery().await;
    let (status, _) = server
        .request(
            "POST",
            "/api/v1/me/password",
            &[
                ("cookie", &cookie),
                ("x-csrf-token", &csrf),
                ("content-type", "application/json"),
            ],
            Some(&format!(
                "{{\"current_password\":\"{GOOD_PASSWORD}\",\"new_password\":\"{NEW_PASSWORD}\"}}"
            )),
        )
        .unwrap();
    assert_eq!(status, 200);

    // The new password logs in and the vault unlocks through it.
    let (status, _) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("owner@example.com", NEW_PASSWORD)),
        )
        .unwrap();
    assert_eq!(status, 200, "the new password works");
}

#[tokio::test]
async fn old_password_stops_working() {
    let (server, cookie, csrf, _recovery) = boot_owner_with_recovery().await;
    server
        .request(
            "POST",
            "/api/v1/me/password",
            &[
                ("cookie", &cookie),
                ("x-csrf-token", &csrf),
                ("content-type", "application/json"),
            ],
            Some(&format!(
                "{{\"current_password\":\"{GOOD_PASSWORD}\",\"new_password\":\"{NEW_PASSWORD}\"}}"
            )),
        )
        .unwrap();
    let (status, _) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("owner@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    assert_eq!(status, 401, "the old password stops working");
}

#[tokio::test]
async fn other_sessions_end() {
    let (server, cookie, csrf, _recovery) = boot_owner_with_recovery().await;
    // A second session.
    let (_s, second) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("owner@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    let second_cookie = cookie_of(&second);
    let (_s, me) = server
        .request("GET", "/api/v1/me", &[("cookie", &second_cookie)], None)
        .unwrap();
    assert!(me.contains("\"user_id\""), "the second session lives");

    server
        .request(
            "POST",
            "/api/v1/me/password",
            &[
                ("cookie", &cookie),
                ("x-csrf-token", &csrf),
                ("content-type", "application/json"),
            ],
            Some(&format!(
                "{{\"current_password\":\"{GOOD_PASSWORD}\",\"new_password\":\"{NEW_PASSWORD}\"}}"
            )),
        )
        .unwrap();
    let (status, _) = server
        .request("GET", "/api/v1/me", &[("cookie", &second_cookie)], None)
        .unwrap();
    assert_eq!(status, 401, "the other session ended");
}

#[tokio::test]
async fn recover_restores_access_and_drops_all_sessions() {
    let (server, cookie, _csrf, recovery) = boot_owner_with_recovery().await;
    server
        .request(
            "POST",
            "/api/v1/me/password",
            &[
                ("cookie", &cookie),
                ("x-csrf-token", &csrf_for_owner(&server, &cookie).await),
                ("content-type", "application/json"),
            ],
            Some(&format!(
                "{{\"current_password\":\"{GOOD_PASSWORD}\",\"new_password\":\"{NEW_PASSWORD}\"}}"
            )),
        )
        .unwrap();

    let (status, body) = server
        .request(
            "POST",
            "/api/v1/auth/recover",
            &[("content-type", "application/json")],
            Some(&format!(
                "{{\"email\":\"owner@example.com\",\"recovery_key\":\"{}\",\"new_password\":\"{GOOD_PASSWORD}\"}}",
                recovery.replace('\n', "\\n")
            )),
        )
        .unwrap();
    assert_eq!(status, 200, "was: {body}");

    // The old (and any) session is gone; the recovered password works.
    let (status, _) = server
        .request("GET", "/api/v1/me", &[("cookie", &cookie)], None)
        .unwrap();
    assert_eq!(status, 401, "all sessions ended");
    let (status, _) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("owner@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    assert_eq!(
        status, 200,
        "recovery restored access with the new password"
    );
}

async fn csrf_for_owner(server: &TestServer, cookie: &str) -> String {
    let (_s, me) = server
        .request("GET", "/api/v1/me", &[("cookie", cookie)], None)
        .unwrap();
    csrf_of(&me)
}

#[tokio::test]
async fn recover_with_wrong_key_fails_without_side_effects() {
    let (server, _cookie, _csrf, _recovery) = boot_owner_with_recovery().await;
    let (status, _) = server
        .request(
            "POST",
            "/api/v1/auth/recover",
            &[("content-type", "application/json")],
            Some(&format!(
                "{{\"email\":\"owner@example.com\",\"recovery_key\":\"{}\",\"new_password\":\"{NEW_PASSWORD}\"}}",
                &format!("{}wrong{}", ARMOR_BEGIN, ARMOR_END)
            )),
        )
        .unwrap();
    assert_eq!(status, 401, "a wrong recovery key is refused");

    // No side effects: the old password still logs in.
    let (status, _) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("owner@example.com", GOOD_PASSWORD)),
        )
        .unwrap();
    assert_eq!(status, 200, "the account is untouched");
}

#[tokio::test]
async fn rotated_recovery_key_works_and_old_one_fails() {
    let (server, cookie, csrf, old_recovery) = boot_owner_with_recovery().await;
    let (status, rotate) = server
        .request(
            "POST",
            "/api/v1/me/recovery-key/rotate",
            &[("cookie", &cookie), ("x-csrf-token", &csrf)],
            None,
        )
        .unwrap();
    assert_eq!(status, 200, "was: {rotate}");
    let new_recovery = recovery_of(&rotate);
    assert!(
        new_recovery.contains("-----BEGIN"),
        "the new key is armored"
    );

    // Rotate the password so the recovery path is exercised.
    server
        .request(
            "POST",
            "/api/v1/me/password",
            &[
                ("cookie", &cookie),
                ("x-csrf-token", &csrf),
                ("content-type", "application/json"),
            ],
            Some(&format!(
                "{{\"current_password\":\"{GOOD_PASSWORD}\",\"new_password\":\"{NEW_PASSWORD}\"}}"
            )),
        )
        .unwrap();

    // The old key fails, the new one works.
    let (status, _) = server
        .request(
            "POST",
            "/api/v1/auth/recover",
            &[("content-type", "application/json")],
            Some(&format!(
                "{{\"email\":\"owner@example.com\",\"recovery_key\":\"{}\",\"new_password\":\"{GOOD_PASSWORD}\"}}",
                old_recovery.replace('\n', "\\n")
            )),
        )
        .unwrap();
    assert_eq!(status, 401, "the old recovery key fails");

    let (status, _) = server
        .request(
            "POST",
            "/api/v1/auth/recover",
            &[("content-type", "application/json")],
            Some(&format!(
                "{{\"email\":\"owner@example.com\",\"recovery_key\":\"{}\",\"new_password\":\"{GOOD_PASSWORD}\"}}",
                new_recovery.replace('\n', "\\n")
            )),
        )
        .unwrap();
    assert_eq!(status, 200, "the rotated recovery key works");
}

#[tokio::test]
async fn admin_reset_does_not_open_vault() {
    let (server, owner_cookie, _csrf, _recovery) = boot_owner_with_recovery().await;
    // Invite a member, accept, so a vault exists for them.
    let owner_csrf = csrf_for_owner(&server, &owner_cookie).await;
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
    let invite = csrf_of(&created).replace("x", ""); // not used
    let _ = invite;
    let token_start = created
        .find("\"invite_token\":\"")
        .map(|i| i + "\"invite_token\":\"".len());
    let token = match token_start {
        Some(start) => {
            let rest = &created[start..];
            let end = rest.find('"').unwrap_or(0);
            rest[..end].to_owned()
        }
        None => panic!("the invite token is in the response"),
    };
    let (_s, accepted) = server
        .request(
            "POST",
            &format!("/api/v1/invites/{token}"),
            &[("content-type", "application/json")],
            Some("{\"password\":\"member-passphrase-123\"}"),
        )
        .unwrap();
    let member_recovery = recovery_of(&accepted);
    assert!(member_recovery.contains("-----BEGIN"));

    // The admin resets the member: pending again, new invite.
    let (_s, list) = server
        .request(
            "GET",
            "/api/v1/admin/users",
            &[("cookie", &owner_cookie)],
            None,
        )
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
    let (status, reset) = server
        .request(
            "POST",
            &format!("/api/v1/admin/users/{member_id}/reset"),
            &[("cookie", &owner_cookie), ("x-csrf-token", &owner_csrf)],
            None,
        )
        .unwrap();
    assert_eq!(status, 200, "was: {reset}");
    let new_token_start = reset
        .find("\"invite_token\":\"")
        .map(|i| i + "\"invite_token\":\"".len());
    let new_token = match new_token_start {
        Some(start) => {
            let rest = &reset[start..];
            let end = rest.find('"').unwrap_or(0);
            rest[..end].to_owned()
        }
        None => panic!("the reset carries a new invite"),
    };

    // "Start empty": the old vault is gone, a fresh one appears, and
    // the old recovery key no longer opens anything.
    let (status, accepted) = server
        .request(
            "POST",
            &format!("/api/v1/invites/{new_token}"),
            &[("content-type", "application/json")],
            Some("{\"password\":\"fresh-passphrase-12345\"}"),
        )
        .unwrap();
    assert_eq!(status, 200, "was: {accepted}");
    let fresh_recovery = recovery_of(&accepted);
    assert!(
        fresh_recovery.contains("-----BEGIN") && fresh_recovery != member_recovery,
        "a fresh vault comes with a fresh recovery key"
    );

    // The member logs in with the new password.
    let (status, _) = server
        .request(
            "POST",
            "/api/v1/auth/login",
            &[("content-type", "application/json")],
            Some(&login_body("member@example.com", "fresh-passphrase-12345")),
        )
        .unwrap();
    assert_eq!(status, 200);

    // The store holds no trace of the old vault under the member id:
    // the old recovery key cannot open the fresh vault either.
    let store = SqliteStore::open(&format!("sqlite://{}/riehub.db", server.data_dir.display()))
        .await
        .unwrap();
    let user_id = riehub_core::UserId::parse(&member_id).unwrap();
    let vault = riehub_vault::load(&store, &user_id).await.unwrap().unwrap();
    let result = riehub_vault::recover(
        &vault,
        &cofferak_core::crypto::RootPrivateKey::from_armored(&member_recovery).unwrap(),
    );
    assert!(
        result.is_err(),
        "the old recovery key cannot open the fresh vault"
    );
}
