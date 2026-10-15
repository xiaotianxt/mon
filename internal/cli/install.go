package cli

import (
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/xiaotianxt/mon/internal/paths"
)

func install(stdout io.Writer, c *installCmd) error {
	current, err := os.Executable()
	if err != nil {
		return fmt.Errorf("failed to locate current executable: %w", err)
	}

	binDir := c.binDir
	if binDir == "" {
		home, err := paths.HomeDir()
		if err != nil {
			return err
		}
		binDir = filepath.Join(home, ".local", "bin")
	}
	if binDir, err = paths.ExpandTilde(binDir); err != nil {
		return err
	}
	if err := os.MkdirAll(binDir, 0o755); err != nil {
		return fmt.Errorf("failed to create %s: %w", binDir, err)
	}

	target := filepath.Join(binDir, "mon")
	if _, err := os.Stat(target); err == nil && !c.force {
		return fmt.Errorf("%s already exists; rerun with --force to replace it", target)
	}
	if err := copyExecutable(current, target); err != nil {
		return fmt.Errorf("failed to copy %s to %s: %w", current, target, err)
	}

	fmt.Fprintf(stdout, "installed: %s\n", target)
	return nil
}

// copyExecutable writes to a temp file and renames it into place, so replacing
// a running binary never leaves a partially written target.
func copyExecutable(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()

	tmp, err := os.CreateTemp(filepath.Dir(dst), ".mon-install-*")
	if err != nil {
		return err
	}
	committed := false
	defer func() {
		if !committed {
			os.Remove(tmp.Name())
		}
	}()

	if _, err := io.Copy(tmp, in); err != nil {
		tmp.Close()
		return err
	}
	if err := tmp.Chmod(0o755); err != nil {
		tmp.Close()
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	if err := os.Rename(tmp.Name(), dst); err != nil {
		return err
	}
	committed = true
	return nil
}
