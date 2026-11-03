package inject

import (
	"errors"
	"fmt"
)

var ErrAccessDenied = errors.New("access denied")

type LibraryError int32

func (e LibraryError) Error() string { return fmt.Sprintf("slinky error %d", int32(e)) }

type CrashError string

func (e CrashError) Error() string { return fmt.Sprintf("crash detected: %s", string(e)) }

type Process struct {
	pid uint32
}
