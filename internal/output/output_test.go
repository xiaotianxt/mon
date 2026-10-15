package output

import (
	"bytes"
	"testing"

	"github.com/xiaotianxt/mon/internal/jsonx"
	"github.com/xiaotianxt/mon/internal/txupdate"
)

func TestTruncateCountsRunes(t *testing.T) {
	if got := truncate("咖啡店咖啡店", 5); got != "咖啡..." {
		t.Fatalf("got %q", got)
	}
	if got := truncate("short", 28); got != "short" {
		t.Fatalf("got %q", got)
	}
}

func TestPrintTransactionsTable(t *testing.T) {
	data, err := jsonx.Decode([]byte(`{"allTransactions":{"totalCount":1,"results":[
	  {"date":"2026-01-02","amount":-4.5,"merchant":null,"plaidName":"Blue Bottle","account":{"displayName":"Checking"},"category":{"name":"Coffee"}}]}}`))
	if err != nil {
		t.Fatal(err)
	}
	var buf bytes.Buffer
	if err := PrintTransactions(&buf, data, false); err != nil {
		t.Fatal(err)
	}
	want := "total: 1\n" +
		"date               amount merchant                     account                category\n" +
		"2026-01-02          -4.50 Blue Bottle                  Checking               Coffee\n"
	if buf.String() != want {
		t.Fatalf("got:\n%s\nwant:\n%s", buf.String(), want)
	}
}

func TestPrintJSONPreservesNumbersAndDoesNotEscapeHTML(t *testing.T) {
	data, _ := jsonx.Decode([]byte(`{"b":1.10,"a":"<&>","big":12345678901234567890}`))
	var buf bytes.Buffer
	if err := PrintJSON(&buf, data); err != nil {
		t.Fatal(err)
	}
	want := "{\n  \"a\": \"<&>\",\n  \"b\": 1.10,\n  \"big\": 12345678901234567890\n}\n"
	if buf.String() != want {
		t.Fatalf("got %q", buf.String())
	}
}

func TestPrintTransactionUpdateJSONShape(t *testing.T) {
	var buf bytes.Buffer
	outcome := txupdate.Outcome{Status: txupdate.OutcomeUnknown, WouldChange: true, TransactionID: "tx-1",
		BeforeCategory: &txupdate.CategoryRef{ID: "c1", Name: "Old"}}
	if err := PrintTransactionUpdate(&buf, outcome, true); err != nil {
		t.Fatal(err)
	}
	want := `{
  "afterCategory": null,
  "beforeCategory": {
    "id": "c1",
    "name": "Old"
  },
  "changed": null,
  "status": "outcome-unknown",
  "transactionId": "tx-1",
  "verified": false,
  "wouldChange": true
}
`
	if buf.String() != want {
		t.Fatalf("got %s", buf.String())
	}

	buf.Reset()
	if err := PrintTransactionUpdate(&buf, outcome, false); err != nil {
		t.Fatal(err)
	}
	if got := buf.String(); got != "outcome-unknown: transaction tx-1: \"Old\" (c1) -> <none> (changed: unknown, would change: true, verified: false)\n" {
		t.Fatalf("got %q", got)
	}
}
