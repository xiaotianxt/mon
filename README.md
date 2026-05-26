# mon

mon is an AI-native command line tool for Monarch Money. It gives local agents
and scripts a small, stable interface for auth, structured GraphQL access,
account inspection, transaction search, and custom finance workflows.

The implementation is based on Monarch's web API shape documented by the
community Python project [`hammem/monarchmoney`](https://github.com/hammem/monarchmoney):

- login: `POST https://api.monarch.com/auth/login/`
- GraphQL: `POST https://api.monarch.com/graphql`
- auth header: `Authorization: Token <token>`

Monarch does not publish this as an official public API. Treat this tool as a
local personal automation client and expect occasional breakage when Monarch
changes its web app API.

## What It Does

- `mon auth login`: login with email/password, handle MFA, and save a local token.
- `mon auth token`: store an existing token without logging in.
- `mon accounts`: list accounts, or print raw JSON.
- `mon transactions`: search transactions by text/date with deterministic output.
- `mon gql`: run a checked-in or ad-hoc GraphQL document.
- `--browser`: run data commands through an already logged-in Monarch web app
  tab via OpenBrowserMCP instead of the saved token.
- `mon doctor`: verify local config and optional online connectivity.
- `mon install`: copy the current binary into `~/.local/bin`.

## Install

### Agent Skill

Install the optional Codex/agent skill for `mon` workflows:

```bash
npx -y github:xiaotianxt/skills mon
```

After the npm package is published:

```bash
npx -y @xiaotianxt/skills mon
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

## Auth

Interactive login:

```bash
mon auth login
```

`mon auth login` reuses a valid saved session by default. This is intentional:
Monarch rate-limits repeated password login attempts and may require CAPTCHA.
Use `mon auth login --force` only when you intentionally want to replace the
saved session.

Non-interactive password flow:

```bash
printf '%s' "$MONARCH_PASSWORD" | mon auth login --email you@example.com --password-stdin
```

If Monarch requires MFA, `mon` prompts for the MFA code from `/dev/tty`, so the
password can still come from stdin.

Store an existing token without putting it in shell history:

```bash
printf '%s' "$MONARCH_TOKEN" | mon auth token --token-stdin
```

The session is stored at `~/.mon/session.json` with `0600` permissions. Override
it with `MON_SESSION_FILE` or `--session-file`.

### Browser Session Fallback

When the saved Monarch token is missing or expired, `mon` can use the active
browser session instead:

```bash
mon auth status --browser --json
mon accounts --browser --json
mon transactions --browser --start-date 2026-04-17 --end-date 2026-05-17 --json
mon gql --browser --operation GetAccounts --query-file queries/accounts.graphql
```

Browser mode does not extract or save a Monarch token. It connects to the local
OpenBrowserMCP server, finds an existing `https://app.monarch.com/` tab, and
runs Monarch GraphQL from inside that page with the browser's normal cookie and
CSRF state. Open Monarch in Helium and log in before using `--browser`.

Useful browser options:

```bash
mon auth status --browser --browser-tab-id 1068097338 --json
mon accounts --browser --browser-id 545de677-bb20-4194-ac5f-c7073ac044e2 --json
```

`--browser-tab-id` is useful when multiple Monarch tabs are open.

## Usage

Show accounts:

```bash
mon accounts
mon accounts --json
mon accounts --browser --json
```

Search transactions:

```bash
mon transactions --search coffee --start-date 2026-01-01 --end-date 2026-04-30
mon transactions --search "payroll" --limit 200 --json
mon transactions --browser --start-date 2026-04-17 --end-date 2026-05-17 --json
```

Run an arbitrary GraphQL file:

```bash
mon gql \
  --operation GetAccounts \
  --query-file queries/accounts.graphql \
  --variables '{}'
```

## Environment Variables

- `MON_SESSION_FILE`: session file path. Defaults to `~/.mon/session.json`.
- `OPENBROWSERMCP_MCP_URL`: OpenBrowserMCP MCP endpoint. Defaults to
  `http://127.0.0.1:3500/mcp`.
- `OPENBROWSERMCP_SETTINGS`: OpenBrowserMCP settings file. Defaults to
  `~/openbrowsermcp/settings.json`.

## AI-Native Contract

mon is designed for agent use:

- every data command supports stable JSON output;
- secrets are stored locally and never printed except with explicit
  `mon auth login --no-save`;
- commands fail loudly with non-zero exits and contextual errors;
- password login is session-aware to reduce rate-limit pressure;
- browser mode reuses an already logged-in Helium session without copying
  cookies or Monarch tokens into `~/.mon/session.json`;
- HTTP 429 and CAPTCHA_REQUIRED responses are surfaced as first-class errors,
  not retried blindly.

## Development

```bash
make fmt
make check
cargo test
```

The project keeps dependencies small: `clap` for CLI parsing, blocking
`reqwest` for HTTP, and `serde_json` for raw GraphQL data.

## Release

Maintainers can release with:

```bash
scripts/release.sh
```

The script runs tests, pushes a tag, waits for GitHub Actions to publish the
`darwin-arm64` release artifact, updates `Formula/mon.rb` in
`xiaotianxt/homebrew-tap`, and verifies the Homebrew install.
