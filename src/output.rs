use anyhow::Result;
use serde_json::Value;

use crate::transaction_update::Category;
use crate::transaction_update::CategoryRef;
use crate::transaction_update::UpdateOutcome;

pub fn print_json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn print_accounts(data: &Value, json: bool) -> Result<()> {
    if json {
        return print_json(data);
    }

    let accounts = data["accounts"].as_array().cloned().unwrap_or_default();
    println!(
        "{:<28} {:>14} {:<18} institution",
        "name", "balance", "type"
    );
    for account in accounts {
        let name = str_at(&account, &["displayName"]);
        let balance = number_at(&account, &["displayBalance"])
            .or_else(|| number_at(&account, &["currentBalance"]));
        let kind = str_at(&account, &["type", "display"]);
        let institution = str_at(&account, &["institution", "name"]);
        println!(
            "{:<28} {:>14} {:<18} {}",
            truncate(&name, 28),
            balance
                .map(|value| format!("{value:.2}"))
                .unwrap_or_else(|| "-".to_owned()),
            truncate(&kind, 18),
            institution
        );
    }
    Ok(())
}

pub fn print_transactions(data: &Value, json: bool) -> Result<()> {
    if json {
        return print_json(data);
    }

    let results = data["allTransactions"]["results"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let total = data["allTransactions"]["totalCount"].as_i64().unwrap_or(0);
    println!("total: {total}");
    println!(
        "{:<12} {:>12} {:<28} {:<22} category",
        "date", "amount", "merchant", "account"
    );
    for tx in results {
        let date = str_at(&tx, &["date"]);
        let amount = number_at(&tx, &["amount"]);
        let merchant = first_nonempty(
            str_at(&tx, &["merchant", "name"]),
            str_at(&tx, &["plaidName"]),
        );
        let account = str_at(&tx, &["account", "displayName"]);
        let category = str_at(&tx, &["category", "name"]);
        println!(
            "{:<12} {:>12} {:<28} {:<22} {}",
            truncate(&date, 12),
            amount
                .map(|value| format!("{value:.2}"))
                .unwrap_or_else(|| "-".to_owned()),
            truncate(&merchant, 28),
            truncate(&account, 22),
            category
        );
    }
    Ok(())
}

pub fn print_categories(categories: &[Category], json: bool) -> Result<()> {
    if json {
        return print_json(&serde_json::json!({ "categories": categories }));
    }

    println!(
        "{:<20} {:<8} {:<10} {:<20} name",
        "id", "disabled", "type", "group"
    );
    for category in categories {
        println!(
            "{:<20} {:<8} {:<10} {:<20} {}",
            truncate(&category.id, 20),
            category.is_disabled,
            truncate(&category.group.kind, 10),
            truncate(&category.group.name, 20),
            category.name
        );
    }
    Ok(())
}

pub fn print_transaction_update(outcome: &UpdateOutcome, json: bool) -> Result<()> {
    if json {
        return print_json(&serde_json::to_value(outcome)?);
    }

    let changed = outcome
        .changed
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_owned());

    println!(
        "{}: transaction {}: {} -> {} (changed: {}, would change: {}, verified: {})",
        outcome.status.as_str(),
        outcome.transaction_id,
        format_category(outcome.before_category.as_ref()),
        format_category(outcome.after_category.as_ref()),
        changed,
        outcome.would_change,
        outcome.verified,
    );
    Ok(())
}

pub fn str_at(value: &Value, path: &[&str]) -> String {
    let mut current = value;
    for segment in path {
        current = &current[*segment];
    }
    current.as_str().unwrap_or("").to_owned()
}

pub fn number_at(value: &Value, path: &[&str]) -> Option<f64> {
    let mut current = value;
    for segment in path {
        current = &current[*segment];
    }
    current.as_f64()
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

fn format_category(category: Option<&CategoryRef>) -> String {
    match category {
        Some(category) => {
            let name = serde_json::to_string(&category.name)
                .unwrap_or_else(|_| "\"<invalid>\"".to_owned());
            format!("{name} ({})", category.id)
        }
        None => "<none>".to_owned(),
    }
}

fn first_nonempty(primary: String, fallback: String) -> String {
    if primary.is_empty() {
        fallback
    } else {
        primary
    }
}
