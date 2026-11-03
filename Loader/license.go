package main

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

const licenseKeyFilename = "key"

func loadLicenseKey(ctx context.Context, input io.Reader, prompt func(string) error) (string, error) {
	path, err := getSlinkyDirFilePath(licenseKeyFilename, false)
	if err != nil {
		return "", err
	}
	return loadOrPromptLicenseKey(ctx, path, input, prompt)
}

func normalizeLicenseKey(text string) (string, bool) {
	key := strings.ToUpper(strings.TrimSpace(text))
	if len(key) != 32 {
		return "", false
	}
	for _, c := range key {
		if !((c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9')) {
			return "", false
		}
	}
	return key, true
}

type licenseRune struct {
	value rune
	err   error
}

func licenseInputReader(ctx context.Context, input io.Reader) func() (rune, error) {
	requests := make(chan struct{})
	results := make(chan licenseRune)
	go func() {
		reader := bufio.NewReader(input)
		for {
			select {
			case <-ctx.Done():
				return
			case <-requests:
			}
			value, _, err := reader.ReadRune()
			select {
			case results <- licenseRune{value, err}:
			case <-ctx.Done():
				return
			}
			if err != nil {
				return
			}
		}
	}()
	return func() (rune, error) {
		select {
		case requests <- struct{}{}:
		case <-ctx.Done():
			return 0, ctx.Err()
		}
		select {
		case result := <-results:
			return result.value, result.err
		case <-ctx.Done():
			return 0, ctx.Err()
		}
	}
}

func loadOrPromptLicenseKey(ctx context.Context, path string, input io.Reader, prompt func(string) error) (string, error) {
	if err := ctx.Err(); err != nil {
		return "", err
	}
	data, err := os.ReadFile(path)
	notice := ""
	if err == nil {
		if key, valid := normalizeLicenseKey(string(data)); valid {
			return key, nil
		}
		notice = "Saved license key is invalid. Enter a replacement.\n\n"
	} else if !errors.Is(err, os.ErrNotExist) {
		return "", fmt.Errorf("reading saved key: %w", err)
	}

	inputCtx, cancel := context.WithCancel(ctx)
	defer cancel()
	readRune := licenseInputReader(inputCtx, input)
	var entered []rune
	showPrompt := func() error {
		return prompt(notice + "Enter your Slinky license key:\n> " + strings.Repeat("*", len(entered)) + "\n\nPress Enter to continue.")
	}
	if err := showPrompt(); err != nil {
		return "", err
	}
	for {
		value, err := readRune()
		if err != nil && err != io.EOF {
			return "", fmt.Errorf("reading license key: %w", err)
		}
		if err == nil && value == 3 {
			return "", context.Canceled
		}
		submit := err == io.EOF || value == '\r' || value == '\n'
		if submit {
			key, valid := normalizeLicenseKey(string(entered))
			if valid {
				if err := ctx.Err(); err != nil {
					return "", err
				}
				if err := os.MkdirAll(filepath.Dir(path), 0o770); err != nil {
					return "", fmt.Errorf("creating key directory: %w", err)
				}
				if err := os.WriteFile(path, []byte(key+"\n"), 0o600); err != nil {
					return "", fmt.Errorf("saving license key: %w", err)
				}
				return key, nil
			}
			if err == io.EOF {
				return "", errors.New("no license key entered")
			}
			notice = "Use a 32-character key containing letters A-Z and digits 0-9.\n\n"
			entered = nil
		} else {
			switch value {
			case '\b', 127:
				if len(entered) > 0 {
					entered = entered[:len(entered)-1]
				}
			case 21:
				entered = nil
			default:
				if value >= 32 {
					if len(entered) >= 256 {
						return "", errors.New("license key input is too long")
					}
					entered = append(entered, value)
				}
			}
		}
		if err := showPrompt(); err != nil {
			return "", err
		}
	}
}
