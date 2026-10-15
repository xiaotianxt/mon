package paths

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
)

func HomeDir() (string, error) {
	home, ok := os.LookupEnv("HOME")
	if !ok {
		return "", errors.New("HOME is not set")
	}
	return home, nil
}

func ExpandTilde(path string) (string, error) {
	if path == "~" {
		return HomeDir()
	}
	if rest, ok := strings.CutPrefix(path, "~/"); ok {
		home, err := HomeDir()
		if err != nil {
			return "", err
		}
		return filepath.Join(home, rest), nil
	}
	return path, nil
}
