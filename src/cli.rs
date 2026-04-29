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
