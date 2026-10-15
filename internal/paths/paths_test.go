package paths

import (
	"path/filepath"
	"strings"
	"testing"
)

func TestExpandsHomeRelativePaths(t *testing.T) {
	t.Setenv("HOME", "/Users/test")
	got, err := ExpandTilde("~/Library/Application Support/mon")
	if err != nil {
		t.Fatal(err)
	}
	if !filepath.IsAbs(got) || !strings.HasSuffix(got, "Library/Application Support/mon") {
		t.Fatalf("unexpected expansion %q", got)
	}
}
