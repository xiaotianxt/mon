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
- `mon doctor`: verify local config and optional online connectivity.
- `mon install`: copy the current binary into `~/.local/bin`.

## Install

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

Run an arbitrary GraphQL file:

```bash
mon gql \
  --operation GetAccounts \
  --query-file queries/accounts.graphql \
  --variables '{}'
```

## Environment Variables

- `MON_SESSION_FILE`: session file path. Defaults to `~/.mon/session.json`.

## AI-Native Contract

mon is designed for agent use:

- every data command supports stable JSON output;
- secrets are stored locally and never printed except with explicit
  `mon auth login --no-save`;
- commands fail loudly with non-zero exits and contextual errors;
- password login is session-aware to reduce rate-limit pressure;
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
