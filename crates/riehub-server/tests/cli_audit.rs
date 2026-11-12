//! The audit CLI against a real server flow: init creates the
//! instance coffer, the audit key lives in it, and verify walks the
//! chain with that key. The tamper writes the database directly,
//! which is exactly the attacker ADR 0010 defends against.

#![allow(clippy::unwrap_used)]

use riehub_audit::event::{Actor, AuditAction, AuditEvent, Event, Outcome};
use riehub_audit::{AuditChainMac, AuditLog};
use riehub_store::sqlite::SqliteStore;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "riehub-cli-audit-{}-{}",
        tag,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_config(dir: &std::path::Path) -> PathBuf {
    // A free loopback port, so a failing serve cannot collide with
    // another server on a fixed port.
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    let url = format!("sqlite://{}/riehub.db", dir.display());
    let key_file = dir.join("host.key");
    let path = dir.join("riehub.toml");
    std::fs::write(
        &path,
        format!(
            "profile = \"dev\"\n\n[server]\nbind = \"127.0.0.1:{}\"\n\n[data]\ndir = \"{}\"\n\n[database]\nurl = \"{}\"\n\n[instance]\nseal = \"file\"\nkey_file = \"{}\"\n",
            port,
            dir.display(),
            url,
            key_file.display()
        ),
    )
    .unwrap();
    path
}

fn run_init(config: &std::path::Path) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_riehub"))
        .arg("init")
        .arg("--config")
        .arg(config)
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
    )
}

fn run_cli(config: &std::path::Path, args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_riehub"))
        .arg("audit")
        .args(args)
        .arg("--config")
        .arg(config)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    (out.status.code().unwrap_or(-1), text)
}

/// Appends events through the real coffer key.
async fn append_events(dir: &std::path::Path, url: &str, count: u64) {
    use riehub_store::Store;
    let store = SqliteStore::open(url).await.unwrap();
    store.migrate().await.unwrap();
    let config = riehub_core::Config::load(Some(&dir.join("riehub.toml"))).unwrap();
    let (coffer, unlocked) = riehub_server::cli::open_instance_coffer(&config, &store)
        .await
        .unwrap();
    let bytes = riehub_vault::instance::get_secret(
        &coffer.file,
        &unlocked,
        riehub_vault::instance::AUDIT_KEY_PATH,
    )
    .unwrap();
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    let mac = AuditChainMac::new(riehub_crypto::Key32::new(key));
    let log = AuditLog::new(Arc::new(store), mac);
    let events: Vec<Event> = (0..count)
        .map(|i| Event {
            ts: 1_700_000_000_000 + i as i64,
            actor: Actor::System,
            action: AuditAction::SystemStart,
            outcome: Outcome::Ok,
            request_id: None,
            target: None,
            ip: None,
            event: AuditEvent::SystemStart {},
        })
        .collect();
    log.append(events).await.unwrap();
}

#[tokio::test]
async fn audit_chain_verifies_with_key_from_coffer() {
    let dir = temp_dir("verify");
    let config = write_config(&dir);
    let (code, out) = run_init(&config);
    assert_eq!(code, 0, "init must succeed: {out}");
    append_events(&dir, &format!("sqlite://{}/riehub.db", dir.display()), 10).await;

    let (code, text) = run_cli(&config, &["verify"]);
    assert_eq!(code, 0, "the chain verifies with the coffer key: {text}");
    assert!(
        text.contains("audit chain ok through seq 10"),
        "was: {text}"
    );
}

#[tokio::test]
async fn cli_verify_exits_3_on_tamper() {
    let dir = temp_dir("tamper");
    let config = write_config(&dir);
    run_init(&config);
    let url = format!("sqlite://{}/riehub.db", dir.display());
    append_events(&dir, &url, 10).await;

    // Tamper: rewrite one event body in place.
    let tamper =
        "UPDATE audit SET event = '{\"type\":\"system_start\",\"x\":1}' WHERE seq = 4".to_owned();
    let pool = sqlx::sqlite::SqlitePool::connect(&format!("sqlite://{}", url))
        .await
        .unwrap();
    sqlx::query(sqlx::AssertSqlSafe(tamper))
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    let (code, text) = run_cli(&config, &["verify"]);
    assert_eq!(code, 3, "a tampered chain must exit 3, got: {text}");
    assert!(text.contains("broken at seq 4"), "was: {text}");
}

#[tokio::test]
async fn cli_head_and_export_show_the_chain() {
    let dir = temp_dir("head");
    let config = write_config(&dir);
    run_init(&config);
    let url = format!("sqlite://{}/riehub.db", dir.display());
    append_events(&dir, &url, 10).await;

    let (code, text) = run_cli(&config, &["head"]);
    assert_eq!(code, 0);
    assert!(text.starts_with("seq 10 mac "), "was: {text}");

    let (code, text) = run_cli(&config, &["export"]);
    assert_eq!(code, 0);
    let lines = text.lines().count();
    assert_eq!(lines, 10, "export writes one json line per event");
    assert!(text.lines().next().unwrap().contains("\"seq\":1"));
}

#[tokio::test]
async fn init_creates_coffer_and_prints_recovery_key_once() {
    let dir = temp_dir("initkey");
    let config = write_config(&dir);
    let (code, out) = run_init(&config);
    assert_eq!(code, 0);
    assert!(
        out.contains("instance recovery key"),
        "the recovery key is printed once: {out}"
    );
    assert!(out.contains("-----BEGIN"), "the key is armored: {out}");

    // A second init prints nothing about the recovery key.
    let (code, out) = run_init(&config);
    assert_eq!(code, 0);
    assert!(
        !out.contains("instance recovery key"),
        "the key is never printed twice: {out}"
    );
}

#[tokio::test]
async fn serve_fails_closed_when_host_key_is_missing_or_wrong() {
    let dir = temp_dir("failclosed");
    let config = write_config(&dir);
    run_init(&config);

    // Corrupt the host key record in the store.
    let url = format!("sqlite://{}/riehub.db", dir.display());
    let pool = sqlx::sqlite::SqlitePool::connect(&format!("sqlite://{}", url))
        .await
        .unwrap();
    sqlx::query("UPDATE records SET data = X'00' WHERE id = 'instance.host-key'")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    let out = Command::new(env!("CARGO_BIN_EXE_riehub"))
        .arg("serve")
        .arg("--config")
        .arg(&config)
        .output()
        .unwrap();
    assert_ne!(
        out.status.code(),
        Some(0),
        "serve must fail closed without a working host seal"
    );
}
