# Architecture

mon is a thin Rust client around Monarch Money's web API. The goal is not to
mirror every screen in Monarch; it is to expose a reliable local surface for
agents and scripts.

## Modules

- `cli`: typed clap commands.
- `client`: blocking HTTP login and GraphQL calls.
- `queries`: built-in GraphQL documents and variable builders.
- `session`: local token storage in `~/.mon/session.json`.
- `paths`: home, session, and rent-tracking path resolution.
- `output`: compact human tables and JSON printing.
- `rent`: AppFolio-oriented transaction export.
- `install`: copy the running binary to a local bin directory.

## Monarch API Surface

The client uses the API conventions observed in the community
`hammem/monarchmoney` package:

```text
POST https://api.monarchmoney.com/auth/login/
POST https://api.monarchmoney.com/graphql
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

## Rent Workflow

`mon rent appfolio` searches Monarch transactions with `search=appfolio`,
normalizes a small row schema, and can write:

```text
~/Desktop/rent-tracking/
  monarch-appfolio-transactions-YYYY-MM-DD.json
  monarch-appfolio-transactions-YYYY-MM-DD.csv
```

This intentionally stays separate from settlement math. The rent folder should
hold facts first: portal ledger exports, Monarch payment exports, and later the
calculation state that reconciles those facts.
