# Architecture

mon is a thin Rust client around Monarch Money's web API. The goal is not to
mirror every screen in Monarch; it is to expose a reliable local surface for
agents and scripts.

## Modules

- `cli`: typed clap commands.
- `client`: blocking HTTP login and GraphQL calls.
- `queries`: built-in GraphQL documents and variable builders.
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

Domain-specific workflows should live outside this repo and call the general
JSON commands (`mon transactions --json`, `mon accounts --json`, or
`mon gql --full`) as data sources.
