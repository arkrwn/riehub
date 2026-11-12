//! The static route table (api spec, section 2). The HTTP application
//! is built from this table and from nothing else. Adding a route
//! without a class does not compile.

use crate::http::AppState;
use axum::extract::Request;
use axum::http::Method;
use axum::response::Response;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

/// Who may call a route (api spec, section 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteClass {
    /// No credential: health endpoints.
    Public,
    /// The caller's own resources.
    Self_,
    /// Instance metadata, needs a permission.
    AdminMeta,
    /// Hub key surfaces. The scope is checked by the router.
    Key(Scope),
    /// Signed webhooks.
    Hook,
}

/// The scopes a hub key can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Ai,
    Manage,
    Mcp,
}

/// One handler: takes the shared state and the request.
pub type Handler = fn(AppState, Request) -> Pin<Box<dyn Future<Output = Response> + Send>>;

/// One row of the table.
#[derive(Debug, Clone, Copy)]
pub struct RouteSpec {
    pub method: &'static Method,
    pub path: &'static str,
    pub class: RouteClass,
    /// The per-route timeout. None uses the default.
    pub timeout: Option<Duration>,
    pub handler: Handler,
}

/// The production route table. Later tasks append to it.
pub static TABLE: &[RouteSpec] = &[
    RouteSpec {
        method: &Method::GET,
        path: "/healthz",
        class: RouteClass::Public,
        timeout: None,
        handler: |state, req| Box::pin(handlers::healthz(state, req)),
    },
    RouteSpec {
        method: &Method::GET,
        path: "/readyz",
        class: RouteClass::Public,
        timeout: None,
        handler: |state, req| Box::pin(handlers::readyz(state, req)),
    },
];

pub mod handlers {
    use super::AppState;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::response::{IntoResponse, Json, Response};
    use riehub_store::Store;
    use serde_json::json;

    pub async fn healthz(_state: AppState, _req: Request) -> Response {
        (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
    }

    pub async fn readyz(state: AppState, _req: Request) -> Response {
        match &state.0.store {
            Some(store) => match store.ping().await {
                Ok(()) => (StatusCode::OK, Json(json!({"status": "ready"}))).into_response(),
                Err(_) => problem(StatusCode::SERVICE_UNAVAILABLE),
            },
            None => problem(StatusCode::SERVICE_UNAVAILABLE),
        }
    }

    fn problem(status: StatusCode) -> Response {
        use crate::http::problem_response;
        let error = if status == StatusCode::SERVICE_UNAVAILABLE {
            riehub_core::Error::Unavailable("the store is not ready".to_owned())
        } else {
            riehub_core::Error::Internal(None)
        };
        problem_response(&error, None)
    }
}
