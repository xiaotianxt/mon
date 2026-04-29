use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use serde_json::Value;

use crate::cli::RentAppfolioArgs;
use crate::client::MonarchClient;
use crate::output;
use crate::paths;
use crate::queries;
use crate::session;

#[derive(Debug, Clone, Serialize)]
struct RentExport {
    source: String,
    search: String,
    start_date: Option<String>,
    end_date: Option<String>,
    exported_at: String,
    total_count: i64,
    matched_count: usize,
    transactions: Vec<RentTransaction>,
}

#[derive(Debug, Clone, Serialize)]
struct RentTransaction {
    id: String,
    date: String,
    amount: Option<f64>,
    merchant: String,
    account: String,
    category: String,
    notes: String,
    pending: bool,
}

pub fn run_appfolio(args: RentAppfolioArgs) -> Result<()> {
    let session_file = paths::session_file(args.session_file.clone())?;
    let stored = session::load(&session_file).with_context(|| {
        format!(
            "no usable session at {}; run `mon auth login`",
            session_file.display()
        )
    })?;
    let client = MonarchClient::new(Some(stored.token))?;

    let variables = queries::transaction_variables_from(
        0,
        args.limit,
        &args.search,
        args.start_date.as_deref(),
        args.end_date.as_deref(),
    )?;
    let data = client.graphql("GetTransactionsList", queries::TRANSACTIONS, variables)?;
    let export = build_export(
        &data,
        &args.search,
        args.start_date.clone(),
        args.end_date.clone(),
    );

    if args.write {
        let dir = paths::tracking_dir(args.tracking_dir.clone())?;
        write_export(&dir, &export)?;
        println!("wrote exports: {}", dir.display());
    }

    if args.json {
        output::print_json(&serde_json::to_value(export)?)?;
    } else {
        print_rent_table(&export);
    }
    Ok(())
}

fn build_export(
    data: &Value,
    search: &str,
    start_date: Option<String>,
    end_date: Option<String>,
) -> RentExport {
    let total_count = data["allTransactions"]["totalCount"].as_i64().unwrap_or(0);
    let transactions = data["allTransactions"]["results"]
        .as_array()
        .map(|items| items.iter().map(parse_transaction).collect::<Vec<_>>())
        .unwrap_or_default();

    RentExport {
        source: "monarch".to_owned(),
        search: search.to_owned(),
        start_date,
        end_date,
        exported_at: Utc::now().to_rfc3339(),
        total_count,
        matched_count: transactions.len(),
        transactions,
    }
}

fn parse_transaction(value: &Value) -> RentTransaction {
    RentTransaction {
        id: output::str_at(value, &["id"]),
        date: output::str_at(value, &["date"]),
        amount: output::number_at(value, &["amount"]),
        merchant: output::str_at(value, &["merchant", "name"])
            .or_else(|| output::str_at(value, &["plaidName"])),
        account: output::str_at(value, &["account", "displayName"]),
        category: output::str_at(value, &["category", "name"]),
        notes: output::str_at(value, &["notes"]),
        pending: value["pending"].as_bool().unwrap_or(false),
    }
}

fn write_export(dir: &Path, export: &RentExport) -> Result<()> {
    fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let stamp = Utc::now().format("%Y-%m-%d").to_string();
    let json_path = dir.join(format!("monarch-appfolio-transactions-{stamp}.json"));
    let csv_path = dir.join(format!("monarch-appfolio-transactions-{stamp}.csv"));

    fs::write(&json_path, serde_json::to_vec_pretty(export)?)
        .with_context(|| format!("failed to write {}", json_path.display()))?;

    let mut writer = csv::Writer::from_path(&csv_path)
        .with_context(|| format!("failed to write {}", csv_path.display()))?;
    for tx in &export.transactions {
        writer.serialize(tx)?;
    }
    writer.flush()?;
    Ok(())
}

fn print_rent_table(export: &RentExport) {
    println!(
        "matched: {} of {} total",
        export.matched_count, export.total_count
    );
    println!(
        "{:<12} {:>12} {:<28} {:<22} {}",
        "date", "amount", "merchant", "account", "category"
    );
    for tx in &export.transactions {
        println!(
            "{:<12} {:>12} {:<28} {:<22} {}",
            tx.date,
            tx.amount
                .map(|value| format!("{value:.2}"))
                .unwrap_or_else(|| "-".to_owned()),
            truncate(&tx.merchant, 28),
            truncate(&tx.account, 22),
            tx.category
        );
    }
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_owned();
    }
    let mut result = value
        .chars()
        .take(width.saturating_sub(3))
        .collect::<String>();
    result.push_str("...");
    result
}

trait EmptyToNone {
    fn or_else<F>(self, fallback: F) -> String
    where
        F: FnOnce() -> String;
}

impl EmptyToNone for String {
    fn or_else<F>(self, fallback: F) -> String
    where
        F: FnOnce() -> String,
    {
        if self.is_empty() {
            fallback()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rent_transactions() {
        let data = serde_json::json!({
            "allTransactions": {
                "totalCount": 1,
                "results": [{
                    "id": "tx1",
                    "date": "2026-03-30",
                    "amount": 1569.13,
                    "pending": false,
                    "plaidName": "APPFOLIO",
                    "notes": "",
                    "merchant": {"name": "AppFolio"},
                    "account": {"displayName": "Checking"},
                    "category": {"name": "Rent"}
                }]
            }
        });
        let export = build_export(&data, "appfolio", None, None);
        assert_eq!(export.matched_count, 1);
        assert_eq!(export.transactions[0].merchant, "AppFolio");
    }
}
