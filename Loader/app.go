package main

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/slinkygg/loader/animation"
	"github.com/slinkygg/loader/api"
	"github.com/slinkygg/loader/inject"
	"github.com/slinkygg/loader/tui"
)

type stepFunc func(context.Context) (stepFunc, error)

type updateJob struct {
	prettyName   string
	fileName     string
	resourceName string
	done         bool
	err          error
	bytesRead    atomic.Uint64
	bytesTotal   atomic.Int64
}

type updateJobResult struct {
	job *updateJob
	err error
}

const (
	colorOK           = "\x1b[38;5;208m"
	colorErr          = "\x1b[91m"
	colorLicense      = "\x1b[38;5;247m"
	colorReset        = "\x1b[0m"
	titleInitializing = "INITIALIZING"
	titleLicenseKey   = "LICENSE KEY"
	titleUpdating     = "DOWNLOADING UPDATES"
	titleInjecting    = "INJECTING"
	titleTweaker      = "LOADING SLINKY TWEAKER"
	titleDone         = "DONE"
	titleError        = "ERROR"
	msgInject         = "Injecting into Minecraft..."
	msgTweaker        = "Loading Slinky Tweaker into Minecraft..."
	msgNotAll         = "\nNot all updates succeeded. Slinky will attempt to continue anyway."
	successFormat     = "Injected successfully. Loader will close in %s%ds%s."
	licenseFormat     = "%sYour license expires in %s%s\n\n"
	supportText       = "If you need further support, please visit: https://slinky.gg/contact/"
)

type app struct {
	step             stepFunc
	licenseKey       string
	title            string
	message          string
	licenseRemaining time.Duration
	error            bool
	frameCounter     int
	errorFrame       int
	anim, errorAnim  *animation.Animation
	jobs             chan func() error
}

func newApp(anim, errorAnim *animation.Animation) *app {
	return &app{title: titleInitializing, anim: anim, errorAnim: errorAnim, jobs: make(chan func() error, 16)}
}

func (a *app) run(ctx context.Context) {
	var err error
	defer func() {
		if err != nil {
			a.jobs <- func() error { return err }
		}
	}()
	for step := a.step; step != nil; {
		select {
		case <-ctx.Done():
			return
		default:
		}
		a.jobs <- func() error { a.error = false; return nil }
		step, err = step(ctx)
		if err != nil {
			return
		}
	}
}

func (a *app) Render(ctx context.Context, buf *bytes.Buffer) error {
	if err := a.executeJobs(); err != nil {
		if err == errLoadedSuccessfully || errors.Is(err, context.Canceled) {
			return err
		}
		a.displayError(err)
	}
	if !a.error {
		a.frameCounter++
		a.writeArt(buf, a.anim, a.frameCounter)
	} else {
		a.writeArt(buf, a.errorAnim, a.errorFrame)
		a.errorFrame++
	}
	buf.WriteString("\n\n> ")
	if a.error {
		buf.WriteString(colorErr)
	} else {
		buf.WriteString(colorOK)
	}
	buf.WriteString(a.title)
	buf.WriteString(colorReset)
	buf.WriteString("\n\n")
	if a.licenseRemaining > 0 {
		fmt.Fprintf(buf, licenseFormat, colorLicense, humanDuration(a.licenseRemaining), colorReset)
	}
	buf.WriteString(a.message)
	buf.WriteRune('\n')
	if a.error && a.errorAnim.Static() {
		return tui.ErrStopRender
	}
	return nil
}

func (a *app) writeArt(buf *bytes.Buffer, anim *animation.Animation, n int) {
	tick := n * anim.FPS / loaderFPS
	buf.WriteString(anim.Frame(tick))
	buf.WriteByte('\n')
	bar := anim.Bar(tick)
	if bar == "" {
		bar = animation.Bars[a.frameCounter%len(animation.Bars)]
	}
	buf.WriteString(bar)
}

func (a *app) executeJobs() error {
	for {
		select {
		case job := <-a.jobs:
			if err := job(); err != nil {
				return err
			}
		default:
			return nil
		}
	}
}

func (a *app) runStepLicense(ctx context.Context) (stepFunc, error) {
	return a.runLicenseStep(ctx, os.Stdin)
}

func (a *app) runLicenseStep(ctx context.Context, input io.Reader) (stepFunc, error) {
	key, err := loadLicenseKey(ctx, input, func(message string) error {
		select {
		case a.jobs <- func() error {
			a.title = titleLicenseKey
			a.message = message
			return nil
		}:
			return nil
		case <-ctx.Done():
			return ctx.Err()
		}
	})
	if err != nil {
		return nil, fmt.Errorf("reading license key: %w", err)
	}
	a.licenseKey = key
	select {
	case a.jobs <- func() error {
		a.title = titleInitializing
		a.message = ""
		return nil
	}:
		return a.runStepUpdate, nil
	case <-ctx.Done():
		return nil, ctx.Err()
	}
}

func (a *app) runStepUpdate(ctx context.Context) (stepFunc, error) {
	client := &api.Client{HttpClient: http.DefaultClient, LicenseKey: a.licenseKey, BaseURL: apiBaseURL}
	go func() {
		if d, err := client.GetLicenseRemainingDuration(ctx); err == nil {
			a.jobs <- func() error { a.licenseRemaining = d; return nil }
			writeLicenseExpiry(d)
		}
	}()
	libraryFile, err := getSlinkyDirFilePath(filepath.Join("bin", slinkyLibraryFilename), true)
	if err != nil {
		return nil, fmt.Errorf("getting library path: %w", err)
	}
	var jobs []*updateJob
	if slinkyLibraryResource != "" {
		jobs = append(jobs, &updateJob{prettyName: "Client", fileName: libraryFile, resourceName: slinkyLibraryResource})
	}
	if len(jobs) == 0 {
		return a.runStepInject, nil
	}
	results := make(chan updateJobResult, len(jobs))
	ctx, cancel := context.WithCancel(ctx)
	defer cancel()
	for _, job := range jobs {
		go func() { results <- updateJobResult{job: job, err: runUpdateJob(ctx, client, job)} }()
	}
	ticker := time.NewTicker(250 * time.Millisecond)
	defer ticker.Stop()
	failed := false
	remaining := len(jobs)
	for remaining > 0 {
		select {
		case result := <-results:
			result.job.done = true
			if result.err != nil {
				var apiErr api.Error
				if errors.As(result.err, &apiErr) && (apiErr.StatusCode == 401 || apiErr.StatusCode == 403) {
					return nil, result.err
				}
				result.job.err = result.err
				failed = true
			}
			remaining--
		case <-ticker.C:
			var builder strings.Builder
			for _, job := range jobs {
				writeJobToBuilder(job, &builder)
			}
			message := builder.String()
			a.jobs <- func() error { a.title = titleUpdating; a.message = message; return nil }
		}
	}
	if failed {
		var builder strings.Builder
		for _, job := range jobs {
			writeJobToBuilder(job, &builder)
		}
		builder.WriteString(msgNotAll)
		message := builder.String()
		a.jobs <- func() error { a.title = titleUpdating; a.message = message; return nil }
		time.Sleep(3 * time.Second)
	}
	return a.runStepInject, nil
}

func (a *app) runStepInject(ctx context.Context) (stepFunc, error) {
	proc, err := inject.FindMinecraft()
	if err != nil {
		return nil, fmt.Errorf("finding minecraft process: %w", err)
	}
	a.jobs <- func() error { a.title = titleInjecting; a.message = msgInject; return nil }
	f, err := openSlinkyDirFile(filepath.Join("bin", slinkyLibraryFilename))
	if err != nil {
		return nil, fmt.Errorf("opening library: %w", err)
	}
	defer f.Close()
	path, err := filepath.Abs(f.Name())
	if err != nil {
		return nil, fmt.Errorf("getting library path: %w", err)
	}
	if err := proc.Inject(path, a.licenseKey); err != nil {
		return nil, fmt.Errorf("injecting: %w", err)
	}
	return a.runStepTweaker, nil
}

func (a *app) runStepTweaker(ctx context.Context) (stepFunc, error) {
	proc, err := inject.FindMinecraft()
	if err != nil {
		return nil, fmt.Errorf("finding minecraft process: %w", err)
	}
	if loaded, err := proc.IsLoaded(tweakerLibraryName); err != nil {
		return nil, fmt.Errorf("checking for slinky tweaker: %w", err)
	} else if loaded {
		return a.runStepSuccess, nil
	}
	a.jobs <- func() error { a.title = titleTweaker; a.message = msgTweaker; return nil }
	if err := withTweakerLibrary(ctx, proc.Load); err != nil {
		return nil, fmt.Errorf("loading slinky tweaker: %w", err)
	}
	return a.runStepSuccess, nil
}

func (a *app) runStepSuccess(ctx context.Context) (stepFunc, error) {
	ticker := time.NewTicker(time.Second)
	defer ticker.Stop()
	count := 5
	for range ticker.C {
		select {
		case <-ctx.Done():
			return nil, ctx.Err()
		default:
		}
		seconds := count
		a.jobs <- func() error {
			if seconds == 0 {
				return errLoadedSuccessfully
			}
			a.title = titleDone
			a.message = fmt.Sprintf(successFormat, colorOK, seconds, colorReset)
			return nil
		}
		count--
	}
	return nil, nil
}

func writeJobToBuilder(job *updateJob, builder *strings.Builder) {
	builder.WriteString(job.prettyName + " - ")
	if job.done {
		if job.err == nil {
			builder.WriteString("Done\n")
			return
		}
		var errno syscall.Errno
		if errors.As(job.err, &errno) && (errno == 225 || errno == 226) {
			fmt.Fprintf(builder, "%sBlocked by Defender: %v%s\n", colorErr, job.fileName, colorReset)
		} else {
			fmt.Fprintf(builder, "%sFailed: %v%s\n", colorErr, job.err, colorReset)
		}
		return
	}
	read, total := job.bytesRead.Load(), job.bytesTotal.Load()
	mib := float64(read) / (1024 * 1024)
	switch {
	case total == 0:
		builder.WriteString("downloading metadata...\n")
	case total < 0:
		fmt.Fprintf(builder, "%.1f MiB\n", mib)
	default:
		bars := read * 20 / uint64(total)
		builder.WriteByte('[')
		for i := uint64(0); i < bars; i++ {
			builder.WriteRune('|')
		}
		for i := uint64(0); i < 20-bars; i++ {
			builder.WriteRune(' ')
		}
		fmt.Fprintf(builder, "] %.0f%% - %.1f MiB\n", float64(read)*100/float64(total), mib)
	}
}

func (a *app) displayError(err error) {
	a.error = true
	a.errorFrame = 0
	a.title = titleError
	a.message = fmt.Sprintf("%s\n\n%s", getErrorMessage(err), supportText)
}

func getErrorMessage(err error) string {
	var libErr inject.LibraryError
	if errors.As(err, &libErr) {
		switch int32(libErr) {
		case 1002:
			return "Main thread not found. You are likely injecting into an incompatible Minecraft version."
		case 1003:
			return "Incompatible Minecraft version/client detected."
		case 2004:
			return "A Java exception occurred while initializing Slinky."
		case 7003:
			return "Authentication failed"
		case 7004:
			return "Your device appears to be different/modified since your last use.\nPlease contact support if you want to transfer your license to this device."
		}
	}
	if errors.Is(err, inject.ErrAccessDenied) {
		return "Process access denied. Try running Slinky as Administrator."
	}
	var errno syscall.Errno
	if errors.As(err, &errno) && (errno == 225 || errno == 226) {
		return "Blocked by Defender. Please disable it or add an exclusion."
	}
	s := err.Error()
	if s == "" {
		return ""
	}
	return strings.ToUpper(s[:1]) + s[1:]
}

func humanDuration(d time.Duration) string {
	switch {
	case d > 48*time.Hour:
		return fmt.Sprintf("%.0f days", d.Hours()/24)
	case d > 2*time.Hour:
		return fmt.Sprintf("%.0f hours", d.Hours())
	default:
		return fmt.Sprintf("%.0f minutes", d.Minutes())
	}
}
