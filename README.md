# mon

mon is an AI-native command line tool for Monarch Money. It gives local agents
and scripts a fast, stable interface for account inspection, category review,
transaction search, single-transaction category updates, and custom GraphQL
access directly through your logged-in browser session.

Instead of storing API tokens or passwords that expire or risk CAPTCHA/MFA
lockouts, mon communicates with Monarch through your active Chromium/Helium
browser tab via [bro](https://github.com/xiaotianxt/bro). Your browser's
normal session cookies and CSRF protections are reused without copying credentials
to disk.

## What It Does

- `mon auth status`: verify active browser session and subscription entitlement.
- `mon accounts`: list accounts in a compact table or raw JSON.
- `mon categories`: list category ids, names, disabled state, and group metadata.
- `mon transactions`: search transactions by text and date with deterministic output.
- `mon transaction update`: update one exact transaction category with read-back verification.
- `mon gql`: run an arbitrary checked-in or ad-hoc GraphQL document.
- `mon doctor`: verify bro bridge connectivity and active Monarch tab status.
- `mon install`: copy the current binary into `~/.local/bin`.

## Install

### Agent Skill

Install the optional agent skill for `mon` workflows:

```bash
npx skills@latest add xiaotianxt/mon -y
```

Install to a specific agent harness (such as Pi, Codex, or OpenCode):

```bash
npx skills@latest add xiaotianxt/mon --agent pi -y
npx skills@latest add xiaotianxt/mon --agent codex -y
npx skills@latest add xiaotianxt/mon --agent opencode -y
```

### Homebrew

```bash
brew install xiaotianxt/tap/mon
```

Install the development build:

```bash
brew install --HEAD xiaotianxt/tap/mon
```

### From Source

Requires a Rust toolchain.

```bash
git clone https://github.com/xiaotianxt/mon.git
cd mon
make install-local
```

`make install-local` installs `mon` to `~/.local/bin/mon`. Make sure
`~/.local/bin` is in your `PATH`.

## How It Works

mon connects to the local `bro` MCP server (`http://127.0.0.1:3500/mcp`),
locates an open Monarch tab (`https://app.monarch.com/`), and executes GraphQL
queries and mutations inside that page using the browser's existing credentials
and CSRF tokens.

No API tokens or credentials are saved to `~/.mon`. To use mon:

1. Have `bro` running (`bro doctor` or `bro serve`).
2. Log in to `https://app.monarch.com/` in your browser (Chrome or Helium) with the bro extension connected.
3. Run `mon` commands directly from your terminal.

Useful browser targeting options (when multiple tabs or browser instances are open):

```bash
mon accounts --browser-tab-id 1068097338
mon accounts --browser-id 545de677-bb20-4194-ac5f-c7073ac044e2
```

## Usage

Show accounts:

```bash
mon accounts
mon accounts --json
```

Search transactions:

```bash
mon transactions --search coffee --start-date 2026-01-01 --end-date 2026-04-30
mon transactions --search "payroll" --limit 200 --json
```

List categories:

```bash
mon categories
mon categories --json
```

Preview a category change for one exact transaction:

```bash
mon transaction update TRANSACTION_ID --category "Groceries" --dry-run --json
```

Apply by exact category name or id:

```bash
mon transaction update TRANSACTION_ID --category "Groceries" --json
mon transaction update TRANSACTION_ID --category-id CATEGORY_ID --json
```

Category names are trimmed and matched exactly, case-insensitively. Missing,
ambiguous, and disabled categories are rejected; use `--category-id` to
resolve duplicate names. The update command never searches for transaction
ids, performs bulk updates, creates rules, retries a mutation, or falls back to
another GraphQL document.

A write succeeds only when read-back finds the requested category. Definitive
payload or schema rejection returns `failed`. Network failures, timeouts,
malformed mutation responses, conflicting observations, and failed read-back
return `outcome-unknown` when the final state cannot be proven. Both states exit
non-zero.

Update JSON contains `status`, `changed`, `wouldChange`, `verified`,
`transactionId`, `beforeCategory`, and `afterCategory`. `changed` is `null` when
the final state is unknown. During a dry run, `afterCategory` is the proposed
target and `verified` is false.

Run an arbitrary GraphQL file:

```bash
mon gql \
  --operation GetAccounts \
  --query-file queries/accounts.graphql \
  --variables '{}'
```

## Environment Variables

- `BRO_MCP_URL`: bro MCP endpoint. Defaults to `http://127.0.0.1:3500/mcp`.
- `BRO_SETTINGS`: bro settings file. Defaults to `~/.bro/settings.json`.

The former `OPENBROWSERMCP_MCP_URL` and `OPENBROWSERMCP_SETTINGS` names remain
accepted as compatibility aliases.

## AI-Native Contract

mon is designed for agent use:

- every data command supports stable JSON output;
- no user credentials, passwords, or tokens are managed or saved;
- execution directly reuses the user's active browser session via bro;
- commands fail loudly with non-zero exits and contextual errors;
- the only built-in domain write targets one explicit transaction and category;
- write mutations are never retried automatically and success requires exact read-back verification.

## Development

```bash
make fmt
make check
cargo test
```

## Release

Maintainers can release with:

```bash
scripts/release.sh
```

The script runs tests, pushes a tag, waits for GitHub Actions to publish the
`darwin-arm64` release artifact, updates `Formula/mon.rb` in
`xiaotianxt/homebrew-tap`, and verifies the Homebrew install.
