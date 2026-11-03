//go:build windows

package main

import (
	"fmt"
	"os"
	"unsafe"

	"golang.org/x/sys/windows"
)

const consoleTitle = "Slinky Tweaker"

var (
	kernel32             = windows.NewLazySystemDLL("kernel32.dll")
	procGetConsoleTitleW = kernel32.NewProc("GetConsoleTitleW")
	procSetConsoleTitleW = kernel32.NewProc("SetConsoleTitleW")
)

func prepareLicenseInput(input *os.File) (func(), error) {
	handle := windows.Handle(input.Fd())
	var originalMode uint32
	if err := windows.GetConsoleMode(handle, &originalMode); err != nil {
		return func() {}, nil
	}
	mode := originalMode &^ (windows.ENABLE_LINE_INPUT | windows.ENABLE_ECHO_INPUT | windows.ENABLE_QUICK_EDIT_MODE)
	mode |= windows.ENABLE_PROCESSED_INPUT | windows.ENABLE_EXTENDED_FLAGS
	if err := windows.SetConsoleMode(handle, mode); err != nil {
		return nil, fmt.Errorf("preparing license input: %w", err)
	}
	return func() { _ = windows.SetConsoleMode(handle, originalMode) }, nil
}

func setConsoleTitle() func() {
	previous := make([]uint16, 1024)
	n, _, _ := procGetConsoleTitleW.Call(uintptr(unsafe.Pointer(&previous[0])), uintptr(len(previous)))
	title, _ := windows.UTF16PtrFromString(consoleTitle)
	procSetConsoleTitleW.Call(uintptr(unsafe.Pointer(title)))
	if n == 0 {
		return func() {}
	}
	return func() { procSetConsoleTitleW.Call(uintptr(unsafe.Pointer(&previous[0]))) }
}
