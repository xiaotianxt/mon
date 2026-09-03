# Architecture

mon is a thin, browser-native Rust CLI for Monarch Money. Rather than storing
ephemeral API tokens or passwords that trigger CAPTCHAs, it delegates network
transport and authentication directly to an active Monarch Money browser tab via
[bro](https://github.com/xiaotianxt/bro).

## Modules

- `cli`: typed clap commands and arguments.
- `browser`: bro MCP connection, Monarch tab discovery, and in-page GraphQL execution.
- `graphql`: shared typed parsing and classification of top-level GraphQL errors.
- `queries`: built-in GraphQL documents and variable builders.
- `transaction_update`: exact category resolution, single-transaction mutation policy, payload validation, and read-back outcome classification.
- `paths`: home directory and tilde expansion.
- `output`: compact human tables and formatted JSON printing.
- `install`: copy the running binary to a local bin directory.

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

## Write Safety

Domain-specific reasoning remains outside this repository and consumes mon's
JSON commands as data sources. `mon transaction update` is the only built-in
domain mutation. It accepts one explicit transaction id and one exact category
name or id; it never searches, writes batches, creates rules, retries a
mutation, or falls back to another GraphQL document.

The command reads the transaction before writing and verifies it afterward.
Only an exact target-category read-back is success. Definitive payload or
schema rejection may return `failed`; ambiguous transport failures, malformed
mutation responses, conflicting observations, and failed verification return
`outcome-unknown`. Both exit non-zero.

Top-level GraphQL errors use a small shared typed representation. Standard
extension codes identify validation and bad-input failures. Monarch currently
omits extension codes for some document failures, so HTTP 400 GraphQL envelopes
from built-in operations are treated as adapter/schema incompatibility. Unknown
codes remain ambiguous.
