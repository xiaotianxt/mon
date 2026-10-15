// Package output renders compact human tables and pretty JSON.
package output

import (
	"encoding/json"
	"fmt"
	"io"

	"github.com/xiaotianxt/mon/internal/jsonx"
	"github.com/xiaotianxt/mon/internal/txupdate"
)

func PrintJSON(w io.Writer, v any) error {
	return jsonx.WritePretty(w, v)
}

// printSortedJSON prints a typed value with object keys in sorted order, the
// established mon JSON contract for every object it emits.
func printSortedJSON(w io.Writer, v any) error {
	raw, err := json.Marshal(v)
	if err != nil {
		return err
	}
	generic, err := jsonx.Decode(raw)
	if err != nil {
		return err
	}
	return jsonx.WritePretty(w, generic)
}

func PrintAccounts(w io.Writer, data any, asJSON bool) error {
	if asJSON {
		return PrintJSON(w, data)
	}

	fmt.Fprintf(w, "%-28s %14s %-18s institution\n", "name", "balance", "type")
	accounts, _ := jsonx.Get(data, "accounts").([]any)
	for _, account := range accounts {
		balance, ok := jsonx.Float(jsonx.Get(account, "displayBalance"))
		if !ok {
			balance, ok = jsonx.Float(jsonx.Get(account, "currentBalance"))
		}
		fmt.Fprintf(w, "%-28s %14s %-18s %s\n",
			truncate(str(account, "displayName"), 28),
			formatAmount(balance, ok),
			truncate(str(account, "type", "display"), 18),
			str(account, "institution", "name"))
	}
	return nil
}

func PrintTransactions(w io.Writer, data any, asJSON bool) error {
	if asJSON {
		return PrintJSON(w, data)
	}

	total := jsonx.Get(data, "allTransactions", "totalCount")
	if _, ok := jsonx.Float(total); !ok {
		total = 0
	}
	fmt.Fprintf(w, "total: %v\n", total)
	fmt.Fprintf(w, "%-12s %12s %-28s %-22s category\n", "date", "amount", "merchant", "account")
	results, _ := jsonx.Get(data, "allTransactions", "results").([]any)
	for _, tx := range results {
		amount, ok := jsonx.Float(jsonx.Get(tx, "amount"))
		merchant := str(tx, "merchant", "name")
		if merchant == "" {
			merchant = str(tx, "plaidName")
		}
		fmt.Fprintf(w, "%-12s %12s %-28s %-22s %s\n",
			truncate(str(tx, "date"), 12),
			formatAmount(amount, ok),
			truncate(merchant, 28),
			truncate(str(tx, "account", "displayName"), 22),
			str(tx, "category", "name"))
	}
	return nil
}

func PrintCategories(w io.Writer, categories []txupdate.Category, asJSON bool) error {
	if asJSON {
		return printSortedJSON(w, map[string]any{"categories": categories})
	}

	fmt.Fprintf(w, "%-20s %-8s %-10s %-20s name\n", "id", "disabled", "type", "group")
	for _, c := range categories {
		fmt.Fprintf(w, "%-20s %-8t %-10s %-20s %s\n",
			truncate(c.ID, 20), c.IsDisabled, truncate(c.Group.Type, 10), truncate(c.Group.Name, 20), c.Name)
	}
	return nil
}

func PrintTransactionUpdate(w io.Writer, outcome txupdate.Outcome, asJSON bool) error {
	if asJSON {
		return printSortedJSON(w, outcome)
	}

	changed := "unknown"
	if outcome.Changed != nil {
		changed = fmt.Sprint(*outcome.Changed)
	}
	fmt.Fprintf(w, "%s: transaction %s: %s -> %s (changed: %s, would change: %t, verified: %t)\n",
		outcome.Status, outcome.TransactionID,
		formatCategory(outcome.BeforeCategory), formatCategory(outcome.AfterCategory),
		changed, outcome.WouldChange, outcome.Verified)
	return nil
}

func str(v any, path ...string) string {
	s, _ := jsonx.String(jsonx.Get(v, path...))
	return s
}

func formatAmount(value float64, ok bool) string {
	if !ok {
		return "-"
	}
	return fmt.Sprintf("%.2f", value)
}

// truncate limits a cell to width runes, marking the cut with "...".
func truncate(s string, width int) string {
	runes := []rune(s)
	if len(runes) <= width {
		return s
	}
	return string(runes[:max(width-3, 0)]) + "..."
}

func formatCategory(c *txupdate.CategoryRef) string {
	if c == nil {
		return "<none>"
	}
	return fmt.Sprintf("%s (%s)", jsonx.Compact(c.Name), c.ID)
}
