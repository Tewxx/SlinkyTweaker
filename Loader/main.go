package main

import (
	"bytes"
	"context"
	"crypto/ecdsa"
	"crypto/sha256"
	"crypto/x509"
	"encoding/hex"
	"encoding/pem"
	"errors"
	"fmt"
	"io"
	"os"
	"os/signal"
	"path/filepath"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/slinkygg/loader/animation"
	"github.com/slinkygg/loader/api"
	"github.com/slinkygg/loader/tui"
	"github.com/slinkygg/loader/update"
)

const loaderFPS = 5

var (
	apiBaseURL            = "https://api.slinky.gg"
	slinkyLibraryResource = "lib_beta_win"
	slinkyLibraryFilename = "slinky_beta"
	allowedKeys           []*ecdsa.PublicKey
)

var embeddedPublicKeys = []string{
	`-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEC9DeLqoDej3l38NqmwsFr2xfNxjc
F9NeyFDGhAIWo76kVwXH8Pcbr5YYml/IW+pD0LbwpX7FlMRjw2cMErVmSw==
-----END PUBLIC KEY-----
`,
	`-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAETFhymMZvqy8ZGspvyB9SY/MWU2xg
mAayy4ANAlaf/f1oFea3EUGdGv8+Ct8sfbCnHXl9w9THMPvO2/d5U7aV9Q==
-----END PUBLIC KEY-----
`,
}

func main() {
	if err := runLoader(); err != nil && !errors.Is(err, context.Canceled) {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func runLoader() error {
	if apiBaseURL == "" || slinkyLibraryFilename == "" {
		panic("missing fileNameLibrary param")
	}
	for _, data := range embeddedPublicKeys {
		key, err := decodeKeyData([]byte(data))
		if err != nil {
			panic(err)
		}
		allowedKeys = append(allowedKeys, key)
	}
	defer setConsoleTitle()()
	restoreInput, err := prepareLicenseInput(os.Stdin)
	if err != nil {
		return err
	}
	defer restoreInput()
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	a := newApp(animation.RandomNormal(), animation.RandomError())
	a.step = a.runStepLicense
	go a.run(ctx)
	err = tui.Run(ctx, a, loaderFPS)
	if err == errLoadedSuccessfully {
		err = nil
	} else if !errors.Is(err, context.Canceled) {
		select {
		case <-ctx.Done():
		case <-time.NewTimer(3 * time.Second).C:
		}
	}
	return err
}

func decodeKeyData(data []byte) (*ecdsa.PublicKey, error) {
	block, _ := pem.Decode(data)
	if block == nil || block.Type != "PUBLIC KEY" {
		return nil, errors.New("invalid public key")
	}
	pub, err := x509.ParsePKIXPublicKey(block.Bytes)
	if err != nil {
		return nil, fmt.Errorf("parsing public key bytes: %w", err)
	}
	key, ok := pub.(*ecdsa.PublicKey)
	if !ok {
		return nil, fmt.Errorf("expected ECDSA key, got %T", pub)
	}
	return key, nil
}

type authenticatingReader struct {
	counter     *atomic.Uint64
	input       io.Reader
	buf         bytes.Buffer
	allowedKeys []*ecdsa.PublicKey
	sig         []byte
}

func (r *authenticatingReader) Read(p []byte) (int, error) {
	n, err := r.input.Read(p)
	r.buf.Write(p[:n])
	r.counter.Add(uint64(n))
	if err == io.EOF {
		if verifyErr := r.verify(); verifyErr != nil {
			err = verifyErr
		}
	}
	return n, err
}

func (r *authenticatingReader) verify() error {
	h := sha256.New()
	r.buf.WriteTo(h)
	digest := h.Sum(nil)
	r.buf.Reset()
	for _, key := range r.allowedKeys {
		if ecdsa.VerifyASN1(key, digest, r.sig) {
			return nil
		}
	}
	return errSignatureInvalid
}

func computeSHA256(path string) (string, error) {
	f, err := os.Open(path)
	if errors.Is(err, os.ErrNotExist) {
		return "", nil
	}
	if err != nil {
		return "", fmt.Errorf("opening file %s: %w", path, err)
	}
	defer f.Close()
	h := sha256.New()
	if _, err := io.Copy(h, f); err != nil {
		return "", fmt.Errorf("hashing file: %w", err)
	}
	return hex.EncodeToString(h.Sum(nil)), nil
}

func isLocalFileOutdated(ctx context.Context, client *api.Client, job *updateJob) (bool, error) {
	local, err := computeSHA256(job.fileName)
	if err != nil {
		return false, fmt.Errorf("computing checksum: %w", err)
	}
	if local == "" {
		return true, nil
	}
	body, _, err := client.GetFile(ctx, job.resourceName+".sha256", false)
	if err != nil {
		return false, fmt.Errorf("downloading checksum for %s: %w", job.resourceName, err)
	}
	defer body.Close()
	remote := make([]byte, 64)
	if _, err := io.ReadAtLeast(body, remote, 64); err != nil {
		return false, fmt.Errorf("reading remote checksum: %w", err)
	}
	return local != string(remote), nil
}

func runUpdateJob(ctx context.Context, client *api.Client, job *updateJob) (err error) {
	outdated, err := isLocalFileOutdated(ctx, client, job)
	if err != nil {
		return fmt.Errorf("checking for update %s: %w", job.resourceName, err)
	}
	if !outdated {
		return nil
	}
	time.Sleep(2 * time.Second)
	sigBody, _, err := client.GetFile(ctx, job.resourceName+".sig", false)
	if err != nil {
		return fmt.Errorf("downloading signature for %s: %w", job.resourceName, err)
	}
	defer sigBody.Close()
	sig, err := io.ReadAll(sigBody)
	if err != nil {
		return fmt.Errorf("reading signature for %s: %w", job.resourceName, err)
	}
	body, total, err := client.GetFile(ctx, job.resourceName, false)
	if err != nil {
		return fmt.Errorf("downloading %s: %w", job.resourceName, err)
	}
	defer func() {
		closeErr := body.Close()
		if err == nil && closeErr != nil {
			err = fmt.Errorf("finalizing %s: %w", job.resourceName, closeErr)
		}
	}()
	job.bytesTotal.Store(total)
	ar := &authenticatingReader{counter: &job.bytesRead, input: body, allowedKeys: allowedKeys, sig: sig}
	if err := update.WriteToFile(ctx, ar, job.fileName); err != nil {
		return fmt.Errorf("updating %s: %w", job.resourceName, err)
	}
	return nil
}

func appendSlinkyDir(parts ...string) ([]string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return nil, fmt.Errorf("getting home dir: %w", err)
	}
	return append(parts, home, ".slinky"), nil
}

func getSlinkyDirFilePath(name string, createDirs bool) (string, error) {
	parts, err := appendSlinkyDir()
	if err != nil {
		return "", err
	}
	p := filepath.Join(append(parts, name)...)
	if createDirs {
		if err := os.MkdirAll(filepath.Dir(p), 0o770); err != nil {
			return "", fmt.Errorf("creating directories: %w", err)
		}
	}
	return p, nil
}

func openSlinkyDirFile(name string) (*os.File, error) {
	p, err := getSlinkyDirFilePath(name, false)
	if err != nil {
		return nil, err
	}
	return os.Open(p)
}
