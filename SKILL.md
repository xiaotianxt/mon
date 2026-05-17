# mon

Use this skill when you need to access Monarch Money from the local `mon` CLI.

## Commands

- `mon auth login`: login and save a local token.
- `mon auth status --online`: verify saved auth.
- `mon auth status --browser --json`: verify the logged-in Helium/Monarch browser session through OpenBrowserMCP.
- `mon accounts --json`: fetch linked accounts.
- `mon accounts --browser --json`: fetch accounts through an already logged-in Monarch browser tab.
- `mon transactions --search TEXT --start-date YYYY-MM-DD --end-date YYYY-MM-DD --json`: search transactions.
- `mon transactions --browser --start-date YYYY-MM-DD --end-date YYYY-MM-DD --json`: search transactions through the browser session.
- `mon gql --operation NAME --query-file FILE --variables '{}'`: run a custom GraphQL document.
- `mon gql --browser --operation NAME --query-file FILE --variables '{}'`: run GraphQL inside the Monarch web app tab.
- `mon doctor`: inspect local paths and auth state.

## Auth Strategy

- Use the saved token path for durable unattended CLI work: `mon auth status --online`, then `mon ... --json`.
- If the saved token is expired or password login risks CAPTCHA/rate limits, prefer `--browser` when Helium is already logged in to `https://app.monarch.com/`.
- Browser mode reads the local OpenBrowserMCP bearer token from `~/openbrowsermcp/settings.json`, connects to `http://127.0.0.1:3500/mcp`, finds a Monarch tab, and runs GraphQL from inside that page with browser cookies/CSRF.
- Do not try to print, scrape, or import a Monarch browser token into `~/.mon/session.json`; recent Monarch browser state may not expose a reusable API token.
- Use `--browser-tab-id TAB_ID` when multiple Monarch tabs are open.

## Binary Selection

The Homebrew `mon` may lag local source changes. If `mon transactions --help`
does not show `--browser`, use `/Users/yupeit/dev/mon/target/release/mon` for
browser-mode work, or release/upgrade the Homebrew formula before relying on the
bare `mon` command.

## Safety

Do not print `~/.mon/session.json` or Monarch tokens. Prefer JSON command output
for agent workflows. Avoid repeated password login attempts; prefer the saved
session and `mon auth status --online`. When using `--browser`, do not print the
OpenBrowserMCP bearer token, browser cookies, localStorage values, or raw
transaction details unless the user explicitly asks for them; aggregate first
for spending summaries.
