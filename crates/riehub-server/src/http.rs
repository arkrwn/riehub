//! The HTTP application: middleware stack in the spec order, problem
//! json errors, and the router built from the static route table.

use crate::routes::{RouteClass, RouteSpec};
use axum::error_handling::HandleErrorLayer;
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::MethodRouter;
use axum::{Json, Router};
use riehub_core::error::Error;
use riehub_store::sqlite::SqliteStore;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::timeout::TimeoutLayer;
use tower::ServiceBuilder;

pub const DEFAULT_BODY_LIMIT: usize = 33_554_432; // 32 MiB
pub const DEFAULT_HEADER_LIMIT: usize = 65_536; // 64 KiB
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// The shared server state. A `None` store means the store is down.
#[derive(Clone, Debug)]
pub struct AppState(pub Arc<InnerState>);

impl std::fmt::Debug for InnerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InnerState")
            .field("store", &self.store.is_some())
            .field("identity", &self.identity.is_some())
            .field("cookie_key", &self.cookie_key.is_some())
            .field("timeout", &self.timeout)
            .field("body_limit", &self.body_limit)
            .field("header_limit", &self.header_limit)
            .finish()
    }
}

pub struct InnerState {
    pub store: Option<Arc<SqliteStore>>,
    pub identity: Option<Arc<riehub_identity::Service>>,
    pub cookie_key: Option<[u8; 32]>,
    pub cookie_name: &'static str,
    pub cookie_secure: bool,
    pub timeout: Duration,
    pub body_limit: usize,
    pub header_limit: usize,
    /// The (method, path) pairs of the route table, for the access log
    /// route templates.
    pub route_templates: Arc<Vec<(&'static Method, &'static str)>>,
}

#[derive(Debug)]
pub struct RouterInput {
    pub store: Option<Arc<SqliteStore>>,
    pub audit: Option<Arc<riehub_audit::AuditLog<SqliteStore>>>,
    pub cookie_key: Option<[u8; 32]>,
    pub cookie_name: &'static str,
    pub cookie_secure: bool,
    pub timeout: Duration,
    pub body_limit: usize,
    pub header_limit: usize,
}

impl Default for RouterInput {
    fn default() -> Self {
        Self {
            store: None,
            audit: None,
            cookie_key: None,
            cookie_name: "__Host-rs",
            cookie_secure: true,
            timeout: DEFAULT_TIMEOUT,
            body_limit: DEFAULT_BODY_LIMIT,
            header_limit: DEFAULT_HEADER_LIMIT,
        }
    }
}

/// Builds the router from the table and nothing else (api spec,
/// section 2). Middleware order: request id, body and header limits,
/// timeout, security headers, access log.
pub fn build_router(input: RouterInput, table: &'static [RouteSpec]) -> Router {
    let templates: Arc<Vec<(&'static Method, &'static str)>> =
        Arc::new(table.iter().map(|spec| (spec.method, spec.path)).collect());
    let identity = match (&input.store, &input.audit) {
        (Some(store), Some(audit)) => Some(Arc::new(riehub_identity::Service::new(
            store.clone(),
            audit.clone(),
        ))),
        _ => None,
    };
    let state = AppState(Arc::new(InnerState {
        store: input.store,
        identity,
        cookie_key: input.cookie_key,
        cookie_name: input.cookie_name,
        cookie_secure: input.cookie_secure,
        timeout: input.timeout,
        body_limit: input.body_limit,
        header_limit: input.header_limit,
        route_templates: templates,
    }));

    let mut router = Router::new();
    for spec in table {
        router = match spec.method {
            m if m == Method::GET => router.route(spec.path, method_route(spec)),
            m if m == Method::POST => router.route(spec.path, method_route(spec)),
            m if m == Method::PUT => router.route(spec.path, method_route(spec)),
            m if m == Method::PATCH => router.route(spec.path, method_route(spec)),
            m if m == Method::DELETE => router.route(spec.path, method_route(spec)),
            _ => router,
        };
    }

    router
        .fallback(fallback_404)
        .layer(DefaultBodyLimit::max(state.0.body_limit))
        .layer(
            ServiceBuilder::new()
                .layer(middleware::from_fn_with_state(state.clone(), csrf_mw))
                .layer(middleware::from_fn(request_id_mw))
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    header_limit_mw,
                ))
                .layer(middleware::from_fn_with_state(state.clone(), body_limit_mw))
                .layer(HandleErrorLayer::new(handle_timeout_error))
                .layer(TimeoutLayer::new(state.0.timeout))
                .layer(middleware::from_fn(security_headers_mw))
                .layer(middleware::from_fn_with_state(state.clone(), access_log_mw)),
        )
        .with_state(state)
}

fn method_route(spec: &'static RouteSpec) -> MethodRouter<AppState> {
    let handler = move |State(state): State<AppState>, req: Request| async move {
        (spec.handler)(state, req).await
    };
    if spec.method == Method::GET {
        axum::routing::get(handler)
    } else if spec.method == Method::POST {
        axum::routing::post(handler)
    } else if spec.method == Method::PUT {
        axum::routing::put(handler)
    } else if spec.method == Method::PATCH {
        axum::routing::patch(handler)
    } else {
        axum::routing::delete(handler)
    }
}

/// Maps riehub errors to problem json (api spec, section 1). The
/// detail never echoes request content.
pub fn problem_response(error: &Error, request_id: Option<&str>) -> Response {
    problem_response_with(error, request_id, &[])
}

/// A problem json response with extra headers, for example
/// `Retry-After` on a rate limited login.
pub fn problem_response_with(
    error: &Error,
    request_id: Option<&str>,
    extra: &[(&'static str, String)],
) -> Response {
    let (status, title, detail): (StatusCode, &'static str, String) = match error {
        Error::NotFound(m) => (StatusCode::NOT_FOUND, "not found", m.clone()),
        Error::Conflict(m) => (StatusCode::CONFLICT, "conflict", m.clone()),
        Error::Unauthorized(m) => (StatusCode::UNAUTHORIZED, "unauthorized", m.clone()),
        Error::Forbidden(m) => (StatusCode::FORBIDDEN, "forbidden", m.clone()),
        Error::Invalid(m) => (StatusCode::BAD_REQUEST, "invalid request", m.clone()),
        Error::RateLimited(m) => (StatusCode::TOO_MANY_REQUESTS, "rate limited", m.clone()),
        Error::Unavailable(m) => (StatusCode::SERVICE_UNAVAILABLE, "unavailable", m.clone()),
        Error::Internal(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal error",
            "internal error".to_owned(),
        ),
    };
    let body = json!({
        "type": format!("https://riehub.dev/problems/{}", slug(title)),
        "title": title,
        "status": status.as_u16(),
        "detail": detail,
        "request_id": request_id.unwrap_or(""),
    });
    let mut response = (status, Json(body)).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    for (name, value) in extra {
        if let Ok(v) = HeaderValue::from_str(value) {
            response.headers_mut().insert(*name, v);
        }
    }
    response
}

fn slug(title: &str) -> String {
    title
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect()
}

async fn fallback_404(State(state): State<AppState>, req: Request) -> Response {
    let request_id = req.extensions().get::<String>().cloned();
    let mut response = problem_response(
        &Error::NotFound("no such route".to_owned()),
        request_id.as_deref(),
    );
    response.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(request_id.as_deref().unwrap_or(""))
            .unwrap_or(HeaderValue::from_static("")),
    );
    let _ = state;
    response
}

/// State-changing /api requests that carry a session cookie need the
/// `x-csrf-token` header (security spec, section 7). The token is
/// `hmac(cookie_key, sid)`; GET /api/v1/me hands it out.
async fn csrf_mw(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    let method = req.method();
    let mutating = !(method == Method::GET || method == Method::HEAD || method == Method::OPTIONS);
    let cookie = session_cookie(&req, &state);
    if mutating && path.starts_with("/api") {
        if let Some(sid) = cookie.as_ref().and_then(|token| sid_of(token)) {
            if let Some(key) = &state.0.cookie_key {
                let expected = riehub_identity::session::csrf_token(key, &sid);
                let provided = req
                    .headers()
                    .get("x-csrf-token")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if provided != expected {
                    return problem_response(
                        &Error::Forbidden("the csrf token does not match".to_owned()),
                        None,
                    );
                }
            }
        }
    }
    next.run(req).await
}

/// The session cookie value, whatever name the profile uses.
fn session_cookie(req: &Request, state: &AppState) -> Option<String> {
    let header = req.headers().get(header::COOKIE)?.to_str().ok()?;
    for pair in header.split(';') {
        let (name, value) = pair.trim().split_once('=')?;
        if name == state.0.cookie_name || name == riehub_identity::session::COOKIE_NAME_HOST {
            return Some(value.to_owned());
        }
    }
    None
}

/// The sid between the token prefixes. The base64url secret may hold
/// dashes, so split at most twice.
fn sid_of(token: &str) -> Option<String> {
    let parts: Vec<&str> = token.splitn(3, '_').collect();
    if parts.len() == 3 && parts[0] == "rs" {
        Some(parts[1].to_owned())
    } else {
        None
    }
}

/// Generates a fresh request id and echoes it on the response.
async fn request_id_mw(mut req: Request, next: Next) -> Response {
    let id = riehub_core::RequestId::new().as_str().to_owned();
    req.extensions_mut().insert(id.clone());
    let mut response = next.run(req).await;
    if let Ok(value) = HeaderValue::from_str(&id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

async fn body_limit_mw(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let too_large = req
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .map(|len| len > state.0.body_limit as u64)
        .unwrap_or(false);
    if too_large {
        return problem_response(
            &Error::Invalid("the request body is too large".to_owned()),
            req.extensions().get::<String>().map(|s| s.as_str()),
        )
        .with_status(StatusCode::PAYLOAD_TOO_LARGE);
    }
    next.run(req).await
}

async fn header_limit_mw(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let total: usize = req
        .headers()
        .iter()
        .map(|(name, value)| name.as_str().len() + value.len())
        .sum();
    if total > state.0.header_limit {
        return problem_response(
            &Error::Invalid("the request headers are too large".to_owned()),
            req.extensions().get::<String>().map(|s| s.as_str()),
        )
        .with_status(StatusCode::PAYLOAD_TOO_LARGE);
    }
    next.run(req).await
}

async fn security_headers_mw(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("text/html"))
        .unwrap_or(false);
    let headers = response.headers_mut();
    headers.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    if is_html {
        headers.insert(
            "content-security-policy",
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
            ),
        );
    }
    response
}

/// The access log carries only the allowlisted fields (api spec,
/// section 1). Never the path with ids, the query string or headers.
async fn access_log_mw(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let request_id = req
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_default();

    let response = next.run(req).await;

    let template = state
        .0
        .route_templates
        .iter()
        .find(|(m, p)| *m == method && *p == path)
        .map(|(_, p)| *p)
        .unwrap_or("<unmatched>");
    let status = response.status().as_u16();
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(0);
    tracing::info!(
        request_id = request_id.as_str(),
        method = method.as_str(),
        route_template = template,
        status = status,
        duration_ms = duration_ms,
    );
    response
}

async fn handle_timeout_error(error: axum::BoxError) -> Response {
    if error.is::<tower::timeout::error::Elapsed>() {
        return problem_response(
            &Error::Unavailable("the request timed out".to_owned()),
            None,
        );
    }
    problem_response(&Error::Internal(None), None)
}

/// Convenience for tests and handlers that need a different status on
/// a problem body.
trait WithStatus: Sized {
    fn with_status(self, status: StatusCode) -> Response;
}

impl WithStatus for Response {
    fn with_status(mut self, status: StatusCode) -> Response {
        *self.status_mut() = status;
        self
    }
}

/// The route classes must all come from the fixed set (api spec,
/// section 2).
/// The one response type allowed to carry the armored instance or
/// user recovery key. It is built only by the setup handler and is
/// the only place the key leaves the server in plaintext. The body
/// only leaves through Debug-free json, so no Debug here.
#[allow(missing_debug_implementations)]
pub struct SetupResponse {
    user_id: String,
    recovery_key: String,
}

impl SetupResponse {
    pub fn new(user_id: String, recovery_key: String) -> Self {
        Self {
            user_id,
            recovery_key,
        }
    }
}

impl IntoResponse for SetupResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::OK,
            Json(json!({
                "status": "ok",
                "user_id": self.user_id,
                "recovery_key": self.recovery_key,
            })),
        )
            .into_response()
    }
}

pub fn class_name(class: &RouteClass) -> &'static str {
    match class {
        RouteClass::Public => "public",
        RouteClass::Self_ => "self",
        RouteClass::AdminMeta => "admin_meta",
        RouteClass::Key(_) => "key",
        RouteClass::Hook => "hook",
    }
}
