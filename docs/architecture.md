# Architecture

mon is a thin, browser-native Go CLI for Monarch Money. Rather than storing
ephemeral API tokens or passwords that trigger CAPTCHAs, it delegates network
transport and authentication directly to an active Monarch Money browser tab via
[bro](https://github.com/xiaotianxt/bro).

## Packages

- `main.go`: entry point; embeds `VERSION`, the single source of the release version.
- `internal/cli`: cobra command tree that parses into typed commands, plus their handlers and `install`.
- `internal/browser`: bro MCP client, Monarch tab discovery and readiness, and in-page GraphQL execution.
- `internal/graphql`: typed parsing and classification of top-level GraphQL errors.
- `internal/queries`: built-in GraphQL documents and variable builders.
- `internal/txupdate`: exact category resolution, single-transaction mutation policy, payload validation, and read-back outcome classification.
- `internal/output`: compact human tables and formatted JSON printing.
- `internal/jsonx`: dynamic JSON helpers that preserve number precision.
- `internal/paths`: home directory and tilde expansion.

## Execution Model

The CLI connects to the local bro MCP server on `127.0.0.1:3500`, locates an
open `https://app.monarch.com/` tab in a Chromium-family browser (Helium or
Chrome), and executes GraphQL queries directly inside the page context:

```javascript
fetch("https://api.monarch.com/graphql", {
  method: "POST",
  credentials: "include",
  headers: {
    "accept": "application/json",
    "content-type": "application/json",
    "client-platform": "web",
    "x-csrftoken": decodeURIComponent(document.cookie.match(/(?:^|;\s*)csrftoken=([^;]+)/)[1]),
  },
  body: JSON.stringify(request)
})
```

This model provides:

- **Zero credential persistence**: no passwords or API tokens are saved to disk.
- **Natural MFA / CAPTCHA bypass**: the user logs in once in the browser; the CLI reuses that live session.
- **HttpOnly cookie reuse**: browser security boundaries remain respected.

Each GraphQL request is one bro `javascript_tool` call with `awaitPromise`, so a
query costs a single MCP round trip. The in-page fetch aborts after 25 seconds,
before bro's 30-second tool-call timeout, so slow requests fail with context.
A tab that mon opens is polled until it has loaded a Monarch document before
any request runs, and is closed when the command finishes.

JSON output keeps Monarch numbers verbatim and emits object keys in sorted
order.

## Write Safety

Domain-specific reasoning remains outside this repository and consumes mon's
JSON commands as data sources. `mon transaction update` is the only built-in
domain mutation. It accepts one explicit transaction id and one exact category
name or id; it never searches, writes batches, creates rules, retries a
mutation, or falls back to another GraphQL document.

The command reads the transaction and the category list concurrently before
writing and verifies the transaction afterward.
Only an exact target-category read-back is success. Definitive payload or
schema rejection may return `failed`; ambiguous transport failures, malformed
mutation responses, conflicting observations, and failed verification return
`outcome-unknown`. Both exit non-zero.

Top-level GraphQL errors use a small shared typed representation. Standard
extension codes identify validation and bad-input failures. Monarch currently
omits extension codes for some document failures, so HTTP 400 GraphQL envelopes
from built-in operations are treated as adapter/schema incompatibility. Unknown
codes remain ambiguous.
