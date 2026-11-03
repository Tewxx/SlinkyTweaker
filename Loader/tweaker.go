package main

import (
	"bytes"
	"context"
	_ "embed"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/slinkygg/loader/update"
	"golang.org/x/sys/windows"
)

//go:embed tweaker/SlinkyTweaker
var tweakerLibrary []byte

const tweakerLibraryName = "SlinkyTweaker"

func withTweakerLibrary(ctx context.Context, load func(string) error) error {
	root := filepath.Join(os.TempDir(), "SlinkyTweaker")
	if err := os.MkdirAll(root, 0o700); err != nil {
		return fmt.Errorf("creating temporary tweaker folder: %w", err)
	}
	unlock, err := lockTweakerTemp(ctx, root)
	if err != nil {
		return err
	}
	defer unlock()

	entries, err := os.ReadDir(root)
	if err != nil {
		return fmt.Errorf("reading temporary tweaker folder: %w", err)
	}
	for _, entry := range entries {
		if entry.IsDir() && strings.HasPrefix(entry.Name(), "load-") {
			_ = os.RemoveAll(filepath.Join(root, entry.Name()))
		}
	}
	dir, err := os.MkdirTemp(root, "load-")
	if err != nil {
		return fmt.Errorf("creating temporary tweaker copy: %w", err)
	}
	defer os.RemoveAll(dir)
	path := filepath.Join(dir, tweakerLibraryName)
	if err := update.WriteToFile(ctx, bytes.NewReader(tweakerLibrary), path); err != nil {
		return fmt.Errorf("writing temporary tweaker copy: %w", err)
	}
	if err := ctx.Err(); err != nil {
		return err
	}
	return load(path)
}

func lockTweakerTemp(ctx context.Context, root string) (func(), error) {
	f, err := os.OpenFile(filepath.Join(root, ".lock"), os.O_CREATE|os.O_RDWR, 0o600)
	if err != nil {
		return nil, fmt.Errorf("opening temporary tweaker lock: %w", err)
	}
	var overlapped windows.Overlapped
	for {
		if err := ctx.Err(); err != nil {
			f.Close()
			return nil, err
		}
		err := windows.LockFileEx(windows.Handle(f.Fd()), windows.LOCKFILE_EXCLUSIVE_LOCK|windows.LOCKFILE_FAIL_IMMEDIATELY, 0, 1, 0, &overlapped)
		if err == nil {
			return func() {
				_ = windows.UnlockFileEx(windows.Handle(f.Fd()), 0, 1, 0, &overlapped)
				f.Close()
			}, nil
		}
		if !errors.Is(err, windows.ERROR_LOCK_VIOLATION) {
			f.Close()
			return nil, fmt.Errorf("locking temporary tweaker folder: %w", err)
		}
		select {
		case <-ctx.Done():
			f.Close()
			return nil, ctx.Err()
		case <-time.After(100 * time.Millisecond):
		}
	}
}
