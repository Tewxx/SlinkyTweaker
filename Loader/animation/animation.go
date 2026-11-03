package animation

import (
	"embed"
	"encoding/json"
	"fmt"
	"io/fs"
	"math/rand"
	"path"
)

//go:embed animations
var files embed.FS

type Animation struct {
	Name     string   `json:"name"`
	FPS      int      `json:"fps"`
	Frames   []string `json:"frames"`
	Bars     []string `json:"bars"`
	LoopFrom int      `json:"loopFrom"`
}

func (a *Animation) Frame(tick int) string { return a.Frames[a.index(tick, len(a.Frames))] }

func (a *Animation) Bar(tick int) string {
	if len(a.Bars) == 0 {
		return ""
	}
	return a.Bars[a.index(tick, len(a.Bars))]
}

func (a *Animation) Static() bool { return len(a.Frames) == 1 && len(a.Bars) <= 1 }

func (a *Animation) index(tick, n int) int {
	if tick < n || a.LoopFrom >= n {
		return min(tick, n-1)
	}
	return a.LoopFrom + (tick-a.LoopFrom)%(n-a.LoopFrom)
}

var (
	Normal, Error           []*Animation
	Original, OriginalError *Animation
	Bars                    []string
)

func RandomNormal() *Animation { return Normal[rand.Intn(len(Normal))] }

func RandomError() *Animation { return Error[rand.Intn(len(Error))] }

func init() {
	Normal, Original = mustLoadDir("normal", "original")
	Error, OriginalError = mustLoadDir("error", "error")

	seq := make([]byte, 0, 25)
	for i := 0; i < 25; i++ {
		if rand.Intn(2) == 0 {
			seq = append(seq, '_')
		} else {
			seq = append(seq, '-')
		}
	}
	s := string(seq)
	for i := 0; i < 30; i++ {
		Bars = append(Bars, s)
		s = s[1:] + s[:1]
	}
}

func mustLoadDir(dir, original string) (all []*Animation, orig *Animation) {
	dir = path.Join("animations", dir)
	entries, err := fs.ReadDir(files, dir)
	if err != nil {
		panic(err)
	}
	for _, entry := range entries {
		data, err := files.ReadFile(path.Join(dir, entry.Name()))
		if err != nil {
			panic(err)
		}
		a := new(Animation)
		if err := json.Unmarshal(data, a); err != nil {
			panic(fmt.Errorf("%s: %w", entry.Name(), err))
		}
		if len(a.Frames) == 0 || a.FPS <= 0 {
			panic(fmt.Errorf("%s: needs frames and a positive fps", entry.Name()))
		}
		all = append(all, a)
		if entry.Name() == original+".json" {
			orig = a
		}
	}
	if orig == nil {
		panic(fmt.Errorf("%s: missing %s.json", dir, original))
	}
	return all, orig
}
