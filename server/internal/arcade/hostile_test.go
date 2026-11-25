package arcade

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"strings"
	"testing"
	"testing/fstest"
	"time"

	w "github.com/tekne-studio/planet/server/internal/wasmtest"
)

// Runners written by hand to break the judge from the inside: what a
// runner gone wrong, or a file that drives one down its worst path, could
// do to the server. The pinned runner is trusted to judge; the judge
// bounds it all the same.

// The runner's code, by what each export does.
type runnerCode struct {
	engine, file, check, out []byte
	// The imports of module `engine`, by name, each (i32, i32) -> i32.
	imports []string
	// Bytes at address zero of its one page.
	data []byte
}

// keeping is a runner that keeps every file: room for it at 1024, and no
// words for why not.
func keeping() runnerCode {
	return runnerCode{
		engine:  w.I32(Engine),
		file:    w.I32(1024),
		check:   w.I32(0),
		out:     w.I32(0),
		imports: []string{"load"},
	}
}

// The first byte of the file in the room is 0xff.
var firstIsFF = w.Join(w.I32(1024), []byte{0x2d, 0x00, 0x00}, w.I32(0xff), []byte{0x46}) // i32.load8_u; i32.eq

// bytes are the runner as a module. Its imports are functions 0 on, then
// engine, file, check and out.
func (r runnerCode) bytes() []byte {
	const (
		ask  = 0 // () -> i32
		take = 1 // (i32) -> i32
		two  = 2 // (i32, i32) -> i32
	)
	var imports [][]byte
	for _, name := range r.imports {
		module, name, found := strings.Cut(name, ".")
		if !found {
			module, name = "engine", module
		}
		imports = append(imports, w.Join(w.Name(module), w.Name(name), []byte{0x00, two}))
	}
	first := byte(len(r.imports))
	exports := [][]byte{w.Join(w.Name("memory"), []byte{0x02, 0x00})}
	for i, name := range []string{"engine", "file", "check", "out"} {
		exports = append(exports, w.Join(w.Name(name), []byte{0x00, first + byte(i)}))
	}
	sections := [][]byte{
		w.Section(1, w.Vector([]byte{0x60, 0x00, 0x01, 0x7f}, []byte{0x60, 0x01, 0x7f, 0x01, 0x7f}, []byte{0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f})),
		w.Section(2, w.Vector(imports...)),
		w.Section(3, w.Vector([]byte{ask}, []byte{take}, []byte{ask}, []byte{ask})),
		w.Section(5, w.Vector([]byte{0x00, 0x01})),
		w.Section(7, w.Vector(exports...)),
		w.Section(10, w.Vector(w.Body(r.engine), w.Body(r.file), w.Body(r.check), w.Body(r.out))),
	}
	if len(r.data) > 0 {
		// One active segment at address zero: its offset, then its length
		// and its bytes.
		segment := w.Join([]byte{0x00}, w.I32(0), []byte{0x0b}, w.Leb(uint32(len(r.data))), r.data)
		sections = append(sections, w.Section(11, w.Vector(segment)))
	}
	return w.Module(sections...)
}

// hosted is a judge of `runner`, its judgements ending within `within`.
func hosted(t *testing.T, runner runnerCode, within time.Duration) *Judge {
	t.Helper()
	j, err := host(context.Background(), runner.bytes())
	if err != nil {
		t.Fatal(err)
	}
	j.deadline = within
	t.Cleanup(func() { _ = j.Close(context.Background()) })
	return j
}

func TestARunnerThatKeepsTheContractOfAJudgeIsHosted(t *testing.T) {
	j := hosted(t, keeping(), deadline)
	if verdict := verdictOf(t, j, []byte(w.Header)); verdict.Code != "" {
		t.Fatalf("kept: %q", verdict.Code)
	}
	words := keeping()
	words.data, words.check = []byte("nope"), w.I32(4)
	j = hosted(t, words, deadline)
	if verdict := verdictOf(t, j, []byte(w.Header)); verdict.Code != "nope" {
		t.Fatalf("the runner's words are the code: %q", verdict.Code)
	}
}

// Every way a runner can fail a judgement refuses the file with a code of
// the judge's own, within the deadline and the memory's ceiling, and the
// next judgement runs on a fresh instance.
func TestARunnerThatFailsAJudgementRefusesTheFile(t *testing.T) {
	const within = 200 * time.Millisecond
	forever := []byte{0x03, 0x40, 0x0c, 0x00, 0x0b} // loop; br 0; end
	growing := []byte{0x03, 0x40, 0x41, 0x01, 0x40, 0x00, 0x41, 0x7f, 0x47, 0x0d, 0x00, 0x0b, 0x00}
	for _, c := range []struct {
		what  string
		check []byte
		file  []byte
		out   []byte
		code  string
	}{
		// Its check on a file of 0xff, and keeps any other.
		{what: "a check without end", check: w.Join(firstIsFF, []byte{0x04, 0x40}, forever, []byte{0x0b}, w.I32(0)), code: Late},
		{what: "a check that asks the engine for a game", check: w.Join(firstIsFF, []byte{0x04, 0x7f}, w.I32(0), w.I32(0), []byte{0x10, 0x00, 0x05}, w.I32(0), []byte{0x0b}), code: Trapped},
		{what: "a check that traps", check: w.Join(firstIsFF, []byte{0x04, 0x40, 0x00, 0x0b}, w.I32(0)), code: Trapped},
		// Grows a page at a time until it is told no, and then traps, as
		// a runner whose allocation fails does.
		{what: "a check that grows past the ceiling", check: w.Join(firstIsFF, []byte{0x04, 0x40}, growing, []byte{0x0b}, w.I32(0)), code: Trapped},
		{what: "words outside its memory", check: w.Join(firstIsFF, []byte{0x04, 0x7f}, w.I32(100), []byte{0x05}, w.I32(0), []byte{0x0b}), out: w.I32(65530), code: Trapped},
	} {
		runner := keeping()
		runner.check = c.check
		if c.out != nil {
			runner.out = c.out
		}
		j := hosted(t, runner, within)
		began := time.Now()
		verdict := verdictOf(t, j, []byte{0xff})
		took := time.Since(began)
		if verdict.Code != c.code {
			t.Fatalf("%s: %q, not %q", c.what, verdict.Code, c.code)
		}
		if took > within+within/2 {
			t.Fatalf("%s: judged in %v, past the deadline %v", c.what, took, within)
		}
		if verdict.Memory > memoryPages<<16 {
			t.Fatalf("%s: the runner's memory grew to %d bytes, past the ceiling", c.what, verdict.Memory)
		}
		if again := verdictOf(t, j, []byte{0x00}); again.Code != "" || again.Memory != 1<<16 {
			t.Fatalf("%s: the next file is judged on a fresh instance: %q, %d bytes", c.what, again.Code, again.Memory)
		}
	}
}

// The ceiling holds what the runner grows to: told no at it, never past.
func TestTheRunnersMemoryStopsAtTheCeiling(t *testing.T) {
	runner := keeping()
	runner.check = w.Join([]byte{0x03, 0x40, 0x41, 0x01, 0x40, 0x00, 0x41, 0x7f, 0x47, 0x0d, 0x00, 0x0b, 0x00})
	verdict := verdictOf(t, hosted(t, runner, deadline), nil)
	if verdict.Code != Trapped || verdict.Memory != memoryPages<<16 {
		t.Fatalf("%q, the memory at %d bytes, not at the ceiling %d", verdict.Code, verdict.Memory, memoryPages<<16)
	}
}

func TestARunnerPointingItsRoomOutsideItsMemoryRefusesTheFile(t *testing.T) {
	runner := keeping()
	runner.file = w.I32(-4096)
	if verdict := verdictOf(t, hosted(t, runner, deadline), []byte(w.Header)); verdict.Code != Trapped {
		t.Fatalf("%q", verdict.Code)
	}
}

// One judgement at a time: the next waits its turn as long as its asker
// does, and one whose asker leaves first is called off with no verdict.
func TestJudgementsTakeTurns(t *testing.T) {
	runner := keeping()
	runner.check = w.Join(firstIsFF, []byte{0x04, 0x40, 0x03, 0x40, 0x0c, 0x00, 0x0b, 0x0b}, w.I32(0))
	j := hosted(t, runner, 300*time.Millisecond)
	// The first's verdict comes back to the test's own goroutine, which
	// alone may fail it.
	type judged struct {
		verdict Verdict
		err     error
	}
	first := make(chan judged, 1)
	go func() {
		verdict, err := j.weigh(context.Background(), []byte{0xff})
		first <- judged{verdict, err}
	}()
	time.Sleep(50 * time.Millisecond)
	leaving, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
	defer cancel()
	if _, err := j.weigh(leaving, []byte{0x00}); err == nil || !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("an asker that leaves before its turn: %v", err)
	}
	began := time.Now()
	if verdict := verdictOf(t, j, []byte{0x00}); verdict.Code != "" {
		t.Fatalf("the next in turn: %q", verdict.Code)
	}
	if waited := time.Since(began); waited < 100*time.Millisecond {
		t.Fatalf("the next judgement waited %v, not its turn", waited)
	}
	if first := <-first; first.err != nil || first.verdict.Code != Late {
		t.Fatalf("the first: %q %v", first.verdict.Code, first.err)
	}
}

// A runner that is not one of this host's version is refused as the
// server starts, never when a file comes.
func TestARunnerOfAnotherEngineIsRefused(t *testing.T) {
	for _, c := range []struct {
		what   string
		change func(*runnerCode)
		says   string
	}{
		{"another version", func(r *runnerCode) { r.engine = w.I32(Engine + 1) }, "asks for engine 2"},
		{"an import past the version", func(r *runnerCode) { r.imports = []string{"load", "clock"} }, "engine.clock"},
		{"an import of another module", func(r *runnerCode) { r.imports = []string{"env.load"} }, "env.load"},
	} {
		runner := keeping()
		c.change(&runner)
		_, err := host(context.Background(), runner.bytes())
		if err == nil || !strings.Contains(err.Error(), c.says) {
			t.Fatalf("%s: %v", c.what, err)
		}
	}
	// A runner missing what a judge calls.
	lacking := w.Module(w.Section(1, w.Vector([]byte{0x60, 0x00, 0x01, 0x7f})), w.Section(3, w.Vector([]byte{0x00})),
		w.Section(5, w.Vector([]byte{0x00, 0x01})), w.Section(7, w.Vector(w.Join(w.Name("engine"), []byte{0x00, 0x00}))),
		w.Section(10, w.Vector(w.Body(w.I32(Engine)))))
	if _, err := host(context.Background(), lacking); err == nil || !strings.Contains(err.Error(), "as a judge calls it") {
		t.Fatalf("a runner with no check: %v", err)
	}
}

// The pin is read as the arcade crate reads it: a runner whose bytes are
// not its name, or a manifest that names another engine, never starts.
func TestAPinThatIsNotWhatItNamesIsRefused(t *testing.T) {
	runner := keeping().bytes()
	sum := sha256.Sum256(runner)
	name := hex.EncodeToString(sum[:]) + ".wasm"
	for _, c := range []struct {
		what     string
		manifest string
		files    map[string][]byte
		says     string
	}{
		{"its runner", `{"engine":1,"runner":"` + name + `"}`, map[string][]byte{name: runner}, ""},
		{"a runner of other bytes", `{"engine":1,"runner":"` + name + `"}`, map[string][]byte{name: append(runner, 0)}, "not to its name"},
		{"a runner not there", `{"engine":1,"runner":"` + name + `"}`, nil, "the pinned runner"},
		{"a runner named otherwise", `{"engine":1,"runner":"runner.wasm"}`, map[string][]byte{"runner.wasm": runner}, "not by the hash"},
		{"another engine", `{"engine":2,"runner":"` + name + `"}`, map[string][]byte{name: runner}, "asks for engine 2"},
		{"no engine", `{"runner":"` + name + `"}`, map[string][]byte{name: runner}, "names no engine"},
	} {
		pin := fstest.MapFS{"emulator.json": {Data: []byte(c.manifest)}}
		for file, bytes := range c.files {
			pin[file] = &fstest.MapFile{Data: bytes}
		}
		j, err := pinned(context.Background(), pin)
		switch {
		case c.says == "" && err != nil:
			t.Fatalf("%s: %v", c.what, err)
		case c.says == "":
			_ = j.Close(context.Background())
		case err == nil || !strings.Contains(err.Error(), c.says):
			t.Fatalf("%s: %v", c.what, err)
		}
	}
}
