# Automation

The library (skills, instructions, agent profiles), chats, and
workflows. Crate: `riehub-automation`. Everything on this page is
user content: stored as encrypted records, owned by one user, invisible
to everyone else. Decision: ADR 0013 (standing grants).

## 1. The library

| Item | What it is |
|---|---|
| **Skill** | a folder in the open `SKILL.md` format: YAML front matter with `name` and `description`, a markdown body, optional attached files. At most 5 MiB per skill |
| **Instruction** | a named system prompt |
| **Agent profile** | name, model or routing group, instruction, skills, allowed tools, auto-approve list |
| **MCP set** | the user's connectors and virtual MCPs ([MCP](mcp.md)) |

Skills are validated on save: front matter parses, `name` matches
`[a-z0-9-]{1,64}`, files have relative paths without `..`. Import and
export use a `.tar.gz` of the folder. Archive extraction rejects
absolute paths, `..`, links and anything over the size limit.

How a skill is used: the name and description of each enabled skill
go into the system prompt. The body is loaded when the model calls the
built-in tool `skill_load(name)`. Attached files are read with
`skill_file(name, path)`. Skills contain text only. riehub never
executes a file from a skill.

The `rie` CLI writes library items into the formats of local coding
agents ([API](api.md), section 5).

## 2. Chats

A chat is a conversation in the web UI or over the API, using the
router, an agent profile and the user's tools.

- `chat` record: profile, model, created and updated times. Title is
  content.
- `chat_message` records: role and parts in the canonical model
  ([router](router.md)), encrypted, append only, ordered by `sort`.
- `POST /api/v1/chats/{id}/messages` streams the reply as server-sent
  events of canonical `ChatEvent`s plus `tool_approval` events.
- Tool calls outside the profile's auto-approve list pause the turn
  until the user approves or refuses in the UI.
- A chat is unlocked by the session that uses it. There is no server
  process that can read chats while the user is away.

## 3. Workflows

A workflow is a YAML file the user writes. One workflow, one record.

```yaml
version: 1
name: weekly-report
description: Collect issues and draft a report.
inputs:
  topic: { type: string, required: true }
triggers:
  - manual
  - schedule: "0 9 * * 1"
    timezone: Asia/Jakarta
  - webhook
defaults:
  model: writing-group
limits:
  max_steps: 50
  max_duration: 15m
  max_cost: 2.00
steps:
  - id: research
    tool: github__search_issues
    with:
      query: ${{ inputs.topic }}
  - id: draft
    prompt: |
      Write a short report from: ${{ steps.research.output }}
    skills: [report-writer]
    tools: [notion__create_page]
    max_steps: 8
  - id: confirm
    approval: Publish the report?
  - id: publish
    if: ${{ steps.confirm.approved }}
    tool: notion__create_page
    with:
      title: Weekly report
      body: ${{ steps.draft.output }}
outputs:
  report: ${{ steps.draft.output }}
```

### 3.1 Schema

| Key | Rule |
|---|---|
| `version` | must be `1` |
| `name` | `[a-z0-9-]{1,64}`, unique per user |
| `inputs` | map of name to `{ type: string or number or boolean, required, default }` |
| `triggers` | any of `manual`, `schedule` (five-field cron, optional `timezone`), `webhook` |
| `limits` | `max_steps` (default 50, cap 500), `max_duration` (default 15m, cap 6h), `max_cost` (default none) |
| `steps` | list, each with a unique `id` and exactly one kind |

Step kinds:

| Kind | Keys | Does |
|---|---|---|
| `prompt` | `prompt`, optional `model`, `instruction`, `skills`, `tools`, `max_steps`, `output: text or json` | one model call, or an agent loop when `tools` is set. The listed tools are auto-approved for this step |
| `tool` | `tool`, `with` | one MCP tool call |
| `approval` | `approval` | pauses until the owner approves or refuses. Sets `approved` |
| `foreach` | `foreach`, `steps`, optional `max_items` (default 100) | runs nested steps per item, in order |
| `workflow` | `workflow`, `with` | runs another workflow of the same user, at most 3 levels deep |
| `set` | `set` | names computed values |

Every step accepts `if`, `timeout` and `on_error: fail or continue`.

Expressions are `${{ ... }}` with: paths (`inputs.x`,
`steps.<id>.output`, `steps.<id>.output.a.b[0]`, `item`), literals,
`==`, `!=`, `<`, `>`, `&&`, `||`, `!`. There are no function calls and
no code execution. There is no shell or script step, by design.

The parser rejects unknown keys, duplicate ids, references to later or
missing steps, and files over 256 KiB. The JSON Schema is published at
`docs/schemas/workflow-v1.json` and generated from the Rust types.

### 3.2 Runs

States: `queued`, `running`, `waiting_approval`, `succeeded`, `failed`,
`cancelled`, `timed_out`. A run record holds the state and counters in
`meta`. Inputs, step outputs and errors are encrypted in `data`.

A run stops when a limit is reached. Every model call and tool call in
a run passes the same governance checks as a direct request, and is
charged to the user.

### 3.3 Triggers and standing grants

- **Manual**: started from a session or with a `manage` hub key. The
  run uses the vault that the session or key unlocked.
- **Schedule** and **webhook**: nobody is present to unlock the vault.
  The user must create a **standing grant** for the workflow first.

A standing grant is an unlock slot (`sg.<ID>`) whose key is kept in the
instance coffer. It is created only from a web session, after the UI
shows this text: "This lets the server run this workflow when you are
away. While the grant exists, the host can use the connections this
workflow needs." Its ACL covers only the connections and connectors
the workflow names. It is listed on the user's security page, written
to the audit log, and removed when the user revokes it or changes the
workflow's tools.

Schedules are run by a scheduler that takes a lease in the store, so
one node fires each tick once. A webhook trigger is `POST
/hooks/<wf_id>` with an `x-riehub-signature` header:
`hmac(webhook_secret, timestamp || "." || body)`, refused when older
than 5 minutes or replayed.
