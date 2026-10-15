package txupdate

import (
	"errors"
	"slices"
	"strings"
	"sync"
	"testing"

	"github.com/xiaotianxt/mon/internal/graphql"
	"github.com/xiaotianxt/mon/internal/jsonx"
)

type reply struct {
	data any
	err  error
}

func decode(t *testing.T, raw string) any {
	t.Helper()
	v, err := jsonx.Decode([]byte(raw))
	if err != nil {
		t.Fatal(err)
	}
	return v
}

const categoriesJSON = `{"categories":[
  {"id":"cat-old","name":"Old Category","isDisabled":false,"group":{"id":"group-expense","name":"Expenses","type":"expense"}},
  {"id":"cat-new","name":"New Category","isDisabled":false,"group":{"id":"group-expense","name":"Expenses","type":"expense"}},
  {"id":"cat-disabled","name":"Disabled Category","isDisabled":true,"group":{"id":"group-expense","name":"Expenses","type":"expense"}}
]}`

func categories(t *testing.T) []Category {
	t.Helper()
	cats, err := ParseCategories(decode(t, categoriesJSON))
	if err != nil {
		t.Fatal(err)
	}
	return cats
}

func txData(t *testing.T, id, name string) reply {
	return reply{data: decode(t, `{"getTransaction":{"id":"tx-1","category":{"id":"`+id+`","name":"`+name+`"}}}`)}
}

func mutationData(t *testing.T, id, name string) reply {
	return reply{data: decode(t, `{"updateTransaction":{"transaction":{"id":"tx-1","category":{"id":"`+id+`","name":"`+name+`"}},"errors":[]}}`)}
}

func updateRequest(dryRun bool) Request {
	return Request{TransactionID: "tx-1", CategoryName: ptr(" new category "), DryRun: dryRun}
}

// executeWith serves per-operation reply queues, since the pre-mutation reads
// run concurrently, and returns the operations issued in order of completion.
func executeWith(t *testing.T, req Request, replies map[string][]reply) (Execution, []string) {
	t.Helper()
	var mu sync.Mutex
	var operations []string
	execution, err := Execute(req, func(operation, _ string, _ any) (any, error) {
		mu.Lock()
		defer mu.Unlock()
		queue := replies[operation]
		if len(queue) == 0 {
			t.Errorf("unexpected GraphQL request %s", operation)
			return nil, errors.New("unexpected request")
		}
		replies[operation] = queue[1:]
		operations = append(operations, operation)
		return queue[0].data, queue[0].err
	})
	if err != nil {
		t.Fatal(err)
	}
	for operation, queue := range replies {
		if len(queue) != 0 {
			t.Fatalf("%d unused replies for %s", len(queue), operation)
		}
	}
	return execution, operations
}

func TestResolvesTrimmedCaseInsensitiveNameExactly(t *testing.T) {
	c, err := resolveCategory(categories(t), ptr("  nEw CaTeGoRy  "), nil)
	if err != nil || c.ID != "cat-new" {
		t.Fatalf("got %+v %v", c, err)
	}
}

func TestRejectsMissingDuplicateAndDisabledCategories(t *testing.T) {
	cats := categories(t)
	if _, err := resolveCategory(cats, ptr("missing"), nil); err == nil {
		t.Fatal("expected missing category to fail")
	}
	if _, err := resolveCategory(cats, ptr("disabled category"), nil); err == nil || !strings.Contains(err.Error(), "disabled") {
		t.Fatalf("err = %v", err)
	}
	duplicates := append(slices.Clone(cats), Category{
		ID: "cat-other", Name: " new CATEGORY ",
		Group: CategoryGroup{ID: "group-expense", Name: "Expenses", Type: "expense"},
	})
	if _, err := resolveCategory(duplicates, ptr("New Category"), nil); err == nil || !strings.Contains(err.Error(), "matched 2 categories") {
		t.Fatalf("err = %v", err)
	}
}

func TestCategoryIDLookupVerifiesExistenceAndEnabledState(t *testing.T) {
	cats := categories(t)
	if c, err := resolveCategory(cats, nil, ptr(" cat-new ")); err != nil || c.Name != "New Category" {
		t.Fatalf("got %+v %v", c, err)
	}
	if _, err := resolveCategory(cats, nil, ptr("missing")); err == nil {
		t.Fatal("expected missing id to fail")
	}
	if _, err := resolveCategory(cats, nil, ptr("cat-disabled")); err == nil || !strings.Contains(err.Error(), "disabled") {
		t.Fatalf("err = %v", err)
	}
}

func TestCategoryParserFailsClosedOnMissingSchemaFields(t *testing.T) {
	for _, raw := range []string{
		`{"categories":[{"id":"cat-1","name":"Food","isDisabled":false}]}`,
		`{"categories":[{"id":"cat-1","name":"Food","group":{"id":"g","name":"G","type":"expense"}}]}`,
		`{"categories":null}`,
		`{}`,
	} {
		if _, err := ParseCategories(decode(t, raw)); err == nil {
			t.Fatalf("expected failure for %s", raw)
		}
	}
}

func TestSelectorValidation(t *testing.T) {
	for _, req := range []Request{
		{TransactionID: "tx-1"},
		{TransactionID: "tx-1", CategoryName: ptr("  ")},
		{TransactionID: "tx-1", CategoryID: ptr("")},
		{TransactionID: "tx-1", CategoryName: ptr("a"), CategoryID: ptr("b")},
		{TransactionID: " ", CategoryName: ptr("a")},
	} {
		if _, err := Execute(req, func(string, string, any) (any, error) {
			t.Fatal("no request expected for invalid input")
			return nil, nil
		}); err == nil {
			t.Fatalf("expected %+v to fail", req)
		}
	}
}

func TestFormatsGeneralAndFieldPayloadErrors(t *testing.T) {
	target, _ := resolveCategory(categories(t), nil, ptr("cat-new"))
	data := decode(t, `{"updateTransaction":{"transaction":null,"errors":[{
	  "message":"Category update rejected","code":"INVALID_CATEGORY",
	  "fieldErrors":[{"field":"category","messages":["Category is disabled","Choose another category"]}]}]}}`)

	attempt := parseMutationPayload(data, "tx-1", target)
	if attempt.kind != mutationDefinitiveFailure ||
		!strings.Contains(attempt.reason, "[INVALID_CATEGORY] Category update rejected") ||
		!strings.Contains(attempt.reason, "category: Category is disabled, Choose another category") {
		t.Fatalf("attempt = %+v", attempt)
	}
}

func TestMutationPayloadRequiresExactTransactionAndCategory(t *testing.T) {
	target, _ := resolveCategory(categories(t), nil, ptr("cat-new"))

	wrongCategory := parseMutationPayload(mutationData(t, "cat-old", "Old Category").data, "tx-1", target)
	if wrongCategory.kind != mutationAmbiguousFailure || !strings.Contains(wrongCategory.reason, "requested category") {
		t.Fatalf("attempt = %+v", wrongCategory)
	}

	wrongTx := decode(t, `{"updateTransaction":{"transaction":{"id":"tx-2","category":{"id":"cat-new","name":"New Category"}},"errors":[]}}`)
	attempt := parseMutationPayload(wrongTx, "tx-1", target)
	if attempt.kind != mutationAmbiguousFailure || !strings.Contains(attempt.reason, "expected transaction id") {
		t.Fatalf("attempt = %+v", attempt)
	}
}

func TestUnchangedIsVerifiedWithoutMutation(t *testing.T) {
	exec, ops := executeWith(t, updateRequest(false), map[string][]reply{
		"GetTransactionForUpdate": {txData(t, "cat-new", "New Category")},
		"GetCategories":           {{data: decode(t, categoriesJSON)}},
	})
	if exec.Outcome.Status != Unchanged || *exec.Outcome.Changed || !exec.Outcome.Verified || len(ops) != 2 {
		t.Fatalf("exec = %+v ops = %v", exec, ops)
	}
}

func TestDryRunNeverSendsMutation(t *testing.T) {
	exec, ops := executeWith(t, updateRequest(true), map[string][]reply{
		"GetTransactionForUpdate": {txData(t, "cat-old", "Old Category")},
		"GetCategories":           {{data: decode(t, categoriesJSON)}},
	})
	if exec.Outcome.Status != DryRun || !exec.Outcome.WouldChange || exec.Outcome.Verified || len(ops) != 2 {
		t.Fatalf("exec = %+v ops = %v", exec, ops)
	}
	if exec.Outcome.AfterCategory.ID != "cat-new" {
		t.Fatalf("proposed category = %+v", exec.Outcome.AfterCategory)
	}
}

func TestUpdatedRequiresExactReadbackVerification(t *testing.T) {
	exec, ops := executeWith(t, updateRequest(false), map[string][]reply{
		"GetTransactionForUpdate":                {txData(t, "cat-old", "Old Category"), txData(t, "cat-new", "New Category")},
		"GetCategories":                          {{data: decode(t, categoriesJSON)}},
		"Web_TransactionDrawerUpdateTransaction": {mutationData(t, "cat-new", "New Category")},
	})
	if exec.Outcome.Status != Updated || !*exec.Outcome.Changed || !exec.Outcome.Verified || exec.Failure != nil {
		t.Fatalf("exec = %+v", exec)
	}
	if !slices.Equal(ops[2:], []string{"Web_TransactionDrawerUpdateTransaction", "GetTransactionForUpdate"}) {
		t.Fatalf("mutation must precede the read-back: %v", ops)
	}
}

func TestAmbiguousTransportFailureWithTargetReadbackIsSuccess(t *testing.T) {
	exec, _ := executeWith(t, updateRequest(false), map[string][]reply{
		"GetTransactionForUpdate":                {txData(t, "cat-old", "Old Category"), txData(t, "cat-new", "New Category")},
		"GetCategories":                          {{data: decode(t, categoriesJSON)}},
		"Web_TransactionDrawerUpdateTransaction": {{err: errors.New("connection closed before response")}},
	})
	if exec.Outcome.Status != Updated || !exec.Outcome.Verified || exec.Failure != nil {
		t.Fatalf("exec = %+v", exec)
	}
}

func TestAmbiguousTransportFailureWithOldReadbackIsUnknown(t *testing.T) {
	exec, _ := executeWith(t, updateRequest(false), map[string][]reply{
		"GetTransactionForUpdate":                {txData(t, "cat-old", "Old Category"), txData(t, "cat-old", "Old Category")},
		"GetCategories":                          {{data: decode(t, categoriesJSON)}},
		"Web_TransactionDrawerUpdateTransaction": {{err: errors.New("request timed out after body was sent")}},
	})
	if exec.Outcome.Status != OutcomeUnknown || exec.Outcome.Changed != nil || exec.Outcome.Verified ||
		exec.Failure == nil || !strings.Contains(exec.Failure.Error(), "may still apply later") {
		t.Fatalf("exec = %+v", exec)
	}
}

func TestSchemaRejectionWithOldReadbackIsFailed(t *testing.T) {
	responseErr, err := graphql.ParseResponseError("Web_TransactionDrawerUpdateTransaction", 400,
		decode(t, `{"errors":[{"message":"Something went wrong while processing.","locations":[{"line":1,"column":1}]}]}`))
	if err != nil {
		t.Fatal(err)
	}
	exec, _ := executeWith(t, updateRequest(false), map[string][]reply{
		"GetTransactionForUpdate":                {txData(t, "cat-old", "Old Category"), txData(t, "cat-old", "Old Category")},
		"GetCategories":                          {{data: decode(t, categoriesJSON)}},
		"Web_TransactionDrawerUpdateTransaction": {{err: responseErr}},
	})
	if exec.Outcome.Status != Failed || *exec.Outcome.Changed ||
		exec.Failure == nil || !strings.Contains(exec.Failure.Error(), "definitively rejected") {
		t.Fatalf("exec = %+v", exec)
	}
}

func TestAcceptedMutationWithConflictingReadbackIsUnknown(t *testing.T) {
	exec, _ := executeWith(t, updateRequest(false), map[string][]reply{
		"GetTransactionForUpdate":                {txData(t, "cat-old", "Old Category"), txData(t, "cat-old", "Old Category")},
		"GetCategories":                          {{data: decode(t, categoriesJSON)}},
		"Web_TransactionDrawerUpdateTransaction": {mutationData(t, "cat-new", "New Category")},
	})
	if exec.Outcome.Status != OutcomeUnknown || exec.Outcome.Changed != nil || exec.Outcome.Verified || exec.Failure == nil {
		t.Fatalf("exec = %+v", exec)
	}
}

func TestFailedReadbackIsOutcomeUnknown(t *testing.T) {
	exec, _ := executeWith(t, updateRequest(false), map[string][]reply{
		"GetTransactionForUpdate":                {txData(t, "cat-old", "Old Category"), {err: errors.New("read-back unavailable")}},
		"GetCategories":                          {{data: decode(t, categoriesJSON)}},
		"Web_TransactionDrawerUpdateTransaction": {mutationData(t, "cat-new", "New Category")},
	})
	if exec.Outcome.Status != OutcomeUnknown || exec.Outcome.Changed != nil || exec.Outcome.Verified || exec.Failure == nil {
		t.Fatalf("exec = %+v", exec)
	}
}

func TestPreReadFailureReportsTransactionFirst(t *testing.T) {
	_, err := Execute(updateRequest(false), func(operation, _ string, _ any) (any, error) {
		return nil, errors.New(operation + " down")
	})
	if err == nil || !strings.Contains(err.Error(), "failed to read transaction tx-1 before update: GetTransactionForUpdate down") {
		t.Fatalf("err = %v", err)
	}
}
