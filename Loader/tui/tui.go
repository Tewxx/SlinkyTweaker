package tui

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"os"
	"time"
)

type Renderer interface {
	Render(ctx context.Context, buf *bytes.Buffer) error
}

type stopRenderError string

func (e stopRenderError) Error() string { return string(e) }

var ErrStopRender error = stopRenderError("stop rendering")

const (
	enterAltScreen = "\x1b[?1049h\x1b[?25l"
	leaveAltScreen = "\x1b[?1049l\x1b[?25h"
	clearScreen    = "\x1b[0m\x1b[1J\x1b[H"
)

func Run(ctx context.Context, r Renderer, fps int) error {
	var originalMode uint32
	if err := platformInit(&originalMode); err != nil {
		return err
	}
	defer platformRestore(originalMode)
	if _, err := os.Stdout.WriteString(enterAltScreen); err != nil {
		return fmt.Errorf("enabling alternative buffer: %w", err)
	}
	defer os.Stdout.WriteString(leaveAltScreen)

	interval := time.Second / time.Duration(fps)
	ticker := time.NewTicker(interval)
	defer ticker.Stop()

	return renderFrames(ctx, r, os.Stdout, ticker.C)
}

func renderFrames(ctx context.Context, r Renderer, out io.Writer, ticks <-chan time.Time) error {
	var buf bytes.Buffer
	for {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}
		buf.WriteString(clearScreen)
		err := r.Render(ctx, &buf)
		_, _ = buf.WriteTo(out)
		buf.Reset()
		if err != nil {
			if err == ErrStopRender {
				<-ctx.Done()
				return ctx.Err()
			}
			return err
		}
		<-ticks
	}
}
