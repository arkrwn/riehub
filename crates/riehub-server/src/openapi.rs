//! The OpenAPI 3 document of the existing API surface (web spec,
//! section 1: API types are generated from the committed document).
//! Built programmatically from the same knowledge as the route table
//! in `routes.rs`. `riehub openapi` prints it, and the Makefile drift
//! check compares the printed document with `docs/api/openapi.json`.

use utoipa::openapi::path::{HttpMethod, Operation, OperationBuilder, PathItem, PathItemBuilder};
use utoipa::openapi::request_body::RequestBodyBuilder;
use utoipa::openapi::response::ResponseBuilder;
use utoipa::openapi::schema::{Object, ObjectBuilder, Schema, Type};
use utoipa::openapi::{Content, InfoBuilder, OpenApi, Required};

/// Builds the whole OpenAPI document of the current API surface.
pub fn openapi_document() -> OpenApi {
    let mut paths = utoipa::openapi::Paths::new();

    put(
        &mut paths,
        "/healthz",
        [get(
            "Liveness",
            "Always answers 200 while the process is up. No credential.",
            None,
            fields(&[("status", "string")]),
        )],
    );
    put(
        &mut paths,
        "/readyz",
        [get(
            "Readiness",
            "200 when the store answers a ping, 503 problem json otherwise. No credential.",
            None,
            fields(&[("status", "string")]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/setup",
        [post(
            "Setup the owner",
            "Creates the first user with the owner role. Fails 409 once the instance is set up.",
            body(&[
                ("email", "string", true),
                ("password", "string", true),
                ("display_name", "string", false),
            ]),
            fields(&[
                ("status", "string"),
                ("user_id", "string"),
                ("recovery_key", "string"),
            ]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/auth/login",
        [post(
            "Password login",
            "Sets the session cookie. Rate limited per account and per IP with `Retry-After`.",
            body(&[("email", "string", true), ("password", "string", true)]),
            fields(&[
                ("status", "string"),
                ("user_id", "string"),
                ("csrf_token", "string"),
            ]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/auth/recover",
        [post(
            "Recover with the recovery key",
            "Resets the password with the recovery key.",
            body(&[
                ("email", "string", true),
                ("recovery_key", "string", true),
                ("new_password", "string", true),
            ]),
            fields(&[("status", "string"), ("user_id", "string")]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/auth/logout",
        [post(
            "Logout",
            "Ends the session and clears the cookie.",
            None,
            fields(&[("status", "string")]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/me",
        [get(
            "The caller",
            "The user id, session id and csrf token of the caller.",
            None,
            fields(&[
                ("user_id", "string"),
                ("session_id", "string"),
                ("csrf_token", "string"),
                ("via", "string"),
            ]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/me/sessions",
        [get(
            "List my sessions",
            "The caller's own web sessions.",
            None,
            fields(&[("items", "array")]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/me/sessions/{id}",
        [delete(
            "End one of my sessions",
            "Ends one session of the caller. 404 for a session of somebody else.",
            None,
            fields(&[("status", "string")]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/me/keys",
        [
            get(
                "List my hub keys",
                "The hub keys of the caller, metadata only, never the tokens.",
                None,
                fields(&[("items", "array")]),
            ),
            post(
                "Create a hub key",
                "A session creates a hub key. The token in the answer is shown once.",
                body(&[
                    ("label", "string", false),
                    ("scopes", "array", true),
                    ("acl_prefixes", "array", false),
                    ("expires_at", "integer", false),
                ]),
                fields(&[
                    ("id", "string"),
                    ("token", "string"),
                    ("scopes", "array"),
                    ("acl_prefixes", "array"),
                ]),
            ),
        ],
    );
    put(
        &mut paths,
        "/api/v1/me/keys/{id}",
        [delete(
            "Delete one of my hub keys",
            "Removes one hub key of the caller.",
            None,
            fields(&[("status", "string")]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/me/password",
        [post(
            "Change my password",
            "A session changes the password of the caller.",
            body(&[
                ("current_password", "string", true),
                ("new_password", "string", true),
            ]),
            fields(&[("status", "string")]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/me/recovery-key/rotate",
        [post(
            "Rotate my recovery key",
            "A session rotates the recovery key. The new key in the answer is shown once.",
            None,
            fields(&[
                ("status", "string"),
                ("user_id", "string"),
                ("recovery_key", "string"),
            ]),
        )],
    );
    put(
        &mut paths,
        "/v1/models",
        [get(
            "List models",
            "The model catalog. A hub key with the ai scope or a session.",
            None,
            fields(&[("models", "array")]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/admin/users",
        [
            get(
                "List users (admin)",
                "Instance metadata, needs the admin role. Ids, emails, names, statuses and roles only.",
                None,
                fields(&[("items", "array")]),
            ),
            post(
                "Create a user (admin)",
                "Creates a user and an invite token. Needs the admin role.",
                body(&[("email", "string", true), ("display_name", "string", false)]),
                fields(&[("user_id", "string"), ("invite_token", "string")]),
            ),
        ],
    );
    put(
        &mut paths,
        "/api/v1/admin/users/{id}",
        [
            patch(
                "Patch a user (admin)",
                "Changes roles or status of a user. Needs the admin role.",
                body(&[("roles", "array", false), ("status", "string", false)]),
                fields(&[("status", "string")]),
            ),
            delete(
                "Delete a user (admin)",
                "Removes a user. Needs the admin role.",
                None,
                fields(&[("status", "string")]),
            ),
        ],
    );
    put(
        &mut paths,
        "/api/v1/admin/invites",
        [post(
            "Create an invite (admin)",
            "Creates an invite token for an email. Needs the admin role.",
            body(&[("email", "string", true), ("display_name", "string", false)]),
            fields(&[("invite_token", "string")]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/admin/users/{id}/reset",
        [post(
            "Reset a user (admin)",
            "Issues a fresh invite token the user can use to set a new password.",
            None,
            fields(&[("invite_token", "string")]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/admin/audit",
        [get(
            "Read the audit log (admin)",
            "The audit rows, metadata only. Needs the admin role.",
            None,
            fields(&[("items", "array")]),
        )],
    );
    put(
        &mut paths,
        "/api/v1/invites/{token}",
        [
            get(
                "Invite info",
                "The email and expiry of an invite token. No credential.",
                None,
                fields(&[("email", "string"), ("expires_at", "string")]),
            ),
            post(
                "Accept an invite",
                "Sets the password and creates the account. No credential.",
                body(&[
                    ("password", "string", true),
                    ("recovery_key", "string", false),
                ]),
                fields(&[
                    ("status", "string"),
                    ("user_id", "string"),
                    ("recovery_key", "string"),
                ]),
            ),
        ],
    );

    let info = InfoBuilder::new()
        .title("riehub API")
        .version(env!("CARGO_PKG_VERSION"))
        .description(Some(
            "One private hub for every AI model, tool, skill and workflow. Session \
             calls carry the `__Host-rs` HttpOnly cookie and, on state changes, the \
             `x-csrf-token` header.",
        ))
        .build();

    OpenApi::builder().info(info).paths(paths).build()
}

/// Adds one path with all its operations to the document.
fn put<const N: usize>(
    paths: &mut utoipa::openapi::Paths,
    path: &str,
    ops: [(HttpMethod, Operation); N],
) {
    paths.paths.insert(path.to_owned(), item(&ops));
}

/// A path with one or more operations.
fn item(operations: &[(HttpMethod, Operation)]) -> PathItem {
    let mut build = PathItemBuilder::new();
    for (method, operation) in operations {
        build = build.operation(method.clone(), operation.clone());
    }
    build.build()
}

fn get(
    summary: &str,
    description: &str,
    request: Option<Schema>,
    answer: Schema,
) -> (HttpMethod, Operation) {
    (HttpMethod::Get, of(summary, description, request, answer))
}

fn post(
    summary: &str,
    description: &str,
    request: Option<Schema>,
    answer: Schema,
) -> (HttpMethod, Operation) {
    (HttpMethod::Post, of(summary, description, request, answer))
}

fn patch(
    summary: &str,
    description: &str,
    request: Option<Schema>,
    answer: Schema,
) -> (HttpMethod, Operation) {
    (HttpMethod::Patch, of(summary, description, request, answer))
}

fn delete(
    summary: &str,
    description: &str,
    request: Option<Schema>,
    answer: Schema,
) -> (HttpMethod, Operation) {
    (
        HttpMethod::Delete,
        of(summary, description, request, answer),
    )
}

fn of(summary: &str, description: &str, request: Option<Schema>, answer: Schema) -> Operation {
    let mut builder = OperationBuilder::new()
        .summary(Some(summary.to_owned()))
        .description(Some(description.to_owned()))
        .tag("riehub")
        .response("200", ok_response("The answer.", answer))
        .response("4XX", problem_response("A problem json error."));
    if let Some(body) = request {
        builder = builder.request_body(Some(
            RequestBodyBuilder::new()
                .required(Some(Required::True))
                .content("application/json", Content::new(Some(body)))
                .build(),
        ));
    }
    builder.build()
}

/// A schema with the named primitive properties.
fn fields(list: &[(&str, &str)]) -> Schema {
    let mut builder = ObjectBuilder::new();
    for (name, kind) in list {
        builder = builder.property(*name, primitive(kind));
    }
    Schema::Object(builder.build())
}

/// A request body schema, with required fields.
fn body(list: &[(&str, &str, bool)]) -> Option<Schema> {
    Some(req(list))
}

/// A request body schema, with required fields.
fn req(list: &[(&str, &str, bool)]) -> Schema {
    let mut builder = ObjectBuilder::new();
    for (name, kind, required) in list {
        builder = builder.property(*name, primitive(kind));
        if *required {
            builder = builder.required(*name);
        }
    }
    Schema::Object(builder.build())
}

fn primitive(kind: &str) -> Schema {
    let kind = match kind {
        "string" => Type::String,
        "integer" => Type::Integer,
        "array" => Type::Array,
        "object" => Type::Object,
        _ => Type::String,
    };
    Schema::Object(Object::builder().schema_type(kind).build())
}

fn ok_response(description: &str, schema: Schema) -> ResponseBuilder {
    ResponseBuilder::new()
        .description(description)
        .content("application/json", Content::new(Some(schema)))
}

fn problem_response(description: &str) -> ResponseBuilder {
    ResponseBuilder::new().description(description).content(
        "application/problem+json",
        Content::new(Some(fields(&[
            ("type", "string"),
            ("title", "string"),
            ("status", "integer"),
            ("detail", "string"),
            ("request_id", "string"),
        ]))),
    )
}
