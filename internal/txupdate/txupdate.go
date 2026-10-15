// Package txupdate implements the single-transaction category update policy:
// exact category resolution, no retries, and read-back verified outcomes.
package txupdate

import (
	"errors"
	"fmt"
	"strings"
	"sync"

	"github.com/xiaotianxt/mon/internal/graphql"
	"github.com/xiaotianxt/mon/internal/jsonx"
	"github.com/xiaotianxt/mon/internal/queries"
)

type CategoryGroup struct {
	ID   string `json:"id"`
	Name string `json:"name"`
	Type string `json:"type"`
}

type Category struct {
	ID         string        `json:"id"`
	Name       string        `json:"name"`
	IsDisabled bool          `json:"isDisabled"`
	Group      CategoryGroup `json:"group"`
}

type CategoryRef struct {
	ID   string `json:"id"`
	Name string `json:"name"`
}

type Status string

const (
	Updated        Status = "updated"
	Unchanged      Status = "unchanged"
	DryRun         Status = "dry-run"
	Failed         Status = "failed"
	OutcomeUnknown Status = "outcome-unknown"
)

type Outcome struct {
	Status Status `json:"status"`
	// Changed is nil when the final state cannot be proven.
	Changed        *bool        `json:"changed"`
	WouldChange    bool         `json:"wouldChange"`
	Verified       bool         `json:"verified"`
	TransactionID  string       `json:"transactionId"`
	BeforeCategory *CategoryRef `json:"beforeCategory"`
	AfterCategory  *CategoryRef `json:"afterCategory"`
}

// Execution pairs a reportable outcome with the failure that makes the command
// exit non-zero; Failure is nil only for updated, unchanged, and dry-run.
type Execution struct {
	Outcome Outcome
	Failure error
}

type Request struct {
	TransactionID string
	// Exactly one of CategoryName and CategoryID must be non-nil.
	CategoryName *string
	CategoryID   *string
	DryRun       bool
}

// GraphQLFunc runs one operation and returns its data member. It must be safe
// for concurrent use.
type GraphQLFunc func(operation, query string, variables any) (any, error)

type transaction struct {
	ID       string       `json:"id"`
	Category *CategoryRef `json:"category"`
}

func (t transaction) hasCategory(id string) bool {
	return t.Category != nil && t.Category.ID == id
}

type mutationKind int

const (
	mutationAccepted mutationKind = iota
	mutationDefinitiveFailure
	mutationAmbiguousFailure
)

type mutationAttempt struct {
	kind   mutationKind
	reason string
}

func ParseCategories(data any) ([]Category, error) {
	raw, ok := jsonx.Lookup(data, "categories")
	if !ok {
		return nil, errors.New("GetCategories response missing categories")
	}
	var wire []struct {
		ID         string         `json:"id"`
		Name       string         `json:"name"`
		IsDisabled *bool          `json:"isDisabled"`
		Group      *CategoryGroup `json:"group"`
	}
	if err := jsonx.Convert(raw, &wire); err != nil {
		return nil, fmt.Errorf("failed to parse GetCategories categories: %w", err)
	}
	if wire == nil {
		return nil, errors.New("failed to parse GetCategories categories: categories is null")
	}

	categories := make([]Category, 0, len(wire))
	for i, w := range wire {
		missing := ""
		switch {
		case blank(w.ID):
			missing = "an empty id"
		case blank(w.Name):
			missing = "an empty name"
		case w.IsDisabled == nil:
			missing = "no isDisabled flag"
		case w.Group == nil:
			missing = "no group"
		case blank(w.Group.ID):
			missing = "an empty group id"
		case blank(w.Group.Name):
			missing = "an empty group name"
		case blank(w.Group.Type):
			missing = "an empty group type"
		}
		if missing != "" {
			return nil, fmt.Errorf("GetCategories category at index %d has %s", i, missing)
		}
		categories = append(categories, Category{ID: w.ID, Name: w.Name, IsDisabled: *w.IsDisabled, Group: *w.Group})
	}
	return categories, nil
}

func Execute(req Request, gql GraphQLFunc) (Execution, error) {
	if blank(req.TransactionID) {
		return Execution{}, errors.New("TRANSACTION_ID must not be empty")
	}
	if err := validateSelector(req); err != nil {
		return Execution{}, err
	}

	// The two pre-mutation reads are independent, so they run concurrently.
	var (
		wg                         sync.WaitGroup
		beforeData, categoriesData any
		beforeErr, categoriesErr   error
	)
	wg.Go(func() {
		beforeData, beforeErr = gql("GetTransactionForUpdate", queries.GetTransactionForUpdate,
			queries.TransactionForUpdateVariables(req.TransactionID))
	})
	wg.Go(func() {
		categoriesData, categoriesErr = gql("GetCategories", queries.Categories, map[string]any{})
	})
	wg.Wait()

	if beforeErr != nil {
		return Execution{}, fmt.Errorf("failed to read transaction %s before update: %w", req.TransactionID, beforeErr)
	}
	before, err := parseTransaction(beforeData, req.TransactionID)
	if err != nil {
		return Execution{}, err
	}
	if categoriesErr != nil {
		return Execution{}, fmt.Errorf("failed to read categories before update: %w", categoriesErr)
	}
	categories, err := ParseCategories(categoriesData)
	if err != nil {
		return Execution{}, err
	}
	target, err := resolveCategory(categories, req.CategoryName, req.CategoryID)
	if err != nil {
		return Execution{}, err
	}
	targetRef := &CategoryRef{ID: target.ID, Name: target.Name}

	if before.hasCategory(target.ID) {
		return Execution{Outcome: Outcome{
			Status: Unchanged, Changed: ptr(false), Verified: true,
			TransactionID: before.ID, BeforeCategory: before.Category, AfterCategory: before.Category,
		}}, nil
	}

	if req.DryRun {
		return Execution{Outcome: Outcome{
			Status: DryRun, Changed: ptr(false), WouldChange: true,
			TransactionID: before.ID, BeforeCategory: before.Category, AfterCategory: targetRef,
		}}, nil
	}

	var attempt mutationAttempt
	data, err := gql("Web_TransactionDrawerUpdateTransaction", queries.UpdateTransaction,
		queries.UpdateTransactionVariables(req.TransactionID, target.ID))
	if err != nil {
		attempt = classifyMutationError(err)
	} else {
		attempt = parseMutationPayload(data, req.TransactionID, target)
	}

	// Verification is a read only. The mutation is never retried.
	afterData, err := gql("GetTransactionForUpdate", queries.GetTransactionForUpdate,
		queries.TransactionForUpdateVariables(req.TransactionID))
	var after transaction
	if err == nil {
		after, err = parseTransaction(afterData, req.TransactionID)
	}
	if err != nil {
		return Execution{
			Outcome: Outcome{
				Status: OutcomeUnknown, WouldChange: true,
				TransactionID: before.ID, BeforeCategory: before.Category,
			},
			Failure: fmt.Errorf("%s; read-back verification failed: %w", describeAttempt(attempt), err),
		}, nil
	}

	if after.hasCategory(target.ID) {
		return Execution{Outcome: Outcome{
			Status: Updated, Changed: ptr(true), WouldChange: true, Verified: true,
			TransactionID: after.ID, BeforeCategory: before.Category, AfterCategory: after.Category,
		}}, nil
	}
	return classifyNonTargetReadback(attempt, before.Category, after, targetRef), nil
}

func classifyNonTargetReadback(attempt mutationAttempt, beforeCategory *CategoryRef, after transaction, target *CategoryRef) Execution {
	actual := describeCategory(after.Category)
	wanted := describeCategory(target)
	outcome := Outcome{
		Status: OutcomeUnknown, WouldChange: true,
		TransactionID: after.ID, BeforeCategory: beforeCategory, AfterCategory: after.Category,
	}

	var failure error
	switch attempt.kind {
	case mutationDefinitiveFailure:
		outcome.Status = Failed
		outcome.Changed = ptr(categoryID(beforeCategory) != categoryID(after.Category))
		failure = fmt.Errorf("mutation was definitively rejected: %s; read-back found %s, not requested %s",
			attempt.reason, actual, wanted)
	case mutationAccepted:
		failure = fmt.Errorf("mutation response reported the requested state, but read-back found %s, not requested %s",
			actual, wanted)
	case mutationAmbiguousFailure:
		failure = fmt.Errorf("mutation outcome was ambiguous: %s; immediate read-back found %s, not requested %s; the mutation may still apply later",
			attempt.reason, actual, wanted)
	default:
		panic(fmt.Sprintf("unknown mutation attempt kind %d", attempt.kind))
	}
	return Execution{Outcome: outcome, Failure: failure}
}

func classifyMutationError(err error) mutationAttempt {
	var responseErr *graphql.ResponseError
	if errors.As(err, &responseErr) {
		switch responseErr.Class() {
		case graphql.SchemaIncompatible, graphql.DefinitiveRejection:
			return mutationAttempt{mutationDefinitiveFailure, err.Error()}
		}
	}
	return mutationAttempt{mutationAmbiguousFailure, err.Error()}
}

func describeAttempt(attempt mutationAttempt) string {
	switch attempt.kind {
	case mutationAccepted:
		return "mutation response was accepted"
	case mutationDefinitiveFailure:
		return "mutation was definitively rejected: " + attempt.reason
	case mutationAmbiguousFailure:
		return "mutation outcome was ambiguous: " + attempt.reason
	}
	panic(fmt.Sprintf("unknown mutation attempt kind %d", attempt.kind))
}

func validateSelector(req Request) error {
	switch {
	case req.CategoryName != nil && req.CategoryID != nil:
		return errors.New("--category and --category-id cannot be used together")
	case req.CategoryName != nil && blank(*req.CategoryName):
		return errors.New("--category must not be empty")
	case req.CategoryID != nil && blank(*req.CategoryID):
		return errors.New("--category-id must not be empty")
	case req.CategoryName == nil && req.CategoryID == nil:
		return errors.New("one of --category or --category-id is required")
	}
	return nil
}

// resolveCategory requires a validated selector: exactly one non-blank of name or id.
func resolveCategory(categories []Category, name, id *string) (Category, error) {
	var matches []Category
	if name != nil {
		wanted := strings.TrimSpace(*name)
		folded := strings.ToLower(wanted)
		for _, c := range categories {
			if strings.ToLower(strings.TrimSpace(c.Name)) == folded {
				matches = append(matches, c)
			}
		}
		if len(matches) == 0 {
			return Category{}, fmt.Errorf("no category named %q; names are matched exactly after trimming, case-insensitively", wanted)
		}
		if len(matches) > 1 {
			return Category{}, fmt.Errorf("category name %q matched %d categories; use --category-id", wanted, len(matches))
		}
	} else {
		wanted := strings.TrimSpace(*id)
		for _, c := range categories {
			if c.ID == wanted {
				matches = append(matches, c)
			}
		}
		if len(matches) == 0 {
			return Category{}, fmt.Errorf("no category with id %q", wanted)
		}
		if len(matches) > 1 {
			return Category{}, fmt.Errorf("category id %q matched multiple categories", wanted)
		}
	}

	category := matches[0]
	if category.IsDisabled {
		return Category{}, fmt.Errorf("category %q (%s) is disabled and cannot be assigned", category.Name, category.ID)
	}
	return category, nil
}

func parseTransaction(data any, expectedID string) (transaction, error) {
	raw, ok := jsonx.Lookup(data, "getTransaction")
	if !ok {
		return transaction{}, errors.New("GetTransactionForUpdate response missing getTransaction")
	}
	if raw == nil {
		return transaction{}, fmt.Errorf("transaction %s was not found", expectedID)
	}
	return parseTransactionValue(raw, expectedID)
}

func parseTransactionValue(raw any, expectedID string) (transaction, error) {
	var tx transaction
	if err := jsonx.Convert(raw, &tx); err != nil {
		return transaction{}, fmt.Errorf("failed to parse transaction: %w", err)
	}
	if tx.ID != expectedID {
		return transaction{}, fmt.Errorf("expected transaction id %q, got %q", expectedID, tx.ID)
	}
	if blank(tx.ID) {
		return transaction{}, errors.New("transaction response has an empty id")
	}
	if tx.Category != nil {
		if blank(tx.Category.ID) {
			return transaction{}, errors.New("transaction category has an empty id")
		}
		if blank(tx.Category.Name) {
			return transaction{}, errors.New("transaction category has an empty name")
		}
	}
	return tx, nil
}

type payloadError struct {
	Message     *string `json:"message"`
	Code        *string `json:"code"`
	FieldErrors []struct {
		Field    *string  `json:"field"`
		Messages []string `json:"messages"`
	} `json:"fieldErrors"`
}

// parseMutationPayload treats payload errors as definitive and any malformed
// or unexpected payload as ambiguous.
func parseMutationPayload(data any, expectedID string, target Category) mutationAttempt {
	ambiguous := func(format string, args ...any) mutationAttempt {
		return mutationAttempt{mutationAmbiguousFailure, "mutation response could not be validated: " + fmt.Sprintf(format, args...)}
	}

	payload, ok := jsonx.Lookup(data, "updateTransaction")
	if !ok {
		return ambiguous("mutation response missing updateTransaction")
	}
	if payload == nil {
		return ambiguous("mutation response returned a null updateTransaction payload")
	}

	rawErrors, ok := jsonx.Lookup(payload, "errors")
	if !ok {
		return ambiguous("mutation payload missing errors")
	}
	if rawErrors != nil {
		var errs []payloadError
		if err := jsonx.Convert(rawErrors, &errs); err != nil {
			return ambiguous("failed to parse mutation payload errors: %v", err)
		}
		if len(errs) > 0 {
			return mutationAttempt{mutationDefinitiveFailure, formatPayloadErrors(errs)}
		}
	}

	rawTx, ok := jsonx.Lookup(payload, "transaction")
	if !ok {
		return ambiguous("mutation payload missing transaction")
	}
	if rawTx == nil {
		return ambiguous("mutation payload returned a null transaction")
	}
	tx, err := parseTransactionValue(rawTx, expectedID)
	if err != nil {
		return ambiguous("%v", err)
	}
	if !tx.hasCategory(target.ID) {
		return ambiguous("mutation payload transaction did not contain requested category %q (%s)", target.Name, target.ID)
	}
	return mutationAttempt{kind: mutationAccepted}
}

func formatPayloadErrors(errs []payloadError) string {
	formatted := make([]string, 0, len(errs))
	for _, e := range errs {
		var parts []string
		code, message := nonBlank(e.Code), nonBlank(e.Message)
		switch {
		case code != "" && message != "":
			parts = append(parts, fmt.Sprintf("[%s] %s", code, message))
		case code != "":
			parts = append(parts, fmt.Sprintf("[%s]", code))
		case message != "":
			parts = append(parts, message)
		}
		for _, fe := range e.FieldErrors {
			field := nonBlank(fe.Field)
			if field == "" {
				field = "<field>"
			}
			messages := "<no message>"
			if len(fe.Messages) > 0 {
				messages = strings.Join(fe.Messages, ", ")
			}
			parts = append(parts, fmt.Sprintf("%s: %s", field, messages))
		}
		if len(parts) == 0 {
			formatted = append(formatted, "<unspecified mutation error>")
		} else {
			formatted = append(formatted, strings.Join(parts, "; "))
		}
	}
	return strings.Join(formatted, " | ")
}

func describeCategory(c *CategoryRef) string {
	if c == nil {
		return "<uncategorized>"
	}
	return fmt.Sprintf("%q (%s)", c.Name, c.ID)
}

func categoryID(c *CategoryRef) string {
	if c == nil {
		return ""
	}
	return c.ID
}

func nonBlank(s *string) string {
	if s == nil || blank(*s) {
		return ""
	}
	return *s
}

func blank(s string) bool { return strings.TrimSpace(s) == "" }

func ptr[T any](v T) *T { return &v }
