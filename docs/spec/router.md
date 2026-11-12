# Router

One request model, four inbound API surfaces, many providers. Crate:
`riehub-router`. Decisions: ADR 0007 (canonical model), ADR 0008
(connection types).

## 1. Canonical request model

Every surface decodes into these types and every provider adapter
encodes from them. No adapter talks to another adapter's format.

```text
ChatRequest  { model: ModelRef, system: Vec<Part>, messages: Vec<Message>, tools: Vec<ToolDef>,
               tool_choice, max_output_tokens, temperature, top_p, stop, reasoning: Option<Reasoning>,
               response_format, stream: bool, metadata: RequestMeta, extra: PassThrough }
Message      { role: user | assistant | tool, parts: Vec<Part> }
Part         = Text | Image { media_type, data | url } | File | ToolCall { id, name, arguments }
             | ToolResult { call_id, parts, is_error } | Reasoning { text, signature }
ChatEvent    = Start { id, model } | TextDelta | ReasoningDelta | ToolCallStart | ToolCallDelta
             | ToolCallEnd | Usage(Usage) | Stop { reason } | Error
Usage        { input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens }
StopReason   = end | max_tokens | tool_use | stop_sequence | content_filter | error
```

All text and bytes inside are `Content`. `PassThrough` keeps fields
the model does not know, keyed by surface, and is forwarded only when
the provider speaks the same format as the caller. A non-streaming
response is the fold of the event stream.

`EmbedRequest { model, inputs, dimensions }` and `EmbedResponse` cover
embeddings.

## 2. Inbound surfaces

| Surface | Routes | Auth header |
|---|---|---|
| OpenAI Chat Completions | `POST /v1/chat/completions`, `GET /v1/models`, `POST /v1/embeddings` | `Authorization: Bearer` |
| OpenAI Responses | `POST /v1/responses` | `Authorization: Bearer` |
| Anthropic Messages | `POST /v1/messages`, `POST /v1/messages/count_tokens` | `x-api-key` or `Authorization: Bearer` |
| Gemini | `POST /v1beta/models/{model}:generateContent`, `:streamGenerateContent`, `:countTokens`, `:embedContent`, `GET /v1beta/models` | `x-goog-api-key` or `?key=` |

Each surface has a codec with four functions: decode request, encode
response, encode stream event, encode error. Errors use the surface's
own error shape and status codes, so SDK retry logic keeps working.
Streaming uses server-sent events in the surface's own event grammar.

The credential on these routes is a hub key with the `ai` scope.

## 3. Model addressing

The `model` a caller sends resolves in this order:

1. A **routing group** or **alias** the user owns with that name.
2. `<connection-slug>/<model-id>`: a model on one named connection.
3. A bare `<model-id>`: the user's default routing for that model, which
   is every enabled connection that offers it, in `order` strategy.

Unknown names get the surface's "model not found" error. `GET
/v1/models` lists the user's groups, aliases and reachable models.

## 4. Providers and connections

A **provider definition** lives in the instance catalog: kind, base
URL, supported auth types, model list source, price table. riehub ships
definitions for the common providers. Admins add more. Kinds:

| Kind | Upstream format | Covers |
|---|---|---|
| `openai` | Chat Completions and Responses | OpenAI, Azure OpenAI, and every compatible API: DeepSeek, Mistral, Groq, OpenRouter, xAI, Ollama, LM Studio, vLLM |
| `anthropic` | Messages | Anthropic, and Anthropic-compatible endpoints |
| `gemini` | generateContent | Google AI Studio, Vertex AI |
| `bedrock` | Converse | AWS Bedrock (version 0.4.0) |

A **connection** belongs to one user: a provider definition plus that
user's credential, stored in their vault. A user may have several
connections to the same provider (several accounts).

| Auth type | Credential | Since |
|---|---|---|
| `api_key` | static key | 0.2.0 |
| `none` | nothing (local runtimes) | 0.2.0 |
| `oauth2` | token set from authorization code with PKCE, or device code, with refresh | 0.4.0 |
| `cloud` | AWS, Google or Azure credential set, signed per request | 0.4.0 |

OAuth sign-in exists only for providers whose terms allow third-party
clients, each recorded in the catalog entry (ADR 0008). Token refresh
happens inside a request that holds the unlocked vault, and writes the
new token set back.

A **shared connection** belongs to the instance. Its credential is in
the instance coffer. Users reach it only through an access profile
([governance](governance.md)).

A user-supplied base URL always goes through the guarded HTTP client
(S11). Local runtimes need the host in `egress.allow`.

## 5. Provider adapters

```text
trait Provider {
    fn kind(&self) -> ProviderKind;
    async fn chat(&self, req: ChatRequest, cred: &Credential, http: &GuardedClient) -> Result<EventStream, ProviderError>;
    async fn embed(&self, req: EmbedRequest, cred: &Credential, http: &GuardedClient) -> Result<EmbedResponse, ProviderError>;
    async fn models(&self, cred: &Credential, http: &GuardedClient) -> Result<Vec<ModelInfo>, ProviderError>;
}
```

`ProviderError` has a class used by routing: `auth`, `quota`,
`rate_limited { retry_after }`, `overloaded`, `timeout`, `bad_request`,
`content_filter`, `server`, `network`. Provider error text is `Content`
and reaches only the caller.

Adapters are tested against a local mock upstream that replays recorded
fixtures. No test calls a real provider.

## 6. Routing groups

A routing group is an ordered list of targets (connection and model, or
another group, at most 3 levels deep) with a strategy:

| Strategy | Picks |
|---|---|
| `order` | the first healthy target, the next on a retryable error |
| `rotate` | round robin over healthy targets |
| `weighted` | random by weight |
| `usage` | the target with the most quota left in its current window |
| `pace` | spreads use so each quota window lasts until it resets |
| `smart` | scores health, recent latency, error rate and quota left |

Shared behaviour:

- **Fallback** on error classes `quota`, `rate_limited`, `overloaded`,
  `timeout`, `server`, `network`. Never on `bad_request`,
  `content_filter` or `auth` (an `auth` error marks the connection as
  needing attention). At most 3 attempts per request by default. No
  fallback after the first event has been sent to the caller.
- **Circuit breaker** per target: open after 5 failures in 30 seconds,
  half-open probe after 30 seconds.
- **Sticky conversations**: requests that share a conversation key (a
  hash of the hub key id and the first message, held in memory only)
  stay on one target while it is healthy, so provider prompt caches
  keep working.
- **Intent routing** (version 0.6.0, opt in per group): a small model
  the user chooses labels the request, and rules map labels to targets.

Health, breaker state and sticky keys are per node and in memory. Quota
windows and counters are in the store, so nodes agree on them.

## 7. Usage and cost

After every request the router writes one usage row and updates the
hourly buckets ([storage](storage.md)):

```text
ts, request_id, user_id, team_id, hub_key_id, project_id, surface, provider_kind,
catalog_model_id, connection_id, tokens (five counters), cost_micros, currency,
latency_ms, time_to_first_token_ms, status, error_class, attempts
```

Cost is tokens times the catalog price table, in millionths of the
price currency. Users can override prices per connection. A row never
holds content. `connection_id` is visible only to the owning user.
Admin views aggregate by user, team, provider kind and model.
