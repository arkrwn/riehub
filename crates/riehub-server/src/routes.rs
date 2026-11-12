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
    RouteSpec {
        method: &Method::GET,
        path: "/api/v1/me",
        class: RouteClass::Self_,
        timeout: None,
        handler: |state, req| Box::pin(handlers::me(state, req)),
    },
    RouteSpec {
        method: &Method::POST,
        path: "/api/v1/auth/logout",
        class: RouteClass::Self_,
        timeout: None,
        handler: |state, req| Box::pin(handlers::logout(state, req)),
    },
    RouteSpec {
        method: &Method::GET,
        path: "/api/v1/me/sessions",
        class: RouteClass::Self_,
        timeout: None,
        handler: |state, req| Box::pin(handlers::my_sessions(state, req)),
    },
    RouteSpec {
        method: &Method::DELETE,
        path: "/api/v1/me/sessions/{id}",
        class: RouteClass::Self_,
        timeout: None,
        handler: |state, req| Box::pin(handlers::end_session(state, req)),
    },
];

pub mod handlers {
    use super::AppState;
    use crate::http::problem_response;
    use axum::extract::Request;
    use axum::http::header;
    use axum::http::HeaderValue;
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
        if status == StatusCode::SERVICE_UNAVAILABLE {
            return problem_response(
                &riehub_core::Error::Unavailable("the store is not ready".to_owned()),
                None,
            );
        }
        // A generic problem body for the requested status.
        let title = status.canonical_reason().unwrap_or("error");
        let body = serde_json::json!({
            "type": format!("https://riehub.dev/problems/{}", title.replace(' ', "-")),
            "title": title,
            "status": status.as_u16(),
            "detail": title,
            "request_id": "",
        });
        let mut response = (status, Json(body)).into_response();
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/problem+json"),
        );
        response
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

    /// The Set-Cookie value. Dev over plain http drops the `__Host-`
    /// prefix and the Secure flag, nowhere else.
    fn session_cookie_value(state: &AppState, token: &str) -> String {
        let secure = if state.0.cookie_secure {
            "; Secure"
        } else {
            ""
        };
        format!(
            "{}={}; HttpOnly; SameSite=Strict; Path=/{}",
            state.0.cookie_name, token, secure
        )
    }

    pub async fn me(state: AppState, req: Request) -> Response {
        let token = cookie_of(&req);
        let session = match require_session(&state, token).await {
            Ok(session) => session,
            Err(response) => return *response,
        };
        let csrf = state
            .0
            .cookie_key
            .map(|key| riehub_identity::session::csrf_token(&key, &session.sid))
            .unwrap_or_default();
        (
            StatusCode::OK,
            Json(json!({
                "user_id": session.user_id.to_string(),
                "session_id": session.session_id,
                "csrf_token": csrf,
            })),
        )
            .into_response()
    }

    pub async fn logout(state: AppState, req: Request) -> Response {
        let token = cookie_of(&req);
        let Some(identity) = &state.0.identity else {
            return problem(StatusCode::SERVICE_UNAVAILABLE);
        };
        let session = match require_session(&state, token).await {
            Ok(session) => session,
            Err(response) => return *response,
        };
        match identity
            .end_session(&session.user_id, &session.sid, &session.unlocked)
            .await
        {
            Ok(()) => {
                let mut response = (StatusCode::OK, Json(json!({"status": "ok"}))).into_response();
                let clear = format!(
                    "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
                    state.0.cookie_name
                );
                if let Ok(value) = HeaderValue::from_str(&clear) {
                    response.headers_mut().insert(header::SET_COOKIE, value);
                }
                response
            }
            Err(e) => problem_response(&e, None),
        }
    }

    pub async fn my_sessions(state: AppState, req: Request) -> Response {
        let token = cookie_of(&req);
        let Some(identity) = &state.0.identity else {
            return problem(StatusCode::SERVICE_UNAVAILABLE);
        };
        let session = match require_session(&state, token).await {
            Ok(session) => session,
            Err(response) => return *response,
        };
        match identity.list_sessions(&session.user_id).await {
            Ok(sessions) => (StatusCode::OK, Json(json!({"items": sessions}))).into_response(),
            Err(e) => problem_response(&e, None),
        }
    }

    pub async fn end_session(state: AppState, req: Request) -> Response {
        let token = cookie_of(&req);
        let Some(identity) = &state.0.identity else {
            return problem(StatusCode::SERVICE_UNAVAILABLE);
        };
        let session = match require_session(&state, token).await {
            Ok(session) => session,
            Err(response) => return *response,
        };
        // The id is the last path segment; never logged.
        let target = req.uri().path().rsplit('/').next().unwrap_or("").to_owned();
        // The caller may end only its own sessions.
        let owned = identity
            .list_sessions(&session.user_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .any(|s| s.session_id.ends_with(&target) || s.sid == target);
        if !owned {
            return problem_response(
                &riehub_core::Error::NotFound("no such session".to_owned()),
                None,
            );
        }
        match identity
            .end_session(&session.user_id, &target, &session.unlocked)
            .await
        {
            Ok(()) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
            Err(e) => problem_response(&e, None),
        }
    }

    /// Reads the session cookie from the request.
    fn cookie_of(req: &Request) -> Option<String> {
        let header = req
            .headers()
            .get(axum::http::header::COOKIE)?
            .to_str()
            .ok()?;
        for pair in header.split(';') {
            let (name, value) = pair.trim().split_once('=')?;
            if name == "__Host-rs" || name == "rs" {
                return Some(value.to_owned());
            }
        }
        None
    }

    /// The authenticated session for cookie-carrying requests. A
    /// missing or bad cookie maps to 401. The cookie is read before
    /// any await so the future stays Send.
    async fn require_session(
        state: &AppState,
        token: Option<String>,
    ) -> std::result::Result<riehub_identity::session::AuthedSession, Box<Response>> {
        let Some(identity) = &state.0.identity else {
            return Err(Box::new(problem(StatusCode::SERVICE_UNAVAILABLE)));
        };
        let token = token.ok_or_else(|| Box::new(problem(StatusCode::UNAUTHORIZED)))?;
        identity
            .validate_session(&token)
            .await
            .map_err(|_| Box::new(problem(StatusCode::UNAUTHORIZED)))
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
        let Some(cookie_key) = &state.0.cookie_key else {
            return problem_response(
                &riehub_core::Error::Unavailable("the instance coffer is not open".to_owned()),
                request_id.as_deref(),
            );
        };
        match identity
            .login(&email, &password, ip.as_deref(), cookie_key)
            .await
        {
            Ok((user_id, session)) => {
                let cookie = session_cookie_value(&state, &session.token);
                let mut response = (
                    StatusCode::OK,
                    Json(json!({
                        "status": "ok",
                        "user_id": user_id.to_string(),
                        "csrf_token": session.csrf_token,
                    })),
                )
                    .into_response();
                if let Ok(value) = HeaderValue::from_str(&cookie) {
                    response.headers_mut().insert(header::SET_COOKIE, value);
                }
                response
            }
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
