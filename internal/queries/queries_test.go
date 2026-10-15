package queries

import (
	"reflect"
	"strings"
	"testing"
)

func TestTransactionVariablesRequireDatePairs(t *testing.T) {
	if _, err := TransactionVariables(0, 100, "", "2026-01-01", ""); err == nil {
		t.Fatal("expected unpaired start date to fail")
	}
	vars, err := TransactionVariables(0, 100, "", "2026-01-01", "2026-01-31")
	if err != nil {
		t.Fatal(err)
	}
	filters := vars["filters"].(map[string]any)
	if filters["startDate"] != "2026-01-01" || filters["endDate"] != "2026-01-31" {
		t.Fatalf("filters = %v", filters)
	}
}

func TestUpdateVariablesTargetOneExactTransaction(t *testing.T) {
	want := map[string]any{"input": map[string]any{"id": "tx-1", "category": "cat-1"}}
	if got := UpdateTransactionVariables("tx-1", "cat-1"); !reflect.DeepEqual(got, want) {
		t.Fatalf("got %v", got)
	}
}

func TestWriteDocumentsUseKnownMinimalPayloadShapes(t *testing.T) {
	for _, check := range []struct {
		doc, needle string
		want        bool
	}{
		{Categories, "group {", true},
		{Categories, "isDisabled", true},
		{GetTransactionForUpdate, "redirectPosted: false", true},
		{UpdateTransaction, "mutation Web_TransactionDrawerUpdateTransaction", true},
		{UpdateTransaction, "fieldErrors {", true},
		{GetTransactionForUpdate, "__typename", false},
		{UpdateTransaction, "__typename", false},
	} {
		if strings.Contains(check.doc, check.needle) != check.want {
			t.Fatalf("contains(%q) != %v", check.needle, check.want)
		}
	}
}
