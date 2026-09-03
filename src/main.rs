mod browser;
mod cli;
mod graphql;
mod install;
mod output;
mod paths;
mod queries;
mod transaction_update;

use std::process::ExitCode;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use cli::AuthCommand;
use cli::BrowserArgs;
use cli::Cli;
use cli::Command;
use cli::TransactionCommand;

fn main() -> ExitCode {
    match entry() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("mon: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn entry() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Auth { command } => match command {
            AuthCommand::Status(args) => auth_status(args)?,
        },
        Command::Accounts(args) => {
            let data = graphql(
                &args.browser,
                "GetAccounts",
                queries::ACCOUNTS,
                serde_json::json!({}),
                false,
            )?;
            output::print_accounts(&data, args.json)?;
        }
        Command::Categories(args) => {
            let data = graphql(
                &args.browser,
                "GetCategories",
                queries::CATEGORIES,
                serde_json::json!({}),
                false,
            )?;
            let categories = transaction_update::parse_categories(&data)?;
            output::print_categories(&categories, args.json)?;
        }
        Command::Transactions(args) => {
            let variables = queries::transaction_variables(&args)?;
            let data = graphql(
                &args.browser,
                "GetTransactionsList",
                queries::TRANSACTIONS,
                variables,
                false,
            )?;
            output::print_transactions(&data, args.json)?;
        }
        Command::Transaction { command } => match command {
            TransactionCommand::Update(args) => update_transaction(args)?,
        },
        Command::Gql(args) => {
            let query = std::fs::read_to_string(&args.query_file)
                .with_context(|| format!("failed to read {}", args.query_file.display()))?;
            let variables = match args.variables {
                Some(raw) => serde_json::from_str(&raw).context("--variables must be JSON")?,
                None => serde_json::json!({}),
            };
            let value = graphql(&args.browser, &args.operation, &query, variables, args.full)?;
            output::print_json(&value)?;
        }
        Command::Doctor(args) => doctor(args)?,
        Command::Install(args) => {
            install::install(args).context("failed to install mon")?;
        }
    }

    Ok(())
}

fn browser_options(args: &BrowserArgs) -> browser::BrowserOptions {
    browser::BrowserOptions {
        tab_id: args.browser_tab_id,
        browser_id: args.browser_id.clone(),
        mcp_url: args.bro_mcp_url.clone(),
        settings_file: args.bro_settings.clone(),
    }
}

fn browser_client(args: &BrowserArgs) -> Result<browser::BrowserMonarchClient> {
    browser::BrowserMonarchClient::connect(browser_options(args))
}

fn graphql(
    browser_args: &BrowserArgs,
    operation: &str,
    query: &str,
    variables: serde_json::Value,
    full: bool,
) -> Result<serde_json::Value> {
    let client = browser_client(browser_args)?;
    client.graphql_full_or_data(operation, query, variables, full)
}

fn update_transaction(args: cli::TransactionUpdateArgs) -> Result<()> {
    let client = browser_client(&args.browser)?;
    let execution = transaction_update::execute(&args, |operation, query, variables| {
        client.graphql(operation, query, variables)
    })?;

    output::print_transaction_update(&execution.outcome, args.json)?;

    if let Some(failure) = execution.failure {
        return Err(failure);
    }

    Ok(())
}

fn auth_status(args: cli::StatusArgs) -> Result<()> {
    let client = browser_client(&args.browser)?;
    let data = client.graphql(
        "GetSubscriptionDetails",
        queries::SUBSCRIPTION,
        serde_json::json!({}),
    )?;

    let status = serde_json::json!({
        "online": true,
        "tabId": client.tab_id(),
        "browserId": client.browser_id(),
        "subscription": data["subscription"].clone(),
    });

    if args.json {
        output::print_json(&status)?;
    } else {
        println!("auth: browser session active");
        println!("tab: {}", client.tab_id());
        if let Some(bid) = client.browser_id() {
            println!("browser: {bid}");
        }
        println!("subscription: ok");
    }
    Ok(())
}

fn doctor(args: cli::DoctorArgs) -> Result<()> {
    let client = browser_client(&args.browser)?;
    let data = client.graphql(
        "GetSubscriptionDetails",
        queries::SUBSCRIPTION,
        serde_json::json!({}),
    )?;

    let report = serde_json::json!({
        "broConnected": true,
        "tabId": client.tab_id(),
        "browserId": client.browser_id(),
        "monarchConnected": true,
        "subscription": data["subscription"].clone(),
    });

    if args.json {
        output::print_json(&report)?;
    } else {
        println!("bro: connected");
        println!("monarch tab: {} (active)", client.tab_id());
        if let Some(bid) = client.browser_id() {
            println!("browser instance: {bid}");
        }
        println!("auth: active browser session");
    }
    Ok(())
}
