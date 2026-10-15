// Package cli parses mon's command line into typed commands and runs them.
package cli

import (
	"fmt"
	"io"

	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

// browserFlags select the bro endpoint and Monarch tab; every networked command has them.
type browserFlags struct {
	legacyBrowser bool
	tabID         *uint64
	browserID     string
	mcpURL        string
	settingsFile  string
}

type authStatusCmd struct {
	json    bool
	browser browserFlags
}

type accountsCmd struct {
	json    bool
	browser browserFlags
}

type categoriesCmd struct {
	json    bool
	browser browserFlags
}

type transactionsCmd struct {
	startDate, endDate string
	search             string
	limit, offset      uint32
	json               bool
	browser            browserFlags
}

type transactionUpdateCmd struct {
	transactionID string
	category      *string
	categoryID    *string
	dryRun        bool
	json          bool
	browser       browserFlags
}

type gqlCmd struct {
	operation string
	queryFile string
	variables *string
	full      bool
	browser   browserFlags
}

type doctorCmd struct {
	json    bool
	browser browserFlags
}

type installCmd struct {
	binDir string
	force  bool
}

// Main runs mon and returns the process exit code.
func Main(version string, args []string, stdout, stderr io.Writer) int {
	a := &app{version: version, stdout: stdout}
	root := newRootCmd(version, a.run)
	root.SetArgs(args)
	root.SetOut(stdout)
	root.SetErr(stderr)
	if err := root.Execute(); err != nil {
		fmt.Fprintf(stderr, "mon: %v\n", err)
		return 1
	}
	return 0
}

// newRootCmd builds the command tree; each leaf passes its parsed command to dispatch.
func newRootCmd(version string, dispatch func(cmd any) error) *cobra.Command {
	root := &cobra.Command{
		Use:           "mon",
		Short:         "AI-native Monarch Money CLI for structured local finance workflows",
		Version:       version,
		SilenceErrors: true,
		SilenceUsage:  true,
	}
	root.SetVersionTemplate("mon {{.Version}}\n")
	root.CompletionOptions.DisableDefaultCmd = true
	root.SetGlobalNormalizationFunc(normalizeLegacyFlags)

	auth := &cobra.Command{Use: "auth", Short: "Inspect Monarch browser auth status."}
	{
		var c authStatusCmd
		cmd := leaf("status", "Show Monarch browser auth status.", &c.browser, func([]string) error { return dispatch(&c) })
		cmd.Flags().BoolVar(&c.json, "json", false, "Print JSON.")
		auth.AddCommand(cmd)
	}
	root.AddCommand(auth)

	{
		var c accountsCmd
		cmd := leaf("accounts", "List Monarch accounts.", &c.browser, func([]string) error { return dispatch(&c) })
		cmd.Flags().BoolVar(&c.json, "json", false, "Print raw JSON instead of a compact table.")
		root.AddCommand(cmd)
	}

	{
		var c categoriesCmd
		cmd := leaf("categories", "List Monarch transaction categories.", &c.browser, func([]string) error { return dispatch(&c) })
		cmd.Flags().BoolVar(&c.json, "json", false, "Print raw JSON instead of a compact table.")
		root.AddCommand(cmd)
	}

	{
		var c transactionsCmd
		cmd := leaf("transactions", "Search Monarch transactions.", &c.browser, func([]string) error { return dispatch(&c) })
		f := cmd.Flags()
		f.StringVar(&c.startDate, "start-date", "", "Earliest transaction date, YYYY-MM-DD. Must be paired with --end-date.")
		f.StringVar(&c.endDate, "end-date", "", "Latest transaction date, YYYY-MM-DD. Must be paired with --start-date.")
		f.StringVar(&c.search, "search", "", "Monarch transaction search text.")
		f.Uint32Var(&c.limit, "limit", 100, "Maximum rows returned by Monarch.")
		f.Uint32Var(&c.offset, "offset", 0, "Offset for pagination.")
		f.BoolVar(&c.json, "json", false, "Print raw JSON instead of a compact table.")
		root.AddCommand(cmd)
	}

	transaction := &cobra.Command{Use: "transaction", Short: "Mutate one exact Monarch transaction."}
	{
		var c transactionUpdateCmd
		var category, categoryID string
		var cmd *cobra.Command
		cmd = leaf("update TRANSACTION_ID", "Update the category of one exact transaction.", &c.browser, func(args []string) error {
			c.transactionID = args[0]
			c.category = optional(cmd.Flags(), "category", category)
			c.categoryID = optional(cmd.Flags(), "category-id", categoryID)
			return dispatch(&c)
		})
		cmd.Args = cobra.ExactArgs(1)
		f := cmd.Flags()
		f.StringVar(&category, "category", "", "Exact category name after trimming, matched case-insensitively.")
		f.StringVar(&categoryID, "category-id", "", "Exact category id.")
		f.BoolVar(&c.dryRun, "dry-run", false, "Resolve and inspect the transaction without mutating it.")
		f.BoolVar(&c.json, "json", false, "Print structured JSON.")
		cmd.MarkFlagsOneRequired("category", "category-id")
		cmd.MarkFlagsMutuallyExclusive("category", "category-id")
		transaction.AddCommand(cmd)
	}
	root.AddCommand(transaction)

	{
		var c gqlCmd
		var variables string
		var cmd *cobra.Command
		cmd = leaf("gql", "Run an arbitrary GraphQL document against Monarch.", &c.browser, func([]string) error {
			c.variables = optional(cmd.Flags(), "variables", variables)
			return dispatch(&c)
		})
		f := cmd.Flags()
		f.StringVar(&c.operation, "operation", "", "GraphQL operation name.")
		f.StringVar(&c.queryFile, "query-file", "", "File containing a GraphQL query or mutation.")
		f.StringVar(&variables, "variables", "", "JSON variables object.")
		f.BoolVar(&c.full, "full", false, "Print the full GraphQL response instead of just data.")
		mustMarkRequired(cmd, "operation", "query-file")
		root.AddCommand(cmd)
	}

	{
		var c doctorCmd
		cmd := leaf("doctor", "Validate local config and Monarch browser connectivity.", &c.browser, func([]string) error { return dispatch(&c) })
		cmd.Flags().BoolVar(&c.json, "json", false, "Print JSON.")
		root.AddCommand(cmd)
	}

	{
		var c installCmd
		cmd := &cobra.Command{
			Use:   "install",
			Short: "Install mon into ~/.local/bin.",
			Args:  cobra.NoArgs,
			RunE:  func(*cobra.Command, []string) error { return dispatch(&c) },
		}
		cmd.Flags().StringVar(&c.binDir, "bin-dir", "", "Directory to install mon into.")
		cmd.Flags().BoolVar(&c.force, "force", false, "Replace an existing mon binary.")
		root.AddCommand(cmd)
	}

	return root
}

// leaf builds a networked command with the shared browser flags.
func leaf(use, short string, b *browserFlags, run func(args []string) error) *cobra.Command {
	var tabID uint64
	cmd := &cobra.Command{Use: use, Short: short, Args: cobra.NoArgs}
	cmd.RunE = func(cmd *cobra.Command, args []string) error {
		if cmd.Flags().Changed("browser-tab-id") {
			b.tabID = &tabID
		}
		return run(args)
	}

	f := cmd.Flags()
	f.BoolVar(&b.legacyBrowser, "browser", false, "Legacy compatibility flag; browser execution is the only mode.")
	if err := f.MarkHidden("browser"); err != nil {
		panic(err)
	}
	f.Uint64Var(&tabID, "browser-tab-id", 0, "Explicit browser tab id to use.")
	f.StringVar(&b.browserID, "browser-id", "", "Explicit bro browser id to use.")
	f.StringVar(&b.mcpURL, "bro-mcp-url", "", "bro MCP endpoint. Defaults to BRO_MCP_URL or http://127.0.0.1:3500/mcp.")
	f.StringVar(&b.settingsFile, "bro-settings", "", "bro settings file. Defaults to BRO_SETTINGS or ~/.bro/settings.json.")
	return cmd
}

// normalizeLegacyFlags accepts the pre-bro openbrowser flag names.
func normalizeLegacyFlags(_ *pflag.FlagSet, name string) pflag.NormalizedName {
	switch name {
	case "openbrowser-mcp-url":
		name = "bro-mcp-url"
	case "openbrowser-settings":
		name = "bro-settings"
	}
	return pflag.NormalizedName(name)
}

// optional distinguishes an explicitly empty flag from an absent one.
func optional(f *pflag.FlagSet, name, value string) *string {
	if !f.Changed(name) {
		return nil
	}
	return &value
}

func mustMarkRequired(cmd *cobra.Command, names ...string) {
	for _, name := range names {
		if err := cmd.MarkFlagRequired(name); err != nil {
			panic(err)
		}
	}
}
