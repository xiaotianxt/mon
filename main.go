package main

import (
	_ "embed"
	"os"
	"strings"

	"github.com/xiaotianxt/mon/internal/cli"
)

// VERSION is the single source of the release version; scripts/release.sh bumps it.
//
//go:embed VERSION
var version string

func main() {
	os.Exit(cli.Main(strings.TrimSpace(version), os.Args[1:], os.Stdout, os.Stderr))
}
