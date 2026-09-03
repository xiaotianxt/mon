---
name: mon
description: "Use when an agent needs to access Monarch Money through the local mon CLI: inspect accounts, list categories, search transactions, update one exact transaction category, or run GraphQL operations via the user's active browser session while keeping tokens and cookies private."
compatibility: "Requires mon 0.4.0 or newer, a running bro server on 127.0.0.1:3500, and an active Monarch tab in Chromium/Helium."
---

# mon

Use this skill when you need to access Monarch Money from the local `mon` CLI.

## Overview

`mon` operates directly through your logged-in Monarch Money browser session via `bro`.
It requires no passwords, API tokens, or saved session files. All commands execute
automatically through an open `https://app.monarch.com/` tab.

## Commands

- `mon --version`: show the installed mon version.
- `mon auth status`: check active browser session and subscription entitlement.
- `mon accounts`: fetch linked accounts in a compact table.
- `mon accounts --json`: fetch accounts as structured JSON.
- `mon categories`: list transaction categories and groups.
- `mon categories --json`: list categories as structured JSON.
- `mon transactions --search TEXT --start-date YYYY-MM-DD --end-date YYYY-MM-DD`: search transactions.
- `mon transactions --limit 50 --json`: fetch recent transactions as JSON.
- `mon transaction update TRANSACTION_ID --category NAME --dry-run --json`: preview one exact category update.
- `mon transaction update TRANSACTION_ID --category NAME --json`: apply one exact category update by name.
- `mon transaction update TRANSACTION_ID --category-id ID --json`: apply one exact category update by id.
- `mon gql --operation NAME --query-file FILE --variables '{}'`: run a custom GraphQL document.
- `mon doctor`: inspect local bro bridge and Monarch tab connectivity.

## Runtime Requirements

- `bro` server running on `127.0.0.1:3500` (token in `~/.bro/settings.json`).
- Chromium-family browser (Helium or Chrome) with the bro WebExtension connected.
- An open Monarch tab at `https://app.monarch.com/dashboard` (or any app page).
- If multiple Monarch tabs are open, optionally target one with `--browser-tab-id TAB_ID`.

## Safety

All commands execute locally through the active browser session. Do not print
the bro bearer token, browser cookies, or raw transaction details unless the
user explicitly asks for them; aggregate first for spending summaries.

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
