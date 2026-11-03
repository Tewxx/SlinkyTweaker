//go:build windows

package update

import (
	"fmt"

	"golang.org/x/sys/windows"
)

const movefileDelayUntilReboot = 0x4

func deleteOnReboot(path string) error {
	p, err := windows.UTF16PtrFromString(path)
	if err != nil {
		return fmt.Errorf("converting path: %w", err)
	}
	return windows.MoveFileEx(p, nil, movefileDelayUntilReboot)
}
