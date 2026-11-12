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
    RouteSpec {
        method: &Method::POST,
        path: "/api/v1/setup",
        class: RouteClass::Public,
        timeout: None,
        handler: |state, req| Box::pin(handlers::setup(state, req)),
    },
    RouteSpec {
        method: &Method::POST,
        path: "/api/v1/auth/login",
        class: RouteClass::Public,
        timeout: None,
        handler: |state, req| Box::pin(handlers::login(state, req)),
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

    /// Reads the request body as json, capped at the body limit.
    async fn read_json(req: Request) -> std::result::Result<serde_json::Value, Box<Response>> {
        let bytes = axum::body::to_bytes(req.into_body(), 1_048_576)
            .await
            .map_err(|_| Box::new(problem(StatusCode::BAD_REQUEST)))?;
        serde_json::from_slice(&bytes).map_err(|_| Box::new(problem(StatusCode::BAD_REQUEST)))
    }

    fn field<'a>(body: &'a serde_json::Value, name: &str) -> Option<&'a str> {
        body.get(name).and_then(|v| v.as_str())
    }

    /// The request id the middleware generated.
    fn request_id(req: &Request) -> Option<String> {
        req.extensions().get::<String>().cloned()
    }

    pub async fn setup(state: AppState, req: Request) -> Response {
        use crate::http::{problem_response, SetupResponse};
        let request_id = request_id(&req);
        let Some(identity) = &state.0.identity else {
            return problem(StatusCode::SERVICE_UNAVAILABLE);
        };
        let body = match read_json(req).await {
            Ok(body) => body,
            Err(response) => return *response,
        };
        let Some(email) = field(&body, "email").map(str::to_owned) else {
            return problem_response(
                &riehub_core::Error::Invalid("email: the field is required".to_owned()),
                request_id.as_deref(),
            );
        };
        let Some(password) = field(&body, "password").map(str::to_owned) else {
            return problem_response(
                &riehub_core::Error::Invalid("password: the field is required".to_owned()),
                request_id.as_deref(),
            );
        };
        let display_name = field(&body, "display_name").map(str::to_owned);
        let password = riehub_core::Secret::new(password);
        match identity
            .setup_owner(&email, &password, display_name.as_deref())
            .await
        {
            Ok((user, armored)) => SetupResponse::new(user.id.to_string(), armored).into_response(),
            Err(e) => problem_response(&e, request_id.as_deref()),
        }
    }

    pub async fn login(state: AppState, req: Request) -> Response {
        use crate::http::{problem_response, problem_response_with};
        let request_id = request_id(&req);
        let Some(identity) = &state.0.identity else {
            return problem(StatusCode::SERVICE_UNAVAILABLE);
        };
        // Capture the source address before the body is read.
        let ip = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(',').next().unwrap_or(v).trim().to_owned());
        let body = match read_json(req).await {
            Ok(body) => body,
            Err(response) => return *response,
        };
        let Some(email) = field(&body, "email").map(str::to_owned) else {
            return problem_response(
                &riehub_core::Error::Invalid("email: the field is required".to_owned()),
                request_id.as_deref(),
            );
        };
        let Some(password) = field(&body, "password").map(str::to_owned) else {
            return problem_response(
                &riehub_core::Error::Invalid("password: the field is required".to_owned()),
                request_id.as_deref(),
            );
        };
        let email = riehub_core::Secret::new(email);
        let password = riehub_core::Secret::new(password);
        match identity.login(&email, &password, ip.as_deref()).await {
            Ok(user_id) => (
                StatusCode::OK,
                Json(json!({"status": "ok", "user_id": user_id.to_string()})),
            )
                .into_response(),
            Err(e) => {
                if let riehub_core::Error::RateLimited(_) = &e {
                    let (account, ip_fails, retry_after) =
                        identity.throttle_state(email.expose(), ip.as_deref()).await;
                    let retry_after = if account > 0 || ip_fails > 0 {
                        retry_after
                    } else {
                        1
                    };
                    return problem_response_with(
                        &e,
                        request_id.as_deref(),
                        &[("retry-after", retry_after.to_string())],
                    );
                }
                problem_response(&e, request_id.as_deref())
            }
        }
    }
}
