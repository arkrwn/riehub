//! The serve loop: bind per config, announce `SystemStart` to the
//! audit log, shut down gracefully on SIGTERM and SIGINT.

use crate::http::{build_router, RouterInput};
use crate::routes;
use riehub_audit::event::{Actor, AuditAction, AuditEvent, Event, Outcome};
use riehub_audit::AuditLog;
use riehub_core::clock::Clock;
use riehub_core::config::Config;
use riehub_core::error::Result;
use riehub_store::sqlite::SqliteStore;
use riehub_store::Store;
use std::sync::Arc;
use std::time::Duration;

/// Announces the process start to the audit log. The write happens
/// before the listener accepts any request; a failure refuses the
/// start (security spec, section 6).
pub async fn announce_start(audit: &AuditLog<SqliteStore>) -> Result<()> {
    let event = Event {
        ts: riehub_core::SystemClock.now_millis(),
        actor: Actor::System,
        action: AuditAction::SystemStart,
        outcome: Outcome::Ok,
        request_id: None,
        target: None,
        ip: None,
        event: AuditEvent::SystemStart {},
    };
    audit.append(vec![event]).await?;
    Ok(())
}

/// The pieces `riehub serve` assembles, exposed for tests.
#[derive(Debug)]
pub struct Server {
    pub config: Config,
    pub store: Arc<SqliteStore>,
    pub audit: Arc<AuditLog<SqliteStore>>,
}

pub async fn assemble(config: Config) -> Result<Server> {
    let store = Arc::new(riehub_store::open(&config.database.url).await?);
    store.migrate().await?;
    // The audit key comes from the instance coffer. If the host seal
    // cannot be opened, the server refuses to start (S6).
    let (coffer, unlocked) = crate::cli::open_instance_coffer(&config, &store).await?;
    let bytes = riehub_vault::instance::get_secret(
        &coffer.file,
        &unlocked,
        riehub_vault::instance::AUDIT_KEY_PATH,
    )?;
    if bytes.len() != 32 {
        return Err(riehub_core::Error::Invalid(
            "the audit key entry must hold 32 bytes".to_owned(),
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    let mac = riehub_audit::AuditChainMac::new(riehub_crypto::Key32::new(key));
    let audit = Arc::new(AuditLog::new(store.clone(), mac));
    Ok(Server {
        config,
        store,
        audit,
    })
}

pub fn init_tracing(level: &str) {
    let max = match level {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };
    let _ = tracing_subscriber::fmt()
        .json()
        .with_max_level(max)
        .with_writer(std::io::stdout)
        .try_init();
}

pub async fn run(server: Server) -> Result<()> {
    announce_start(&server.audit).await?;
    let router = build_router(
        RouterInput {
            store: Some(server.store.clone()),
            audit: Some(server.audit.clone()),
            timeout: Duration::from_secs(30),
            body_limit: crate::http::DEFAULT_BODY_LIMIT,
            header_limit: crate::http::DEFAULT_HEADER_LIMIT,
        },
        routes::TABLE,
    );
    let listener = tokio::net::TcpListener::bind(server.config.server.bind.as_str())
        .await
        .map_err(|_| {
            riehub_core::Error::Unavailable("the bind address is not usable".to_owned())
        })?;
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|_| riehub_core::Error::Internal(None))?;
    Ok(())
}

/// Resolves on SIGTERM or SIGINT (security spec, section 6).
pub async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(term) => term,
                Err(_) => {
                    let _ = ctrl_c.await;
                    return;
                }
            };
        tokio::select! {
            _ = ctrl_c => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
}
