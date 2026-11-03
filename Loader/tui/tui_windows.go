//go:build windows

package tui

import (
	"fmt"

	"golang.org/x/sys/windows"
)

func platformInit(originalMode *uint32) error {
	if err := windows.GetConsoleMode(windows.Stdout, originalMode); err != nil {
		return fmt.Errorf("getting console mode: %w", err)
	}
	return windows.SetConsoleMode(windows.Stdout, *originalMode|windows.ENABLE_VIRTUAL_TERMINAL_PROCESSING)
}

func platformRestore(originalMode uint32) {
	_ = windows.SetConsoleMode(windows.Stdout, originalMode)
}
