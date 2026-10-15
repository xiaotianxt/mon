// Package queries holds the built-in GraphQL documents and variable builders.
package queries

import "errors"

const Subscription = `
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
`

const Accounts = `
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
`

const Categories = `
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
`

const GetTransactionForUpdate = `
query GetTransactionForUpdate($id: UUID!) {
  getTransaction(id: $id, redirectPosted: false) {
    id
    category {
      id
      name
    }
  }
}
`

const UpdateTransaction = `
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
`

const Transactions = `
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
`

func TransactionForUpdateVariables(id string) map[string]any {
	return map[string]any{"id": id}
}

func UpdateTransactionVariables(transactionID, categoryID string) map[string]any {
	return map[string]any{
		"input": map[string]any{
			"id":       transactionID,
			"category": categoryID,
		},
	}
}

// TransactionVariables treats an empty date as absent; both dates or neither are required.
func TransactionVariables(offset, limit uint32, search, startDate, endDate string) (map[string]any, error) {
	if (startDate == "") != (endDate == "") {
		return nil, errors.New("--start-date and --end-date must be provided together")
	}

	filters := map[string]any{
		"search":     search,
		"categories": []any{},
		"accounts":   []any{},
		"tags":       []any{},
	}
	if startDate != "" {
		filters["startDate"] = startDate
		filters["endDate"] = endDate
	}

	return map[string]any{
		"offset":  offset,
		"limit":   limit,
		"orderBy": "date",
		"filters": filters,
	}, nil
}
