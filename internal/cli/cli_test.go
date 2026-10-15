package cli

import (
	"bytes"
	"strings"
	"testing"
)

// parse runs the command tree with a dispatch that captures the parsed command.
func parse(t *testing.T, args ...string) (any, error) {
	t.Helper()
	var got any
	root := newRootCmd("1.2.3", func(cmd any) error {
		got = cmd
		return nil
	})
	root.SetArgs(args)
	root.SetOut(&bytes.Buffer{})
	root.SetErr(&bytes.Buffer{})
	err := root.Execute()
	return got, err
}

func mustParse[T any](t *testing.T, args ...string) T {
	t.Helper()
	got, err := parse(t, args...)
	if err != nil {
		t.Fatalf("parse %v: %v", args, err)
	}
	cmd, ok := got.(T)
	if !ok {
		t.Fatalf("parse %v: got %T", args, got)
	}
	return cmd
}

func TestExposesPackageVersion(t *testing.T) {
	var out bytes.Buffer
	if code := Main("1.2.3", []string{"--version"}, &out, &out); code != 0 || out.String() != "mon 1.2.3\n" {
		t.Fatalf("code=%d out=%q", code, out.String())
	}
}

func TestAcceptsLegacyOpenbrowserFlagAliases(t *testing.T) {
	c := mustParse[*accountsCmd](t, "accounts",
		"--openbrowser-mcp-url", "http://127.0.0.1:3500/mcp",
		"--openbrowser-settings", "/tmp/settings.json")
	if c.browser.mcpURL != "http://127.0.0.1:3500/mcp" || c.browser.settingsFile != "/tmp/settings.json" {
		t.Fatalf("browser = %+v", c.browser)
	}
}

func TestAcceptsCommandsWithoutBrowserFlag(t *testing.T) {
	c := mustParse[*accountsCmd](t, "accounts", "--json")
	if !c.json || c.browser.tabID != nil {
		t.Fatalf("c = %+v", c)
	}
}

func TestAcceptsLegacyBrowserFlagSilently(t *testing.T) {
	c := mustParse[*accountsCmd](t, "accounts", "--browser", "--json")
	if !c.browser.legacyBrowser || !c.json {
		t.Fatalf("c = %+v", c)
	}
}

func TestPreservesTransactionsSearchCLI(t *testing.T) {
	c := mustParse[*transactionsCmd](t, "transactions", "--search", "coffee",
		"--start-date", "2026-01-01", "--end-date", "2026-01-31", "--json")
	if c.search != "coffee" || c.startDate != "2026-01-01" || c.endDate != "2026-01-31" ||
		!c.json || c.limit != 100 || c.offset != 0 {
		t.Fatalf("c = %+v", c)
	}
}

func TestParsesCategoriesCommand(t *testing.T) {
	if c := mustParse[*categoriesCmd](t, "categories", "--json"); !c.json {
		t.Fatalf("c = %+v", c)
	}
}

func TestParsesSingleTransactionUpdate(t *testing.T) {
	c := mustParse[*transactionUpdateCmd](t, "transaction", "update", "tx-1",
		"--category", "Groceries", "--dry-run", "--json", "--browser-tab-id", "42")
	if c.transactionID != "tx-1" || c.category == nil || *c.category != "Groceries" || c.categoryID != nil ||
		!c.dryRun || !c.json || c.browser.tabID == nil || *c.browser.tabID != 42 {
		t.Fatalf("c = %+v", c)
	}
}

func TestExplicitEmptyCategoryIsPresent(t *testing.T) {
	c := mustParse[*transactionUpdateCmd](t, "transaction", "update", "tx-1", "--category", "")
	if c.category == nil || *c.category != "" {
		t.Fatalf("category = %v", c.category)
	}
}

func TestTransactionUpdateRequiresExactlyOneCategorySelector(t *testing.T) {
	for _, args := range [][]string{
		{"transaction", "update", "tx-1"},
		{"transaction", "update", "tx-1", "--category", "Food", "--category-id", "cat-1"},
		{"transaction", "update", "--category", "Food"},
	} {
		if _, err := parse(t, args...); err == nil {
			t.Fatalf("expected %v to fail", args)
		}
	}
}

func TestGqlRequiresOperationAndQueryFile(t *testing.T) {
	if _, err := parse(t, "gql", "--operation", "X"); err == nil || !strings.Contains(err.Error(), "query-file") {
		t.Fatalf("err = %v", err)
	}
	c := mustParse[*gqlCmd](t, "gql", "--operation", "X", "--query-file", "q.graphql", "--full")
	if c.variables != nil || !c.full {
		t.Fatalf("c = %+v", c)
	}
}

func TestRejectsUnexpectedPositionalArguments(t *testing.T) {
	if _, err := parse(t, "accounts", "extra"); err == nil {
		t.Fatal("expected positional argument to be rejected")
	}
}
