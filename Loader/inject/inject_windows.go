//go:build windows

package inject

import (
	"errors"
	"fmt"
	"path/filepath"
	"strings"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows"
)

const (
	processAccess = 0x43A
	memRelease    = 0x8000
	stillActive   = 259
)

var (
	errMinecraftNotFound         = errors.New("minecraft not found")
	errModuleNotFound            = errors.New("module not found")
	user32                       = windows.MustLoadDLL("user32.dll")
	procFindWindowW              = user32.MustFindProc("FindWindowW")
	procGetWindowThreadProcessId = user32.MustFindProc("GetWindowThreadProcessId")
	kernel32                     = windows.MustLoadDLL("kernel32.dll")
	procOpenProcess              = kernel32.MustFindProc("OpenProcess")
	procCloseHandle              = kernel32.MustFindProc("CloseHandle")
	procVirtualAllocEx           = kernel32.MustFindProc("VirtualAllocEx")
	procVirtualFreeEx            = kernel32.MustFindProc("VirtualFreeEx")
	procWriteProcessMemory       = kernel32.MustFindProc("WriteProcessMemory")
	procReadProcessMemory        = kernel32.MustFindProc("ReadProcessMemory")
	procCreateRemoteThread       = kernel32.MustFindProc("CreateRemoteThread")
	procLoadLibraryW             = kernel32.MustFindProc("LoadLibraryW")
	procWaitForSingleObject      = kernel32.MustFindProc("WaitForSingleObject")
	procGetExitCodeThread        = kernel32.MustFindProc("GetExitCodeThread")
	procGetExitCodeProcess       = kernel32.MustFindProc("GetExitCodeProcess")
)

func findMinecraftWindowFromClass(class string) (uintptr, error) {
	c, err := windows.UTF16PtrFromString(class)
	if err != nil {
		return 0, err
	}
	hwnd, _, _ := procFindWindowW.Call(uintptr(unsafe.Pointer(c)), 0)
	if hwnd == 0 {
		return 0, errMinecraftNotFound
	}
	return hwnd, nil
}

func FindMinecraft() (Process, error) {
	var p Process
	hwnd, err := findMinecraftWindowFromClass("LWJGL")
	if err == errMinecraftNotFound {
		hwnd, err = findMinecraftWindowFromClass("GLFW30")
		if err != nil {
			return p, err
		}
	}
	if result, _, err := procGetWindowThreadProcessId.Call(hwnd, uintptr(unsafe.Pointer(&p.pid))); result == 0 {
		return p, err
	}
	return p, nil
}

func (p Process) Inject(libraryPath, authBlob string) error {
	h, _, err := procOpenProcess.Call(processAccess, 0, uintptr(p.pid))
	if h == 0 {
		if errors.Is(err, windows.ERROR_ACCESS_DENIED) {
			return ErrAccessDenied
		}
		return fmt.Errorf("accessing process: %w", err)
	}
	defer procCloseHandle.Call(h)
	base, err := p.findLibBase(libraryPath)
	if err != nil {
		if err != errModuleNotFound {
			return fmt.Errorf("error while locating module: %w", err)
		}
		if err := p.loadLib(windows.Handle(h), libraryPath); err != nil {
			return fmt.Errorf("failed to load library: %w", err)
		}
		base, err = p.findLibBase(libraryPath)
		if err != nil {
			return fmt.Errorf("failed to find module: %w", err)
		}
	}
	if err := p.init(windows.Handle(h), base, authBlob); err != nil {
		return fmt.Errorf("initializing library: %w", err)
	}
	return nil
}

func (p Process) IsLoaded(libraryPath string) (bool, error) {
	_, err := p.findLibBase(libraryPath)
	if err == errModuleNotFound {
		return false, nil
	}
	return err == nil, err
}

func (p Process) Load(libraryPath string) error {
	h, _, err := procOpenProcess.Call(processAccess, 0, uintptr(p.pid))
	if h == 0 {
		if errors.Is(err, windows.ERROR_ACCESS_DENIED) {
			return ErrAccessDenied
		}
		return fmt.Errorf("accessing process: %w", err)
	}
	defer procCloseHandle.Call(h)
	if _, err := p.findLibBase(libraryPath); err != errModuleNotFound {
		if err != nil {
			return fmt.Errorf("error while locating module: %w", err)
		}
		return nil
	}
	if err := p.loadLib(windows.Handle(h), libraryPath); err != nil {
		return fmt.Errorf("failed to load library: %w", err)
	}
	if _, err := p.findLibBase(libraryPath); err != nil {
		return fmt.Errorf("library did not load: %w", err)
	}
	return nil
}

func (p Process) loadLib(h windows.Handle, libraryPath string) error {
	if !strings.ContainsRune(filepath.Base(libraryPath), '.') {
		libraryPath += "."
	}
	utf16, err := syscall.UTF16FromString(libraryPath)
	if err != nil {
		return err
	}
	size := uintptr(len(libraryPath) * 2)
	addr, _, err := procVirtualAllocEx.Call(uintptr(h), 0, size+2, 0x3000, 4)
	if addr == 0 {
		return fmt.Errorf("allocating buffer: %w", err)
	}
	defer procVirtualFreeEx.Call(uintptr(h), addr, 0, memRelease)
	if ok, _, err := procWriteProcessMemory.Call(uintptr(h), addr, uintptr(unsafe.Pointer(&utf16[0])), size, 0); ok == 0 {
		return fmt.Errorf("writing buffer: %w", err)
	}
	thread, _, err := procCreateRemoteThread.Call(uintptr(h), 0, 0, procLoadLibraryW.Addr(), addr, 0, 0)
	if thread == 0 {
		return fmt.Errorf("loading library: %w", err)
	}
	defer procCloseHandle.Call(thread)
	procWaitForSingleObject.Call(thread, windows.INFINITE)
	return nil
}

func getExitError(process, thread windows.Handle) error {
	var processCode uint32
	if ok, _, err := procGetExitCodeProcess.Call(uintptr(process), uintptr(unsafe.Pointer(&processCode))); ok == 0 {
		return fmt.Errorf("reading process state: %w", err)
	}
	if processCode != stillActive {
		return CrashError(fmt.Sprintf("process exited: %d", processCode))
	}
	var threadCode int32
	if ok, _, err := procGetExitCodeThread.Call(uintptr(thread), uintptr(unsafe.Pointer(&threadCode))); ok == 0 {
		return fmt.Errorf("reading exit code: %w", err)
	}
	if threadCode != 0 {
		return LibraryError(threadCode)
	}
	return nil
}

func (p Process) init(h windows.Handle, base uintptr, authBlob string) error {
	fn, err := p.findInitFunc(h, base)
	if err != nil {
		return err
	}
	utf16, err := syscall.UTF16FromString(authBlob)
	if err != nil {
		return err
	}
	size := uintptr(len(authBlob) * 2)
	addr, _, err := procVirtualAllocEx.Call(uintptr(h), 0, size+2, 0x3000, 4)
	if addr == 0 {
		return fmt.Errorf("allocating buffer: %w", err)
	}
	defer procVirtualFreeEx.Call(uintptr(h), addr, 0, memRelease)
	if ok, _, err := procWriteProcessMemory.Call(uintptr(h), addr, uintptr(unsafe.Pointer(&utf16[0])), size, 0); ok == 0 {
		return fmt.Errorf("writing buffer: %w", err)
	}
	thread, _, err := procCreateRemoteThread.Call(uintptr(h), 0, 0, fn, addr, 0, 0)
	if thread == 0 {
		return fmt.Errorf("initializing library: %w", err)
	}
	defer procCloseHandle.Call(thread)
	procWaitForSingleObject.Call(thread, windows.INFINITE)
	return getExitError(h, windows.Handle(thread))
}

func (p Process) findLibBase(libraryPath string) (uintptr, error) {
	var snap windows.Handle
	for i := 0; i < 100; i++ {
		var err error
		snap, err = windows.CreateToolhelp32Snapshot(windows.TH32CS_SNAPMODULE, p.pid)
		if err == windows.ERROR_BAD_LENGTH {
			continue
		}
		if err != nil {
			return 0, fmt.Errorf("snapshot failed: %w", err)
		}
		break
	}
	defer windows.CloseHandle(snap)
	entry := windows.ModuleEntry32{Size: uint32(unsafe.Sizeof(windows.ModuleEntry32{}))}
	if err := windows.Module32First(snap, &entry); err != nil {
		return 0, fmt.Errorf("iterating modules: %w", err)
	}
	for {
		if strings.HasSuffix(libraryPath, syscall.UTF16ToString(entry.Module[:])) {
			if entry.ModBaseAddr == 0 {
				return 0, errModuleNotFound
			}
			return entry.ModBaseAddr, nil
		}
		if err := windows.Module32Next(snap, &entry); err != nil {
			if err == windows.ERROR_NO_MORE_FILES {
				return 0, errModuleNotFound
			}
			return 0, fmt.Errorf("iterating modules: %w", err)
		}
	}
}

func (p Process) findInitFunc(h windows.Handle, base uintptr) (uintptr, error) {
	var header int32
	if ok, _, err := procReadProcessMemory.Call(uintptr(h), base+0x3c, uintptr(unsafe.Pointer(&header)), 4, 0); ok == 0 {
		return 0, fmt.Errorf("reading header VA: %w", err)
	}
	var exportDir uint32
	if ok, _, err := procReadProcessMemory.Call(uintptr(h), base+uintptr(header)+0x88, uintptr(unsafe.Pointer(&exportDir)), 4, 0); ok == 0 {
		return 0, fmt.Errorf("reading export dir VA: %w", err)
	}
	var functions uint32
	if ok, _, err := procReadProcessMemory.Call(uintptr(h), base+uintptr(exportDir)+0x1c, uintptr(unsafe.Pointer(&functions)), 4, 0); ok == 0 {
		return 0, fmt.Errorf("reading export dir functions: %w", err)
	}
	var firstExport uint32
	if ok, _, err := procReadProcessMemory.Call(uintptr(h), base+uintptr(functions), uintptr(unsafe.Pointer(&firstExport)), 4, 0); ok == 0 {
		return 0, fmt.Errorf("reading export VA: %w", err)
	}
	return base + uintptr(firstExport), nil
}
