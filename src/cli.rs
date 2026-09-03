use std::path::PathBuf;

use clap::Args;
use clap::Parser;
use clap::Subcommand;

#[derive(Debug, Parser)]
#[command(name = "mon")]
#[command(about = "AI-native Monarch Money CLI for structured local finance workflows")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage Monarch auth and local session state.
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// List Monarch accounts.
    Accounts(JsonSessionArgs),
    /// Search Monarch transactions.
    Transactions(TransactionArgs),
    /// Run an arbitrary GraphQL document against Monarch.
    Gql(GqlArgs),
    /// Validate local config and optional API connectivity.
    Doctor(DoctorArgs),
    /// Install mon into ~/.local/bin.
    Install(InstallArgs),
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Login with email/password and save the returned session token.
    Login(LoginArgs),
    /// Save an existing Monarch token without logging in.
    Token(TokenArgs),
    /// Show local auth status.
    Status(StatusArgs),
    /// Remove the saved session token.
    Logout(LogoutArgs),
}

#[derive(Debug, Clone, Args)]
pub struct JsonSessionArgs {
    /// Print raw JSON instead of a compact table.
    #[arg(long)]
    pub json: bool,

    #[command(flatten)]
    pub browser: BrowserArgs,

    /// Session file. Defaults to $MON_SESSION_FILE or ~/.mon/session.json.
    #[arg(long)]
    pub session_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct LoginArgs {
    /// Monarch account email. Prompted when omitted.
    #[arg(long)]
    pub email: Option<String>,

    /// Read password from stdin instead of prompting.
    #[arg(long)]
    pub password_stdin: bool,

    /// MFA code to send during login. Prompted when Monarch requires MFA.
    #[arg(long)]
    pub mfa_code: Option<String>,

    /// Re-authenticate even when the saved session is still valid.
    #[arg(long)]
    pub force: bool,

    /// Print the token instead of saving it.
    #[arg(long)]
    pub no_save: bool,

    /// Session file. Defaults to $MON_SESSION_FILE or ~/.mon/session.json.
    #[arg(long)]
    pub session_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct TokenArgs {
    /// Token value. Prefer --token-stdin to avoid shell history.
    #[arg(long)]
    pub token: Option<String>,

    /// Read token from stdin.
    #[arg(long)]
    pub token_stdin: bool,

    /// Session file. Defaults to $MON_SESSION_FILE or ~/.mon/session.json.
    #[arg(long)]
    pub session_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct StatusArgs {
    /// Verify the token with a lightweight Monarch API request.
    #[arg(long)]
    pub online: bool,

    /// Print JSON.
    #[arg(long)]
    pub json: bool,

    #[command(flatten)]
    pub browser: BrowserArgs,

    /// Session file. Defaults to $MON_SESSION_FILE or ~/.mon/session.json.
    #[arg(long)]
    pub session_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct LogoutArgs {
    /// Session file. Defaults to $MON_SESSION_FILE or ~/.mon/session.json.
    #[arg(long)]
    pub session_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct TransactionArgs {
    /// Earliest transaction date, YYYY-MM-DD. Must be paired with --end-date.
    #[arg(long)]
    pub start_date: Option<String>,

    /// Latest transaction date, YYYY-MM-DD. Must be paired with --start-date.
    #[arg(long)]
    pub end_date: Option<String>,

    /// Monarch transaction search text.
    #[arg(long, default_value = "")]
    pub search: String,

    /// Maximum rows returned by Monarch.
    #[arg(long, default_value_t = 100)]
    pub limit: u32,

    /// Offset for pagination.
    #[arg(long, default_value_t = 0)]
    pub offset: u32,

    /// Print raw JSON instead of a compact table.
    #[arg(long)]
    pub json: bool,

    #[command(flatten)]
    pub browser: BrowserArgs,

    /// Session file. Defaults to $MON_SESSION_FILE or ~/.mon/session.json.
    #[arg(long)]
    pub session_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct GqlArgs {
    /// GraphQL operation name.
    #[arg(long)]
    pub operation: String,

    /// File containing a GraphQL query or mutation.
    #[arg(long)]
    pub query_file: PathBuf,

    /// JSON variables object.
    #[arg(long)]
    pub variables: Option<String>,

    /// Print the full GraphQL response instead of just data.
    #[arg(long)]
    pub full: bool,

    #[command(flatten)]
    pub browser: BrowserArgs,

    /// Session file. Defaults to $MON_SESSION_FILE or ~/.mon/session.json.
    #[arg(long)]
    pub session_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct DoctorArgs {
    /// Verify the token with a lightweight Monarch API request.
    #[arg(long)]
    pub online: bool,

    /// Print JSON.
    #[arg(long)]
    pub json: bool,

    #[command(flatten)]
    pub browser: BrowserArgs,

    /// Session file. Defaults to $MON_SESSION_FILE or ~/.mon/session.json.
    #[arg(long)]
    pub session_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct InstallArgs {
    /// Directory to install mon into.
    #[arg(long)]
    pub bin_dir: Option<PathBuf>,

    /// Replace an existing mon binary.
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Clone, Default, Args)]
pub struct BrowserArgs {
    /// Use a logged-in Monarch web app tab through bro instead of the saved token.
    #[arg(long)]
    pub browser: bool,

    /// Explicit browser tab id to use with --browser.
    #[arg(long, value_name = "TAB_ID")]
    pub browser_tab_id: Option<u64>,

    /// Explicit bro browser id to use with --browser.
    #[arg(long, value_name = "BROWSER_ID")]
    pub browser_id: Option<String>,

    /// bro MCP endpoint. Defaults to BRO_MCP_URL or http://127.0.0.1:3500/mcp.
    #[arg(
        long = "bro-mcp-url",
        alias = "openbrowser-mcp-url",
        value_name = "URL"
    )]
    pub bro_mcp_url: Option<String>,

    /// bro settings file. Defaults to BRO_SETTINGS or ~/.bro/settings.json.
    #[arg(
        long = "bro-settings",
        alias = "openbrowser-settings",
        value_name = "PATH"
    )]
    pub bro_settings: Option<PathBuf>,
}

impl BrowserArgs {
    pub fn enabled(&self) -> bool {
        self.browser
            || self.browser_tab_id.is_some()
            || self.browser_id.is_some()
            || self.bro_mcp_url.is_some()
            || self.bro_settings.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_legacy_openbrowser_flag_aliases() {
        let cli = Cli::try_parse_from([
            "mon",
            "accounts",
            "--openbrowser-mcp-url",
            "http://127.0.0.1:3500/mcp",
            "--openbrowser-settings",
            "/tmp/settings.json",
        ])
        .unwrap();

        let Command::Accounts(args) = cli.command else {
            panic!("expected accounts command");
        };
        assert_eq!(
            args.browser.bro_mcp_url.as_deref(),
            Some("http://127.0.0.1:3500/mcp")
        );
        assert_eq!(
            args.browser.bro_settings.as_deref(),
            Some(std::path::Path::new("/tmp/settings.json"))
        );
    }
}
