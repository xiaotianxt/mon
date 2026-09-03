use anyhow::Result;
use serde_json::Value;

use crate::cli::TransactionArgs;

pub const SUBSCRIPTION: &str = r#"
query GetSubscriptionDetails {
  subscription {
    id
    paymentSource
    referralCode
    isOnFreeTrial
    hasPremiumEntitlement
    __typename
  }
}
"#;

pub const ACCOUNTS: &str = r#"
query GetAccounts {
  accounts {
    id
    displayName
    syncDisabled
    deactivatedAt
    isHidden
    isAsset
    mask
    currentBalance
    displayBalance
    includeInNetWorth
    hideFromList
    hideTransactionsFromReports
    dataProvider
    isManual
    transactionsCount
    holdingsCount
    logoUrl
    type {
      name
      display
      group
      __typename
    }
    subtype {
      name
      display
      __typename
    }
    institution {
      id
      name
      __typename
    }
    __typename
  }
  householdPreferences {
    id
    accountGroupOrder
    __typename
  }
}
"#;

pub const CATEGORIES: &str = r#"
query GetCategories {
  categories {
    id
    name
    isDisabled
    group {
      id
      name
      type
    }
  }
}
"#;

pub const GET_TRANSACTION_FOR_UPDATE: &str = r#"
query GetTransactionForUpdate($id: UUID!) {
  getTransaction(id: $id, redirectPosted: false) {
    id
    category {
      id
      name
    }
  }
}
"#;

pub const UPDATE_TRANSACTION: &str = r#"
mutation Web_TransactionDrawerUpdateTransaction($input: UpdateTransactionMutationInput!) {
  updateTransaction(input: $input) {
    transaction {
      id
      category {
        id
        name
      }
    }
    errors {
      message
      code
      fieldErrors {
        field
        messages
      }
    }
  }
}
"#;

pub const TRANSACTIONS: &str = r#"
query GetTransactionsList($offset: Int, $limit: Int, $filters: TransactionFilterInput, $orderBy: TransactionOrdering) {
  allTransactions(filters: $filters) {
    totalCount
    results(offset: $offset, limit: $limit, orderBy: $orderBy) {
      id
      amount
      pending
      date
      hideFromReports
      plaidName
      notes
      isRecurring
      reviewStatus
      needsReview
      isSplitTransaction
      createdAt
      updatedAt
      category {
        id
        name
        __typename
      }
      merchant {
        name
        id
        transactionsCount
        __typename
      }
      account {
        id
        displayName
        __typename
      }
      tags {
        id
        name
        color
        order
        __typename
      }
      __typename
    }
    __typename
  }
  transactionRules {
    id
    __typename
  }
}
"#;

pub fn transaction_for_update_variables(id: &str) -> Value {
    serde_json::json!({ "id": id })
}

pub fn update_transaction_variables(transaction_id: &str, category_id: &str) -> Value {
    serde_json::json!({
        "input": {
            "id": transaction_id,
            "category": category_id,
        }
    })
}

pub fn transaction_variables(args: &TransactionArgs) -> Result<Value> {
    transaction_variables_from(
        args.offset,
        args.limit,
        &args.search,
        args.start_date.as_deref(),
        args.end_date.as_deref(),
    )
}

pub fn transaction_variables_from(
    offset: u32,
    limit: u32,
    search: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<Value> {
    if start_date.is_some() ^ end_date.is_some() {
        anyhow::bail!("--start-date and --end-date must be provided together");
    }

    let mut filters = serde_json::json!({
        "search": search,
        "categories": [],
        "accounts": [],
        "tags": [],
    });

    if let (Some(start), Some(end)) = (start_date, end_date) {
        filters["startDate"] = serde_json::json!(start);
        filters["endDate"] = serde_json::json!(end);
    }

    Ok(serde_json::json!({
        "offset": offset,
        "limit": limit,
        "orderBy": "date",
        "filters": filters,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_variables_require_date_pairs() {
        assert!(transaction_variables_from(0, 100, "", Some("2026-01-01"), None).is_err());
        assert!(
            transaction_variables_from(0, 100, "", Some("2026-01-01"), Some("2026-01-31")).is_ok()
        );
    }

    #[test]
    fn update_variables_target_one_exact_transaction() {
        assert_eq!(
            update_transaction_variables("tx-1", "cat-1"),
            serde_json::json!({
                "input": {
                    "id": "tx-1",
                    "category": "cat-1"
                }
            })
        );
    }

    #[test]
    fn write_documents_use_known_minimal_payload_shapes() {
        assert!(CATEGORIES.contains("group {"));
        assert!(CATEGORIES.contains("isDisabled"));
        assert!(GET_TRANSACTION_FOR_UPDATE.contains("redirectPosted: false"));
        assert!(UPDATE_TRANSACTION.contains("mutation Web_TransactionDrawerUpdateTransaction"));
        assert!(UPDATE_TRANSACTION.contains("fieldErrors {"));
        assert!(!GET_TRANSACTION_FOR_UPDATE.contains("__typename"));
        assert!(!UPDATE_TRANSACTION.contains("__typename"));
    }
}
