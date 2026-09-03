# Architecture

mon is a thin Rust client around Monarch Money's web API. The goal is not to
mirror every screen in Monarch; it is to expose a reliable local surface for
agents and scripts.

## Modules

- `cli`: typed clap commands.
- `client`: blocking HTTP login and GraphQL calls.
- `browser`: fixed-tab browser-session GraphQL transport through bro.
- `graphql`: shared typed parsing and classification of top-level GraphQL errors.
- `queries`: built-in GraphQL documents and variable builders.
- `transaction_update`: exact category resolution, single-transaction mutation policy, payload validation, and read-back outcome classification.
- `session`: local token storage in `~/.mon/session.json`.
- `paths`: home and session path resolution.
- `output`: compact human tables and JSON printing.
- `install`: copy the running binary to a local bin directory.

## Monarch API Surface

The client uses the API conventions observed in the community
`hammem/monarchmoney` package:

```text
POST https://api.monarch.com/auth/login/
POST https://api.monarch.com/graphql
Authorization: Token <token>
Client-Platform: web
```

GraphQL calls use this request shape:

```json
{
  "operationName": "GetTransactionsList",
  "variables": {},
  "query": "query GetTransactionsList { ... }"
}
```

The CLI prints the `data` object by default for GraphQL operations and can print
the full response with `mon gql --full`.

## Session Model

Tokens are stored as JSON:

```json
{
  "token": "...",
  "created_at": "2026-04-29T00:00:00Z"
}
```

The file defaults to `~/.mon/session.json`, can be overridden with
`MON_SESSION_FILE`, and is chmodded to `0600` on Unix.

## Rate Limits

Monarch rate-limits repeated password login attempts and may require CAPTCHA.
`mon` is deliberately conservative:

- saved sessions are reused by `mon auth login` unless `--force` is passed;
- HTTP redirects are not followed automatically, so API host changes fail
  clearly instead of mutating POST into GET;
- HTTP 429 responses are surfaced as explicit rate-limit errors;
- `CAPTCHA_REQUIRED` stops automated login attempts instead of retrying.

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
codes remain ambiguous. Both saved-token and browser transports use the same
parser.
