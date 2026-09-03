---
name: mon
description: "Use when an agent needs to access Monarch Money through the local mon CLI: authenticate, inspect accounts, search transactions, update one exact transaction category, run GraphQL operations, or use a logged-in browser session through bro while keeping tokens and cookies private."
compatibility: "Requires mon 0.3.0 or newer; browser mode also requires a running bro server and connected browser extension."
---

# mon

Use this skill when you need to access Monarch Money from the local `mon` CLI.

## Commands

- `mon --version`: show the installed mon version.
- `mon auth login`: login and save a local token.
- `mon auth status --online`: verify saved auth.
- `mon auth status --browser --json`: verify the logged-in Monarch browser session through bro.
- `mon accounts --json`: fetch linked accounts.
- `mon accounts --browser --json`: fetch accounts through an already logged-in Monarch browser tab.
- `mon categories --json`: list category ids, names, disabled state, and group metadata.
- `mon categories --browser --json`: list categories through the browser session.
- `mon transactions --search TEXT --start-date YYYY-MM-DD --end-date YYYY-MM-DD --json`: search transactions.
- `mon transactions --browser --start-date YYYY-MM-DD --end-date YYYY-MM-DD --json`: search transactions through the browser session.
- `mon transaction update TRANSACTION_ID --category NAME --dry-run --json`: preview one exact category update.
- `mon transaction update TRANSACTION_ID --category NAME --json`: apply one exact category update by name.
- `mon transaction update TRANSACTION_ID --category-id ID --json`: apply one exact category update by id.
- `mon gql --operation NAME --query-file FILE --variables '{}'`: run a custom GraphQL document.
- `mon gql --browser --operation NAME --query-file FILE --variables '{}'`: run GraphQL inside the Monarch web app tab.
- `mon doctor`: inspect local paths and auth state.

## Auth Strategy

- Use the saved token path for durable unattended CLI work: `mon auth status --online`, then `mon ... --json`.
- If the saved token is expired or password login risks CAPTCHA/rate limits, prefer `--browser` when a bro-connected browser is already logged in to `https://app.monarch.com/`.
- Browser mode reads the local bro bearer token from `~/.bro/settings.json`, connects to `http://127.0.0.1:3500/mcp`, finds a Monarch tab, and runs GraphQL from inside that page with browser cookies/CSRF.
- Do not try to print, scrape, or import a Monarch browser token into `~/.mon/session.json`; recent Monarch browser state may not expose a reusable API token.
- Use `--browser-tab-id TAB_ID` when multiple Monarch tabs are open.

## Binary Selection

Run `mon --version` before relying on newer commands. Version 0.3.0 introduced
`categories` and verified single-transaction category updates. If Homebrew is
behind, use `brew update && brew upgrade xiaotianxt/tap/mon`. When developing
from a checkout, build with `cargo build --release` and resolve
`target/release/mon` relative to this skill's repository instead of assuming a
user-specific absolute path.

## Safety

Do not print `~/.mon/session.json` or Monarch tokens. Prefer JSON command output
for agent workflows. Avoid repeated password login attempts; prefer the saved
session and `mon auth status --online`. When using `--browser`, do not print the
bro bearer token, browser cookies, localStorage values, or raw
transaction details unless the user explicitly asks for them; aggregate first
for spending summaries.

Treat `mon transaction update` as an explicit write. Prefer `--dry-run` first
and do not apply a change unless the user authorized the exact transaction and
target category. The command intentionally accepts one transaction id only; do
not build fuzzy, search-driven, rule-based, rent-specific, or implicit batch
updates around it. It never retries a mutation. `failed` and `outcome-unknown`
exit non-zero and must not be reported as success.

## References

For detailed command semantics, installation, and write outcomes, read
[README.md](README.md) when needed. Read [docs/architecture.md](docs/architecture.md)
only when developing, reviewing, or debugging mon itself.
