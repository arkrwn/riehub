//! `riehub audit verify` against a tampered database. The tamper
//! writes the database directly, which is exactly the attacker ADR
//! 0010 defends against: someone who can write the rows cannot forge
//! or re-chain events without the audit key.

#![allow(clippy::unwrap_used)]

use riehub_audit::event::{Actor, AuditAction, AuditEvent, Event, Outcome};
use riehub_audit::{AuditChainMac, AuditLog};
use riehub_store::sqlite::SqliteStore;
use riehub_store::Store;
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

async fn build_log(dir: &std::path::Path) -> (AuditLog<SqliteStore>, String) {
    let url = format!("sqlite://{}/riehub.db", dir.display());
    let store = SqliteStore::open(&url).await.unwrap();
    store.migrate().await.unwrap();
    let mac = AuditChainMac::new(riehub_crypto::Key32::new([5u8; 32]));
    let log = AuditLog::new(Arc::new(store), mac);
    let events: Vec<Event> = (0..10u64)
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
    (log, url)
}

fn write_config(dir: &std::path::Path, url: &str) -> PathBuf {
    // The CLI reads the audit key from the data directory; write the
    // same key the log appended with, mode 0600.
    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt;
        let key_path = dir.join("audit.key");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&key_path)
            .unwrap();
        file.write_all(&[5u8; 32]).unwrap();
    }
    let path = dir.join("riehub.toml");
    std::fs::write(
        &path,
        format!(
            "profile = \"dev\"\n\n[server]\nbind = \"127.0.0.1:8080\"\n\n[data]\ndir = \"{}\"\n\n[database]\nurl = \"{}\"\n",
            dir.display(),
            url
        ),
    )
    .unwrap();
    path
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

#[tokio::test]
async fn cli_verify_exits_3_on_tamper() {
    let dir = temp_dir("tamper");
    let (_log, url) = build_log(&dir).await;
    let config = write_config(&dir, &url);

    // The untampered chain verifies with exit 0.
    let (code, text) = run_cli(&config, &["verify"]);
    assert_eq!(code, 0, "a clean chain must exit 0, got: {text}");

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
    let (_log, url) = build_log(&dir).await;
    let config = write_config(&dir, &url);

    let (code, text) = run_cli(&config, &["head"]);
    assert_eq!(code, 0);
    assert!(text.starts_with("seq 10 mac "), "was: {text}");

    let (code, text) = run_cli(&config, &["export"]);
    assert_eq!(code, 0);
    let lines = text.lines().count();
    assert_eq!(lines, 10, "export writes one json line per event");
    assert!(text.lines().next().unwrap().contains("\"seq\":1"));
}
