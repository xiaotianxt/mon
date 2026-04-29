# mon

Use this skill when you need to access Monarch Money from the local `mon` CLI.

## Commands

- `mon auth login`: login and save a local token.
- `mon auth status --online`: verify saved auth.
- `mon accounts --json`: fetch linked accounts.
- `mon transactions --search TEXT --start-date YYYY-MM-DD --end-date YYYY-MM-DD --json`: search transactions.
- `mon gql --operation NAME --query-file FILE --variables '{}'`: run a custom GraphQL document.
- `mon doctor`: inspect local paths and auth state.

## Safety

Do not print `~/.mon/session.json` or Monarch tokens. Prefer JSON command output
for agent workflows. Avoid repeated password login attempts; prefer the saved
session and `mon auth status --online`.
