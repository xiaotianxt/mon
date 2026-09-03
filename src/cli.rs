use std::path::PathBuf;

use clap::Args;
use clap::Parser;
use clap::Subcommand;

#[derive(Debug, Parser)]
#[command(name = "mon")]
#[command(version)]
#[command(about = "AI-native Monarch Money CLI for structured local finance workflows")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect Monarch browser auth status.
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// List Monarch accounts.
    Accounts(JsonArgs),
    /// List Monarch transaction categories.
    Categories(JsonArgs),
    /// Search Monarch transactions.
    Transactions(TransactionArgs),
    /// Mutate one exact Monarch transaction.
    Transaction {
        #[command(subcommand)]
        command: TransactionCommand,
    },
    /// Run an arbitrary GraphQL document against Monarch.
    Gql(GqlArgs),
    /// Validate local config and Monarch browser connectivity.
    Doctor(DoctorArgs),
    /// Install mon into ~/.local/bin.
    Install(InstallArgs),
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Show Monarch browser auth status.
    Status(StatusArgs),
}

#[derive(Debug, Subcommand)]
pub enum TransactionCommand {
    /// Update the category of one exact transaction.
    Update(TransactionUpdateArgs),
}

#[derive(Debug, Clone, Args)]
pub struct JsonArgs {
    /// Print raw JSON instead of a compact table.
    #[arg(long)]
    pub json: bool,

    #[command(flatten)]
    pub browser: BrowserArgs,
}

#[derive(Debug, Clone, Args)]
pub struct StatusArgs {
    /// Print JSON.
    #[arg(long)]
    pub json: bool,

    #[command(flatten)]
    pub browser: BrowserArgs,
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
}

#[derive(Debug, Clone, Args)]
pub struct TransactionUpdateArgs {
    /// Exact transaction id to update.
    #[arg(value_name = "TRANSACTION_ID")]
    pub transaction_id: String,

    /// Exact category name after trimming, matched case-insensitively.
    #[arg(
        long,
        value_name = "NAME",
        required_unless_present = "category_id",
        conflicts_with = "category_id"
    )]
    pub category: Option<String>,

    /// Exact category id.
    #[arg(
        long,
        value_name = "ID",
        required_unless_present = "category",
        conflicts_with = "category"
    )]
    pub category_id: Option<String>,

    /// Resolve and inspect the transaction without mutating it.
    #[arg(long)]
    pub dry_run: bool,

    /// Print structured JSON.
    #[arg(long)]
    pub json: bool,

    #[command(flatten)]
    pub browser: BrowserArgs,
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
}

#[derive(Debug, Clone, Args)]
pub struct DoctorArgs {
    /// Print JSON.
    #[arg(long)]
    pub json: bool,

    #[command(flatten)]
    pub browser: BrowserArgs,
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
    /// Legacy compatibility flag; browser execution is now the default and only mode.
    #[arg(long, hide = true)]
    pub browser: bool,

    /// Explicit browser tab id to use.
    #[arg(long, value_name = "TAB_ID")]
    pub browser_tab_id: Option<u64>,

    /// Explicit bro browser id to use.
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

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn exposes_package_version() {
        let command = Cli::command();
        assert_eq!(command.get_version(), Some(env!("CARGO_PKG_VERSION")));
    }

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

    #[test]
    fn accepts_commands_without_browser_flag() {
        let cli = Cli::try_parse_from(["mon", "accounts", "--json"]).unwrap();
        let Command::Accounts(args) = cli.command else {
            panic!("expected accounts command");
        };
        assert!(args.json);
    }

    #[test]
    fn accepts_legacy_browser_flag_silently() {
        let cli = Cli::try_parse_from(["mon", "accounts", "--browser", "--json"]).unwrap();
        let Command::Accounts(args) = cli.command else {
            panic!("expected accounts command");
        };
        assert!(args.browser.browser);
        assert!(args.json);
    }

    #[test]
    fn preserves_transactions_search_cli() {
        let cli = Cli::try_parse_from([
            "mon",
            "transactions",
            "--search",
            "coffee",
            "--start-date",
            "2026-01-01",
            "--end-date",
            "2026-01-31",
            "--json",
        ])
        .unwrap();

        let Command::Transactions(args) = cli.command else {
            panic!("expected transactions command");
        };
        assert_eq!(args.search, "coffee");
        assert_eq!(args.start_date.as_deref(), Some("2026-01-01"));
        assert_eq!(args.end_date.as_deref(), Some("2026-01-31"));
        assert!(args.json);
        assert_eq!(args.limit, 100);
        assert_eq!(args.offset, 0);
    }

    #[test]
    fn parses_categories_command() {
        let cli = Cli::try_parse_from(["mon", "categories", "--json"]).unwrap();

        let Command::Categories(args) = cli.command else {
            panic!("expected categories command");
        };
        assert!(args.json);
    }

    #[test]
    fn parses_single_transaction_update() {
        let cli = Cli::try_parse_from([
            "mon",
            "transaction",
            "update",
            "tx-1",
            "--category",
            "Groceries",
            "--dry-run",
            "--json",
            "--browser-tab-id",
            "42",
        ])
        .unwrap();

        let Command::Transaction {
            command: TransactionCommand::Update(args),
        } = cli.command
        else {
            panic!("expected transaction update command");
        };

        assert_eq!(args.transaction_id, "tx-1");
        assert_eq!(args.category.as_deref(), Some("Groceries"));
        assert!(args.category_id.is_none());
        assert!(args.dry_run);
        assert!(args.json);
        assert_eq!(args.browser.browser_tab_id, Some(42));
    }

    #[test]
    fn transaction_update_requires_exactly_one_category_selector() {
        assert!(Cli::try_parse_from(["mon", "transaction", "update", "tx-1"]).is_err());
        assert!(Cli::try_parse_from([
            "mon",
            "transaction",
            "update",
            "tx-1",
            "--category",
            "Food",
            "--category-id",
            "cat-1",
        ])
        .is_err());
    }
}
