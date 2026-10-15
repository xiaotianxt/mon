package cli

import (
	"fmt"
	"io"
	"os"

	"github.com/xiaotianxt/mon/internal/browser"
	"github.com/xiaotianxt/mon/internal/jsonx"
	"github.com/xiaotianxt/mon/internal/output"
	"github.com/xiaotianxt/mon/internal/queries"
	"github.com/xiaotianxt/mon/internal/txupdate"
)

type app struct {
	version string
	stdout  io.Writer
}

func (a *app) run(cmd any) error {
	switch c := cmd.(type) {
	case *authStatusCmd:
		return a.withClient(c.browser, func(client *browser.Client) error { return a.authStatus(c, client) })
	case *accountsCmd:
		data, err := a.graphql(c.browser, "GetAccounts", queries.Accounts, map[string]any{}, false)
		if err != nil {
			return err
		}
		return output.PrintAccounts(a.stdout, data, c.json)
	case *categoriesCmd:
		data, err := a.graphql(c.browser, "GetCategories", queries.Categories, map[string]any{}, false)
		if err != nil {
			return err
		}
		categories, err := txupdate.ParseCategories(data)
		if err != nil {
			return err
		}
		return output.PrintCategories(a.stdout, categories, c.json)
	case *transactionsCmd:
		variables, err := queries.TransactionVariables(c.offset, c.limit, c.search, c.startDate, c.endDate)
		if err != nil {
			return err
		}
		data, err := a.graphql(c.browser, "GetTransactionsList", queries.Transactions, variables, false)
		if err != nil {
			return err
		}
		return output.PrintTransactions(a.stdout, data, c.json)
	case *transactionUpdateCmd:
		return a.updateTransaction(c)
	case *gqlCmd:
		return a.gql(c)
	case *doctorCmd:
		return a.withClient(c.browser, func(client *browser.Client) error { return a.doctor(c, client) })
	case *installCmd:
		if err := install(a.stdout, c); err != nil {
			return fmt.Errorf("failed to install mon: %w", err)
		}
		return nil
	}
	panic(fmt.Sprintf("unhandled command %T", cmd))
}

func (a *app) withClient(flags browserFlags, op func(*browser.Client) error) error {
	client, err := browser.Connect(browser.Options{
		TabID:         flags.tabID,
		BrowserID:     flags.browserID,
		MCPURL:        flags.mcpURL,
		SettingsFile:  flags.settingsFile,
		ClientVersion: a.version,
	})
	if err != nil {
		return err
	}
	return client.Run(op)
}

func (a *app) graphql(flags browserFlags, operation, query string, variables any, full bool) (any, error) {
	var data any
	err := a.withClient(flags, func(client *browser.Client) error {
		var err error
		data, err = client.GraphQLFullOrData(operation, query, variables, full)
		return err
	})
	return data, err
}

func (a *app) gql(c *gqlCmd) error {
	query, err := os.ReadFile(c.queryFile)
	if err != nil {
		return fmt.Errorf("failed to read %s: %w", c.queryFile, err)
	}
	var variables any = map[string]any{}
	if c.variables != nil {
		if variables, err = jsonx.Decode([]byte(*c.variables)); err != nil {
			return fmt.Errorf("--variables must be JSON: %w", err)
		}
	}
	value, err := a.graphql(c.browser, c.operation, string(query), variables, c.full)
	if err != nil {
		return err
	}
	return output.PrintJSON(a.stdout, value)
}

func (a *app) updateTransaction(c *transactionUpdateCmd) error {
	req := txupdate.Request{
		TransactionID: c.transactionID,
		CategoryName:  c.category,
		CategoryID:    c.categoryID,
		DryRun:        c.dryRun,
	}
	var execution txupdate.Execution
	err := a.withClient(c.browser, func(client *browser.Client) error {
		var err error
		execution, err = txupdate.Execute(req, client.GraphQL)
		return err
	})
	if err != nil {
		return err
	}
	if err := output.PrintTransactionUpdate(a.stdout, execution.Outcome, c.json); err != nil {
		return err
	}
	return execution.Failure
}

func (a *app) subscription(client *browser.Client) (any, error) {
	data, err := client.GraphQL("GetSubscriptionDetails", queries.Subscription, map[string]any{})
	if err != nil {
		return nil, err
	}
	return jsonx.Get(data, "subscription"), nil
}

// browserIDValue is the JSON form of an optional browser id.
func browserIDValue(client *browser.Client) any {
	if id := client.BrowserID(); id != "" {
		return id
	}
	return nil
}

func (a *app) authStatus(c *authStatusCmd, client *browser.Client) error {
	subscription, err := a.subscription(client)
	if err != nil {
		return err
	}
	if c.json {
		return output.PrintJSON(a.stdout, map[string]any{
			"online":       true,
			"tabId":        client.TabID(),
			"browserId":    browserIDValue(client),
			"subscription": subscription,
		})
	}
	fmt.Fprintln(a.stdout, "auth: browser session active")
	fmt.Fprintf(a.stdout, "tab: %d\n", client.TabID())
	if id := client.BrowserID(); id != "" {
		fmt.Fprintf(a.stdout, "browser: %s\n", id)
	}
	fmt.Fprintln(a.stdout, "subscription: ok")
	return nil
}

func (a *app) doctor(c *doctorCmd, client *browser.Client) error {
	subscription, err := a.subscription(client)
	if err != nil {
		return err
	}
	if c.json {
		return output.PrintJSON(a.stdout, map[string]any{
			"broConnected":     true,
			"tabId":            client.TabID(),
			"browserId":        browserIDValue(client),
			"monarchConnected": true,
			"subscription":     subscription,
		})
	}
	fmt.Fprintln(a.stdout, "bro: connected")
	fmt.Fprintf(a.stdout, "monarch tab: %d (active)\n", client.TabID())
	if id := client.BrowserID(); id != "" {
		fmt.Fprintf(a.stdout, "browser instance: %s\n", id)
	}
	fmt.Fprintln(a.stdout, "auth: active browser session")
	return nil
}
