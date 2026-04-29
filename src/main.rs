mod cli;
mod client;
mod install;
mod output;
mod paths;
mod queries;
mod rent;
mod session;

use std::io::Read;
use std::process::ExitCode;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use cli::AuthCommand;
use cli::Cli;
use cli::Command;
use cli::RentCommand;
use client::LoginResult;

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
            AuthCommand::Login(args) => auth_login(args)?,
            AuthCommand::Token(args) => auth_token(args)?,
            AuthCommand::Status(args) => auth_status(args)?,
            AuthCommand::Logout(args) => auth_logout(args)?,
        },
        Command::Accounts(args) => {
            let client = client_from_session(args.session_file)?;
            let data = client.graphql("GetAccounts", queries::ACCOUNTS, serde_json::json!({}))?;
            output::print_accounts(&data, args.json)?;
        }
        Command::Transactions(args) => {
            let client = client_from_session(args.session_file.clone())?;
            let variables = queries::transaction_variables(&args)?;
            let data = client.graphql("GetTransactionsList", queries::TRANSACTIONS, variables)?;
            output::print_transactions(&data, args.json)?;
        }
        Command::Gql(args) => {
            let client = client_from_session(args.session_file)?;
            let query = std::fs::read_to_string(&args.query_file)
                .with_context(|| format!("failed to read {}", args.query_file.display()))?;
            let variables = match args.variables {
                Some(raw) => serde_json::from_str(&raw).context("--variables must be JSON")?,
                None => serde_json::json!({}),
            };
            let value =
                client.graphql_full_or_data(&args.operation, &query, variables, args.full)?;
            output::print_json(&value)?;
        }
        Command::Rent { command } => match command {
            RentCommand::Appfolio(args) => rent::run_appfolio(args)?,
        },
        Command::Doctor(args) => doctor(args)?,
        Command::Install(args) => {
            install::install(args).context("failed to install mon")?;
        }
    }

    Ok(())
}

fn client_from_session(path: Option<std::path::PathBuf>) -> Result<client::MonarchClient> {
    let path = paths::session_file(path)?;
    let stored = session::load(&path).with_context(|| {
        format!(
            "no usable session at {}; run `mon auth login`",
            path.display()
        )
    })?;
    Ok(client::MonarchClient::new(Some(stored.token))?)
}

fn auth_login(args: cli::LoginArgs) -> Result<()> {
    let email = match args.email {
        Some(email) => email,
        None => prompt("Email: ")?,
    };
    let password = if args.password_stdin {
        read_stdin_secret("password from stdin")?
    } else {
        rpassword::prompt_password("Password: ").context("failed to read password")?
    };

    let client = client::MonarchClient::new(None)?;
    let token = match client.login(&email, &password, args.mfa_code.as_deref())? {
        LoginResult::Token(token) => token,
        LoginResult::MfaRequired => {
            let code = prompt("MFA code: ")?;
            if code.trim().is_empty() {
                anyhow::bail!("MFA code is required");
            }
            match client.login(&email, &password, Some(code.trim()))? {
                LoginResult::Token(token) => token,
                LoginResult::MfaRequired => anyhow::bail!("MFA is still required"),
            }
        }
    };

    if args.no_save {
        println!("{token}");
        return Ok(());
    }

    let path = paths::session_file(args.session_file)?;
    session::save(&path, &token)?;
    println!("saved session: {}", path.display());
    Ok(())
}

fn auth_token(args: cli::TokenArgs) -> Result<()> {
    let token = if args.token_stdin {
        read_stdin_secret("token from stdin")?
    } else if let Some(token) = args.token {
        token
    } else {
        rpassword::prompt_password("Monarch token: ").context("failed to read token")?
    };

    let token = token.trim();
    if token.is_empty() {
        anyhow::bail!("empty token");
    }

    let path = paths::session_file(args.session_file)?;
    session::save(&path, token)?;
    println!("saved session: {}", path.display());
    Ok(())
}

fn auth_status(args: cli::StatusArgs) -> Result<()> {
    let path = paths::session_file(args.session_file)?;
    let loaded = session::load(&path).ok();
    let has_token = loaded.is_some();
    let mut status = serde_json::json!({
        "sessionFile": path,
        "hasToken": has_token,
    });

    if args.online {
        let stored = loaded
            .clone()
            .context("no saved session; run `mon auth login`")?;
        let client = client::MonarchClient::new(Some(stored.token))?;
        let data = client.graphql(
            "GetSubscriptionDetails",
            queries::SUBSCRIPTION,
            serde_json::json!({}),
        )?;
        status["online"] = serde_json::json!(true);
        status["subscription"] = data["subscription"].clone();
    }

    if args.json {
        output::print_json(&status)?;
    } else {
        println!("session: {}", status["sessionFile"].as_str().unwrap_or(""));
        println!("token: {}", if has_token { "present" } else { "missing" });
        if args.online {
            println!("online: ok");
        }
    }
    Ok(())
}

fn auth_logout(args: cli::LogoutArgs) -> Result<()> {
    let path = paths::session_file(args.session_file)?;
    if path.exists() {
        std::fs::remove_file(&path)
            .with_context(|| format!("failed to remove {}", path.display()))?;
    }
    println!("removed session: {}", path.display());
    Ok(())
}

fn doctor(args: cli::DoctorArgs) -> Result<()> {
    let session_file = paths::session_file(args.session_file)?;
    let config_dir = paths::config_dir()?;
    let mut report = serde_json::json!({
        "configDir": config_dir,
        "sessionFile": session_file,
        "sessionExists": session_file.exists(),
        "monarchApi": client::MonarchClient::base_url(),
    });

    if args.online {
        let stored =
            session::load(&session_file).context("no saved session; run `mon auth login`")?;
        let client = client::MonarchClient::new(Some(stored.token))?;
        let data = client.graphql(
            "GetSubscriptionDetails",
            queries::SUBSCRIPTION,
            serde_json::json!({}),
        )?;
        report["online"] = serde_json::json!(true);
        report["subscription"] = data["subscription"].clone();
    }

    if args.json {
        output::print_json(&report)?;
    } else {
        println!("config: {}", report["configDir"].as_str().unwrap_or(""));
        println!("session: {}", report["sessionFile"].as_str().unwrap_or(""));
        println!(
            "session token: {}",
            if report["sessionExists"].as_bool().unwrap_or(false) {
                "present"
            } else {
                "missing"
            }
        );
        println!("api: {}", client::MonarchClient::base_url());
        if args.online {
            println!("online: ok");
        }
    }
    Ok(())
}

fn prompt(label: &str) -> Result<String> {
    use std::fs::OpenOptions;
    use std::io::BufRead;
    use std::io::BufReader;
    use std::io::Write;

    if let Ok(mut tty) = OpenOptions::new().read(true).write(true).open("/dev/tty") {
        write!(tty, "{label}").context("failed to write prompt")?;
        tty.flush().context("failed to flush prompt")?;

        let mut value = String::new();
        BufReader::new(tty)
            .read_line(&mut value)
            .context("failed to read /dev/tty")?;
        return Ok(value.trim().to_owned());
    }

    print!("{label}");
    std::io::stdout()
        .flush()
        .context("failed to flush stdout")?;
    let mut value = String::new();
    std::io::stdin()
        .read_line(&mut value)
        .context("failed to read stdin")?;
    Ok(value.trim().to_owned())
}

fn read_stdin_secret(name: &str) -> Result<String> {
    let mut value = String::new();
    std::io::stdin()
        .read_to_string(&mut value)
        .with_context(|| format!("failed to read {name}"))?;
    Ok(value.trim().to_owned())
}
