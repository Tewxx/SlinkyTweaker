package update

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
)

func WriteToFile(ctx context.Context, reader io.Reader, path string) error {
	tmp := path + ".tmp"
	old := path + ".old"
	removeTemp := true
	defer func() {
		if removeTemp {
			_ = os.Remove(tmp)
		}
	}()

	mode := os.FileMode(0o666)
	if info, err := os.Stat(path); err == nil {
		mode = info.Mode()
	}
	if err := writeToFile(ctx, reader, tmp, mode); err != nil {
		return err
	}

	renameErr := os.Rename(path, old)
	if renameErr != nil && !errors.Is(renameErr, os.ErrNotExist) {
		return fmt.Errorf("renaming file '%s': %w", path, renameErr)
	}
	if err := os.Rename(tmp, path); err != nil {
		_ = os.Rename(old, path)
		return fmt.Errorf("renaming file '%s': %s", tmp, err)
	}
	removeTemp = false
	if renameErr == nil {
		if err := os.Remove(old); err != nil {
			_ = deleteOnReboot(old)
		}
	}
	return nil
}

func writeToFile(ctx context.Context, reader io.Reader, path string, mode os.FileMode) (err error) {
	f, err := os.OpenFile(path, os.O_CREATE|os.O_RDWR|os.O_TRUNC, mode)
	if err != nil {
		return fmt.Errorf("creating file '%s': %w", path, err)
	}
	defer func() {
		closeErr := f.Close()
		if err == nil && closeErr != nil {
			err = fmt.Errorf("closing file '%s': %w", path, closeErr)
		}
	}()
	return copyContext(ctx, reader, f)
}

func copyContext(ctx context.Context, src io.Reader, dst io.Writer) error {
	done := make(chan error, 1)
	go func() {
		_, err := io.Copy(dst, src)
		done <- err
	}()
	select {
	case <-ctx.Done():
		return ctx.Err()
	case err := <-done:
		return err
	}
}
