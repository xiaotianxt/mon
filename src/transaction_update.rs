use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::cli::TransactionUpdateArgs;
use crate::graphql::GraphqlErrorClass;
use crate::graphql::GraphqlResponseError;
use crate::queries;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryGroup {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    pub is_disabled: bool,
    pub group: CategoryGroup,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateStatus {
    Updated,
    Unchanged,
    DryRun,
    Failed,
    OutcomeUnknown,
}

impl UpdateStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Updated => "updated",
            Self::Unchanged => "unchanged",
            Self::DryRun => "dry-run",
            Self::Failed => "failed",
            Self::OutcomeUnknown => "outcome-unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOutcome {
    pub status: UpdateStatus,
    pub changed: Option<bool>,
    pub would_change: bool,
    pub verified: bool,
    pub transaction_id: String,
    pub before_category: Option<CategoryRef>,
    pub after_category: Option<CategoryRef>,
}

pub struct UpdateExecution {
    pub outcome: UpdateOutcome,
    pub failure: Option<anyhow::Error>,
}

#[derive(Debug, Deserialize)]
struct Transaction {
    id: String,
    category: Option<CategoryRef>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PayloadError {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    field_errors: Option<Vec<PayloadFieldError>>,
}

#[derive(Debug, Deserialize)]
struct PayloadFieldError {
    #[serde(default)]
    field: Option<String>,
    #[serde(default)]
    messages: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum CategorySelector<'a> {
    Name(&'a str),
    Id(&'a str),
}

#[derive(Debug)]
enum MutationPayloadOutcome {
    Accepted,
    Rejected(String),
}

enum MutationAttempt {
    Accepted,
    DefinitiveFailure(String),
    AmbiguousFailure(String),
}

pub fn parse_categories(data: &Value) -> Result<Vec<Category>> {
    let raw = data
        .get("categories")
        .context("GetCategories response missing categories")?;
    let categories: Vec<Category> =
        serde_json::from_value(raw.clone()).context("failed to parse GetCategories categories")?;

    for (index, category) in categories.iter().enumerate() {
        if category.id.trim().is_empty() {
            anyhow::bail!("GetCategories category at index {index} has an empty id");
        }
        if category.name.trim().is_empty() {
            anyhow::bail!("GetCategories category at index {index} has an empty name");
        }
        if category.group.id.trim().is_empty() {
            anyhow::bail!("GetCategories category at index {index} has an empty group id");
        }
        if category.group.name.trim().is_empty() {
            anyhow::bail!("GetCategories category at index {index} has an empty group name");
        }
        if category.group.kind.trim().is_empty() {
            anyhow::bail!("GetCategories category at index {index} has an empty group type");
        }
    }

    Ok(categories)
}

#[cfg(test)]
fn resolve_category(
    categories: &[Category],
    category_name: Option<&str>,
    category_id: Option<&str>,
) -> Result<Category> {
    let selector = category_selector(category_name, category_id)?;
    resolve_selector(categories, selector)
}

pub fn execute<F>(args: &TransactionUpdateArgs, mut graphql: F) -> Result<UpdateExecution>
where
    F: FnMut(&str, &str, Value) -> Result<Value>,
{
    if args.transaction_id.trim().is_empty() {
        anyhow::bail!("TRANSACTION_ID must not be empty");
    }

    let selector = category_selector(args.category.as_deref(), args.category_id.as_deref())?;

    let before_data = graphql(
        "GetTransactionForUpdate",
        queries::GET_TRANSACTION_FOR_UPDATE,
        queries::transaction_for_update_variables(&args.transaction_id),
    )
    .with_context(|| {
        format!(
            "failed to read transaction {} before update",
            args.transaction_id
        )
    })?;
    let before = parse_transaction(&before_data, &args.transaction_id)?;

    let categories_data = graphql("GetCategories", queries::CATEGORIES, serde_json::json!({}))
        .context("failed to read categories before update")?;
    let categories = parse_categories(&categories_data)?;
    let target = resolve_selector(&categories, selector)?;
    let target_ref = CategoryRef::from(&target);
    let before_category = before.category.clone();

    if transaction_has_category(&before, &target.id) {
        return Ok(UpdateExecution {
            outcome: UpdateOutcome {
                status: UpdateStatus::Unchanged,
                changed: Some(false),
                would_change: false,
                verified: true,
                transaction_id: before.id,
                before_category: before_category.clone(),
                after_category: before_category,
            },
            failure: None,
        });
    }

    if args.dry_run {
        return Ok(UpdateExecution {
            outcome: UpdateOutcome {
                status: UpdateStatus::DryRun,
                changed: Some(false),
                would_change: true,
                verified: false,
                transaction_id: before.id,
                before_category,
                after_category: Some(target_ref),
            },
            failure: None,
        });
    }

    let mutation_attempt = match graphql(
        "Web_TransactionDrawerUpdateTransaction",
        queries::UPDATE_TRANSACTION,
        queries::update_transaction_variables(&args.transaction_id, &target.id),
    ) {
        Ok(data) => match parse_mutation_payload(&data, &args.transaction_id, &target) {
            Ok(MutationPayloadOutcome::Accepted) => MutationAttempt::Accepted,
            Ok(MutationPayloadOutcome::Rejected(message)) => {
                MutationAttempt::DefinitiveFailure(message)
            }
            Err(error) => MutationAttempt::AmbiguousFailure(format!(
                "mutation response could not be validated: {error:#}"
            )),
        },
        Err(error) => classify_mutation_error(error),
    };

    // Verification is a read only. The mutation is never retried.
    let after_result = graphql(
        "GetTransactionForUpdate",
        queries::GET_TRANSACTION_FOR_UPDATE,
        queries::transaction_for_update_variables(&args.transaction_id),
    )
    .and_then(|data| parse_transaction(&data, &args.transaction_id));

    match after_result {
        Ok(after) if transaction_has_category(&after, &target.id) => Ok(UpdateExecution {
            outcome: UpdateOutcome {
                status: UpdateStatus::Updated,
                changed: Some(true),
                would_change: true,
                verified: true,
                transaction_id: after.id,
                before_category,
                after_category: after.category,
            },
            failure: None,
        }),
        Ok(after) => Ok(classify_non_target_readback(
            mutation_attempt,
            before_category,
            after,
            &target_ref,
        )),
        Err(read_error) => {
            let mutation_detail = match mutation_attempt {
                MutationAttempt::Accepted => "mutation response was accepted".to_owned(),
                MutationAttempt::DefinitiveFailure(reason) => {
                    format!("mutation was definitively rejected: {reason}")
                }
                MutationAttempt::AmbiguousFailure(reason) => {
                    format!("mutation outcome was ambiguous: {reason}")
                }
            };

            Ok(UpdateExecution {
                outcome: UpdateOutcome {
                    status: UpdateStatus::OutcomeUnknown,
                    changed: None,
                    would_change: true,
                    verified: false,
                    transaction_id: before.id,
                    before_category,
                    after_category: None,
                },
                failure: Some(anyhow::anyhow!(
                    "{mutation_detail}; read-back verification failed: {read_error:#}"
                )),
            })
        }
    }
}

fn classify_non_target_readback(
    mutation_attempt: MutationAttempt,
    before_category: Option<CategoryRef>,
    after: Transaction,
    target: &CategoryRef,
) -> UpdateExecution {
    let actual_description = describe_category(after.category.as_ref());
    let target_description = describe_category(Some(target));

    match mutation_attempt {
        MutationAttempt::DefinitiveFailure(reason) => {
            let changed = !same_category(before_category.as_ref(), after.category.as_ref());
            UpdateExecution {
                outcome: UpdateOutcome {
                    status: UpdateStatus::Failed,
                    changed: Some(changed),
                    would_change: true,
                    verified: false,
                    transaction_id: after.id,
                    before_category,
                    after_category: after.category,
                },
                failure: Some(anyhow::anyhow!(
                    "mutation was definitively rejected: {reason}; read-back found {actual_description}, not requested {target_description}"
                )),
            }
        }
        MutationAttempt::Accepted => UpdateExecution {
            outcome: UpdateOutcome {
                status: UpdateStatus::OutcomeUnknown,
                changed: None,
                would_change: true,
                verified: false,
                transaction_id: after.id,
                before_category,
                after_category: after.category,
            },
            failure: Some(anyhow::anyhow!(
                "mutation response reported the requested state, but read-back found {actual_description}, not requested {target_description}"
            )),
        },
        MutationAttempt::AmbiguousFailure(reason) => UpdateExecution {
            outcome: UpdateOutcome {
                status: UpdateStatus::OutcomeUnknown,
                changed: None,
                would_change: true,
                verified: false,
                transaction_id: after.id,
                before_category,
                after_category: after.category,
            },
            failure: Some(anyhow::anyhow!(
                "mutation outcome was ambiguous: {reason}; immediate read-back found {actual_description}, not requested {target_description}; the mutation may still apply later"
            )),
        },
    }
}

fn classify_mutation_error(error: anyhow::Error) -> MutationAttempt {
    let class = error
        .downcast_ref::<GraphqlResponseError>()
        .map(GraphqlResponseError::class);
    let description = format!("{error:#}");

    match class {
        Some(GraphqlErrorClass::SchemaIncompatible | GraphqlErrorClass::DefinitiveRejection) => {
            MutationAttempt::DefinitiveFailure(description)
        }
        Some(GraphqlErrorClass::Ambiguous) | None => MutationAttempt::AmbiguousFailure(description),
    }
}

impl From<&Category> for CategoryRef {
    fn from(category: &Category) -> Self {
        Self {
            id: category.id.clone(),
            name: category.name.clone(),
        }
    }
}

fn category_selector<'a>(
    category_name: Option<&'a str>,
    category_id: Option<&'a str>,
) -> Result<CategorySelector<'a>> {
    match (category_name, category_id) {
        (Some(name), None) if !name.trim().is_empty() => Ok(CategorySelector::Name(name)),
        (None, Some(id)) if !id.trim().is_empty() => Ok(CategorySelector::Id(id)),
        (Some(_), None) => anyhow::bail!("--category must not be empty"),
        (None, Some(_)) => anyhow::bail!("--category-id must not be empty"),
        (None, None) => anyhow::bail!("one of --category or --category-id is required"),
        (Some(_), Some(_)) => {
            anyhow::bail!("--category and --category-id cannot be used together")
        }
    }
}

fn resolve_selector(categories: &[Category], selector: CategorySelector<'_>) -> Result<Category> {
    let matches = match selector {
        CategorySelector::Name(name) => {
            let wanted = name.trim();
            let folded = wanted.to_lowercase();
            let matches = categories
                .iter()
                .filter(|category| category.name.trim().to_lowercase() == folded)
                .collect::<Vec<_>>();

            if matches.is_empty() {
                anyhow::bail!(
                    "no category named {wanted:?}; names are matched exactly after trimming, case-insensitively"
                );
            }
            if matches.len() > 1 {
                anyhow::bail!(
                    "category name {wanted:?} matched {} categories; use --category-id",
                    matches.len()
                );
            }
            matches
        }
        CategorySelector::Id(id) => {
            let wanted = id.trim();
            let matches = categories
                .iter()
                .filter(|category| category.id == wanted)
                .collect::<Vec<_>>();

            if matches.is_empty() {
                anyhow::bail!("no category with id {wanted:?}");
            }
            if matches.len() > 1 {
                anyhow::bail!("category id {wanted:?} matched multiple categories");
            }
            matches
        }
    };

    let category = matches[0];
    if category.is_disabled {
        anyhow::bail!(
            "category {:?} ({}) is disabled and cannot be assigned",
            category.name,
            category.id
        );
    }

    Ok(category.clone())
}

fn parse_transaction(data: &Value, expected_id: &str) -> Result<Transaction> {
    let raw = data
        .get("getTransaction")
        .context("GetTransactionForUpdate response missing getTransaction")?;
    if raw.is_null() {
        anyhow::bail!("transaction {expected_id} was not found");
    }

    parse_transaction_value(raw, expected_id)
}

fn parse_transaction_value(raw: &Value, expected_id: &str) -> Result<Transaction> {
    let transaction: Transaction =
        serde_json::from_value(raw.clone()).context("failed to parse transaction")?;

    if transaction.id != expected_id {
        anyhow::bail!(
            "expected transaction id {expected_id:?}, got {:?}",
            transaction.id
        );
    }
    if transaction.id.trim().is_empty() {
        anyhow::bail!("transaction response has an empty id");
    }
    if let Some(category) = &transaction.category {
        if category.id.trim().is_empty() {
            anyhow::bail!("transaction category has an empty id");
        }
        if category.name.trim().is_empty() {
            anyhow::bail!("transaction category has an empty name");
        }
    }

    Ok(transaction)
}

fn parse_mutation_payload(
    data: &Value,
    expected_id: &str,
    target: &Category,
) -> Result<MutationPayloadOutcome> {
    let payload = data
        .get("updateTransaction")
        .context("mutation response missing updateTransaction")?;
    if payload.is_null() {
        anyhow::bail!("mutation response returned a null updateTransaction payload");
    }

    let errors_value = payload
        .get("errors")
        .context("mutation payload missing errors")?;
    if !errors_value.is_null() {
        let errors: Vec<PayloadError> = serde_json::from_value(errors_value.clone())
            .context("failed to parse mutation payload errors")?;
        if !errors.is_empty() {
            return Ok(MutationPayloadOutcome::Rejected(format_payload_errors(
                &errors,
            )));
        }
    }

    let transaction_value = payload
        .get("transaction")
        .context("mutation payload missing transaction")?;
    if transaction_value.is_null() {
        anyhow::bail!("mutation payload returned a null transaction");
    }

    let transaction = parse_transaction_value(transaction_value, expected_id)?;
    if !transaction_has_category(&transaction, &target.id) {
        anyhow::bail!(
            "mutation payload transaction did not contain requested category {:?} ({})",
            target.name,
            target.id
        );
    }

    Ok(MutationPayloadOutcome::Accepted)
}

fn format_payload_errors(errors: &[PayloadError]) -> String {
    errors
        .iter()
        .map(|error| {
            let mut parts = Vec::new();

            match (
                error
                    .code
                    .as_deref()
                    .filter(|value| !value.trim().is_empty()),
                error
                    .message
                    .as_deref()
                    .filter(|value| !value.trim().is_empty()),
            ) {
                (Some(code), Some(message)) => parts.push(format!("[{code}] {message}")),
                (Some(code), None) => parts.push(format!("[{code}]")),
                (None, Some(message)) => parts.push(message.to_owned()),
                (None, None) => {}
            }

            for field_error in error.field_errors.as_deref().unwrap_or_default() {
                let field = field_error
                    .field
                    .as_deref()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or("<field>");
                let messages = if field_error.messages.is_empty() {
                    "<no message>".to_owned()
                } else {
                    field_error.messages.join(", ")
                };
                parts.push(format!("{field}: {messages}"));
            }

            if parts.is_empty() {
                "<unspecified mutation error>".to_owned()
            } else {
                parts.join("; ")
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn transaction_has_category(transaction: &Transaction, category_id: &str) -> bool {
    transaction
        .category
        .as_ref()
        .map(|category| category.id.as_str())
        == Some(category_id)
}

fn same_category(left: Option<&CategoryRef>, right: Option<&CategoryRef>) -> bool {
    left.map(|category| category.id.as_str()) == right.map(|category| category.id.as_str())
}

fn describe_category(category: Option<&CategoryRef>) -> String {
    match category {
        Some(category) => format!("{:?} ({})", category.name, category.id),
        None => "<uncategorized>".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use crate::cli::BrowserArgs;

    fn update_args(dry_run: bool) -> TransactionUpdateArgs {
        TransactionUpdateArgs {
            transaction_id: "tx-1".to_owned(),
            category: Some(" new category ".to_owned()),
            category_id: None,
            dry_run,
            json: true,
            browser: BrowserArgs::default(),
            session_file: None,
        }
    }

    fn categories_data() -> Value {
        serde_json::json!({
            "categories": [
                {
                    "id": "cat-old",
                    "name": "Old Category",
                    "isDisabled": false,
                    "group": { "id": "group-expense", "name": "Expenses", "type": "expense" }
                },
                {
                    "id": "cat-new",
                    "name": "New Category",
                    "isDisabled": false,
                    "group": { "id": "group-expense", "name": "Expenses", "type": "expense" }
                },
                {
                    "id": "cat-disabled",
                    "name": "Disabled Category",
                    "isDisabled": true,
                    "group": { "id": "group-expense", "name": "Expenses", "type": "expense" }
                }
            ]
        })
    }

    fn transaction_data(category_id: &str, category_name: &str) -> Value {
        serde_json::json!({
            "getTransaction": {
                "id": "tx-1",
                "category": { "id": category_id, "name": category_name }
            }
        })
    }

    fn mutation_data(category_id: &str, category_name: &str) -> Value {
        serde_json::json!({
            "updateTransaction": {
                "transaction": {
                    "id": "tx-1",
                    "category": { "id": category_id, "name": category_name }
                },
                "errors": []
            }
        })
    }

    fn execute_with(
        args: TransactionUpdateArgs,
        responses: Vec<Result<Value>>,
    ) -> (UpdateExecution, Vec<String>) {
        let mut responses = VecDeque::from(responses);
        let mut operations = Vec::new();

        let execution = execute(&args, |operation, _query, _variables| {
            operations.push(operation.to_owned());
            responses.pop_front().expect("unexpected GraphQL request")
        })
        .unwrap();

        assert!(responses.is_empty());
        (execution, operations)
    }

    #[test]
    fn resolves_trimmed_case_insensitive_name_exactly() {
        let categories = parse_categories(&categories_data()).unwrap();
        let category = resolve_category(&categories, Some("  nEw CaTeGoRy  "), None).unwrap();
        assert_eq!(category.id, "cat-new");
    }

    #[test]
    fn rejects_missing_duplicate_and_disabled_categories() {
        let categories = parse_categories(&categories_data()).unwrap();
        assert!(resolve_category(&categories, Some("missing"), None).is_err());
        assert!(
            resolve_category(&categories, Some("disabled category"), None)
                .unwrap_err()
                .to_string()
                .contains("disabled")
        );

        let mut duplicates = categories.clone();
        duplicates.push(Category {
            id: "cat-other".to_owned(),
            name: " new CATEGORY ".to_owned(),
            is_disabled: false,
            group: CategoryGroup {
                id: "group-expense".to_owned(),
                name: "Expenses".to_owned(),
                kind: "expense".to_owned(),
            },
        });
        assert!(resolve_category(&duplicates, Some("New Category"), None)
            .unwrap_err()
            .to_string()
            .contains("matched 2 categories"));
    }

    #[test]
    fn category_id_lookup_verifies_existence_and_enabled_state() {
        let categories = parse_categories(&categories_data()).unwrap();
        assert_eq!(
            resolve_category(&categories, None, Some(" cat-new "))
                .unwrap()
                .name,
            "New Category"
        );
        assert!(resolve_category(&categories, None, Some("missing")).is_err());
        assert!(resolve_category(&categories, None, Some("cat-disabled"))
            .unwrap_err()
            .to_string()
            .contains("disabled"));
    }

    #[test]
    fn category_parser_fails_closed_on_missing_schema_fields() {
        let data = serde_json::json!({
            "categories": [{ "id": "cat-1", "name": "Food", "isDisabled": false }]
        });
        assert!(parse_categories(&data).is_err());
    }

    #[test]
    fn formats_general_and_field_payload_errors() {
        let target = resolve_category(
            &parse_categories(&categories_data()).unwrap(),
            None,
            Some("cat-new"),
        )
        .unwrap();
        let data = serde_json::json!({
            "updateTransaction": {
                "transaction": null,
                "errors": [{
                    "message": "Category update rejected",
                    "code": "INVALID_CATEGORY",
                    "fieldErrors": [{
                        "field": "category",
                        "messages": ["Category is disabled", "Choose another category"]
                    }]
                }]
            }
        });

        let result = parse_mutation_payload(&data, "tx-1", &target).unwrap();
        let MutationPayloadOutcome::Rejected(message) = result else {
            panic!("expected payload rejection");
        };
        assert!(message.contains("[INVALID_CATEGORY] Category update rejected"));
        assert!(message.contains("category: Category is disabled, Choose another category"));
    }

    #[test]
    fn mutation_payload_requires_exact_transaction_and_category() {
        let target = resolve_category(
            &parse_categories(&categories_data()).unwrap(),
            None,
            Some("cat-new"),
        )
        .unwrap();

        let wrong_category = mutation_data("cat-old", "Old Category");
        assert!(parse_mutation_payload(&wrong_category, "tx-1", &target)
            .unwrap_err()
            .to_string()
            .contains("requested category"));

        let mut wrong_transaction = mutation_data("cat-new", "New Category");
        wrong_transaction["updateTransaction"]["transaction"]["id"] = serde_json::json!("tx-2");
        assert!(parse_mutation_payload(&wrong_transaction, "tx-1", &target)
            .unwrap_err()
            .to_string()
            .contains("expected transaction id"));
    }

    #[test]
    fn unchanged_is_verified_without_mutation() {
        let (execution, operations) = execute_with(
            update_args(false),
            vec![
                Ok(transaction_data("cat-new", "New Category")),
                Ok(categories_data()),
            ],
        );

        assert_eq!(execution.outcome.status, UpdateStatus::Unchanged);
        assert_eq!(execution.outcome.changed, Some(false));
        assert!(execution.outcome.verified);
        assert_eq!(operations, vec!["GetTransactionForUpdate", "GetCategories"]);
    }

    #[test]
    fn dry_run_never_sends_mutation() {
        let (execution, operations) = execute_with(
            update_args(true),
            vec![
                Ok(transaction_data("cat-old", "Old Category")),
                Ok(categories_data()),
            ],
        );

        assert_eq!(execution.outcome.status, UpdateStatus::DryRun);
        assert!(execution.outcome.would_change);
        assert!(!execution.outcome.verified);
        assert_eq!(operations, vec!["GetTransactionForUpdate", "GetCategories"]);
    }

    #[test]
    fn updated_requires_exact_readback_verification() {
        let (execution, operations) = execute_with(
            update_args(false),
            vec![
                Ok(transaction_data("cat-old", "Old Category")),
                Ok(categories_data()),
                Ok(mutation_data("cat-new", "New Category")),
                Ok(transaction_data("cat-new", "New Category")),
            ],
        );

        assert_eq!(execution.outcome.status, UpdateStatus::Updated);
        assert_eq!(execution.outcome.changed, Some(true));
        assert!(execution.outcome.verified);
        assert!(execution.failure.is_none());
        assert_eq!(
            operations,
            vec![
                "GetTransactionForUpdate",
                "GetCategories",
                "Web_TransactionDrawerUpdateTransaction",
                "GetTransactionForUpdate"
            ]
        );
    }

    #[test]
    fn ambiguous_transport_failure_with_target_readback_is_success() {
        let (execution, _) = execute_with(
            update_args(false),
            vec![
                Ok(transaction_data("cat-old", "Old Category")),
                Ok(categories_data()),
                Err(anyhow::anyhow!("connection closed before response")),
                Ok(transaction_data("cat-new", "New Category")),
            ],
        );

        assert_eq!(execution.outcome.status, UpdateStatus::Updated);
        assert!(execution.outcome.verified);
        assert!(execution.failure.is_none());
    }

    #[test]
    fn ambiguous_transport_failure_with_old_readback_is_unknown() {
        let (execution, _) = execute_with(
            update_args(false),
            vec![
                Ok(transaction_data("cat-old", "Old Category")),
                Ok(categories_data()),
                Err(anyhow::anyhow!("request timed out after body was sent")),
                Ok(transaction_data("cat-old", "Old Category")),
            ],
        );

        assert_eq!(execution.outcome.status, UpdateStatus::OutcomeUnknown);
        assert_eq!(execution.outcome.changed, None);
        assert!(!execution.outcome.verified);
        assert!(execution
            .failure
            .unwrap()
            .to_string()
            .contains("may still apply later"));
    }

    #[test]
    fn schema_rejection_with_old_readback_is_failed() {
        let response = serde_json::json!({
            "errors": [{
                "message": "Something went wrong while processing.",
                "locations": [{ "line": 1, "column": 1 }]
            }]
        });
        let graphql_error = crate::graphql::response_error(
            "Web_TransactionDrawerUpdateTransaction",
            Some(400),
            &response,
        )
        .unwrap()
        .unwrap();

        let (execution, _) = execute_with(
            update_args(false),
            vec![
                Ok(transaction_data("cat-old", "Old Category")),
                Ok(categories_data()),
                Err(anyhow::Error::new(graphql_error)),
                Ok(transaction_data("cat-old", "Old Category")),
            ],
        );

        assert_eq!(execution.outcome.status, UpdateStatus::Failed);
        assert_eq!(execution.outcome.changed, Some(false));
        assert!(execution
            .failure
            .unwrap()
            .to_string()
            .contains("definitively rejected"));
    }

    #[test]
    fn accepted_mutation_with_conflicting_readback_is_unknown() {
        let (execution, _) = execute_with(
            update_args(false),
            vec![
                Ok(transaction_data("cat-old", "Old Category")),
                Ok(categories_data()),
                Ok(mutation_data("cat-new", "New Category")),
                Ok(transaction_data("cat-old", "Old Category")),
            ],
        );

        assert_eq!(execution.outcome.status, UpdateStatus::OutcomeUnknown);
        assert_eq!(execution.outcome.changed, None);
        assert!(!execution.outcome.verified);
    }

    #[test]
    fn failed_readback_is_outcome_unknown() {
        let (execution, _) = execute_with(
            update_args(false),
            vec![
                Ok(transaction_data("cat-old", "Old Category")),
                Ok(categories_data()),
                Ok(mutation_data("cat-new", "New Category")),
                Err(anyhow::anyhow!("read-back unavailable")),
            ],
        );

        assert_eq!(execution.outcome.status, UpdateStatus::OutcomeUnknown);
        assert_eq!(execution.outcome.changed, None);
        assert!(!execution.outcome.verified);
        assert!(execution.failure.is_some());
    }
}
