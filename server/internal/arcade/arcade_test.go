package arcade

import (
	"bytes"
	"context"
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	w "github.com/tekne-studio/planet/server/internal/wasmtest"
)

// The engine's pinned emulator and the ROMs it ships, from this package.
const (
	pinFolder = "../../../arcade/emulator"
	romFolder = "../../../arcade/roms"
)

// loaded is the judge of the pin this binary carries.
func loaded(t *testing.T) *Judge {
	t.Helper()
	j, err := Load(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = j.Close(context.Background()) })
	return j
}

// pin is the engine's pinned manifest, read as the test needs it.
type pin struct {
	Runner string `json:"runner"`
	Most   int    `json:"most"`
	Cases  string `json:"cases"`
}

func readPin(t *testing.T) pin {
	t.Helper()
	read, err := os.ReadFile(filepath.Join(pinFolder, "emulator.json"))
	if err != nil {
		t.Fatal(err)
	}
	var said pin
	if err := json.Unmarshal(read, &said); err != nil {
		t.Fatal(err)
	}
	return said
}

// What the binary embeds is the engine's pin, byte for byte: `bun run
// module` copies it, and a copy older than the pin is caught here.
func TestTheEmbeddedEmulatorIsThePin(t *testing.T) {
	for _, name := range []string{"emulator.json", readPin(t).Runner} {
		embeddedBytes, err := embedded.ReadFile("emulator/" + name)
		if err != nil {
			t.Fatalf("%s is not embedded: %v; `bun run module` copies it", name, err)
		}
		pinned, err := os.ReadFile(filepath.Join(pinFolder, name))
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(embeddedBytes, pinned) {
			t.Fatalf("the embedded %s is not the pin's: `bun run module` copies it again", name)
		}
	}
	loaded(t)
}

// verdictOf judges `file` and fails the test on an error.
func verdictOf(t *testing.T, j *Judge, file []byte) Verdict {
	t.Helper()
	verdict, err := j.weigh(context.Background(), file)
	if err != nil {
		t.Fatal(err)
	}
	return verdict
}

// Every ROM this version ships keeps the contract.
func TestEveryROMIsKept(t *testing.T) {
	j := loaded(t)
	roms, err := filepath.Glob(filepath.Join(romFolder, "*.wasm"))
	if err != nil || len(roms) == 0 {
		t.Fatalf("no ROM in %s: %v", romFolder, err)
	}
	for _, rom := range roms {
		file, err := os.ReadFile(rom)
		if err != nil {
			t.Fatal(err)
		}
		verdict := verdictOf(t, j, file)
		if verdict.Code != "" {
			t.Fatalf("%s is refused: %q", filepath.Base(rom), verdict.Code)
		}
		t.Logf("%s, %d bytes: kept in %v, the runner's memory %d KiB", filepath.Base(rom), len(file), verdict.Took, verdict.Memory>>10)
	}
}

// The largest cartridges, written to cost most, are judged well within
// the deadline and the memory's ceiling: what the world asks before it
// keeps one never refuses one that keeps the contract. The best of three
// is what one costs, the others what the machine was doing besides.
func TestTheCostliestCartridgesAreJudgedWellWithinTheLimits(t *testing.T) {
	j := loaded(t)
	most := readPin(t).Most
	for _, c := range []struct {
		what string
		file []byte
	}{
		{"of loops", w.Cartridge(1000, w.Loops(35))},
		{"of direct calls", w.Cartridge(1000, w.Calls(120))},
	} {
		if len(c.file) > most || len(c.file) < most*9/10 {
			t.Fatalf("the cartridge %s is %d bytes, not just under %d", c.what, len(c.file), most)
		}
		best := Verdict{Took: deadline}
		for range 3 {
			verdict := verdictOf(t, j, c.file)
			if verdict.Code != "" {
				t.Fatalf("a cartridge %s of %d bytes: %q", c.what, len(c.file), verdict.Code)
			}
			if verdict.Took < best.Took {
				best = verdict
			}
		}
		t.Logf("a cartridge %s of %d bytes is judged in %v, the runner's memory at %d KiB", c.what, len(c.file), best.Took, best.Memory>>10)
		if best.Took > deadline/2 {
			t.Fatalf("judging one %s took %v, over half the deadline %v", c.what, best.Took, deadline)
		}
		if best.Memory > memoryPages<<16/2 {
			t.Fatalf("judging one %s grew the runner to %d KiB, over half the ceiling", c.what, best.Memory>>10)
		}
	}
}

// A file past the most a cartridge may have is refused by its size, and
// never enters the runner: its memory stays smaller than the file.
func TestAFilePastTheMostIsRefusedByItsSizeAndNeverCopied(t *testing.T) {
	j := loaded(t)
	most := readPin(t).Most
	for _, size := range []int{most + 1, 16 << 20} {
		file := append([]byte(w.Header), make([]byte, size-len(w.Header))...)
		verdict := verdictOf(t, j, file)
		if verdict.Code != "size" {
			t.Fatalf("a file of %d bytes: %q, not size", size, verdict.Code)
		}
		if int(verdict.Memory) >= 16<<20 {
			t.Fatalf("a file of %d bytes entered the runner, its memory %d bytes", size, verdict.Memory)
		}
	}
}

// A kind the arcade does not judge is kept without asking the runner.
func TestAnotherKindIsNotJudged(t *testing.T) {
	j := loaded(t)
	code, err := j.Judge(context.Background(), "png", []byte("\x89PNG\r\n\x1a\n"))
	if err != nil || code != "" || j.judged != 0 {
		t.Fatalf("a picture: %q %v, judged %d", code, err, j.judged)
	}
}

// A case of the emulator's cases, as a judge reads it: the test it came
// from, its file, and the verdict every host's check gave it.
type judged struct {
	Test  string  `json:"test"`
	File  string  `json:"file"`
	Check *string `json:"check"`
	bytes []byte
}

// readCases is the pinned emulator's cases.
func readCases(t *testing.T) []judged {
	t.Helper()
	read, err := os.ReadFile(filepath.Join(pinFolder, readPin(t).Cases))
	if err != nil {
		t.Fatal(err)
	}
	var cases []judged
	if err := json.Unmarshal(read, &cases); err != nil {
		t.Fatal(err)
	}
	if len(cases) == 0 {
		t.Fatal("the pinned emulator holds no case")
	}
	for at := range cases {
		c := &cases[at]
		if c.bytes, err = hex.DecodeString(c.File); err != nil {
			t.Fatalf("case %d, %s: its file is no hex: %v", at, c.Test, err)
		}
		if c.Check == nil {
			t.Fatalf("case %d, %s, says no verdict: a pin older than its judge", at, c.Test)
		}
	}
	return cases
}

// The emulator's cases are the conformance suite of every host. A judge
// plays no game, so of each case only the verdict applies, what `check`
// says of its file: the frames, the tones, the stop, the updates and the
// last screen are a player's, and every host that plays is held to them.
// Every case applies, and gives the verdict every host's check gave.
func TestEveryCaseIsJudgedAsEveryHostJudgedIt(t *testing.T) {
	j := loaded(t)
	cases := readCases(t)
	var worst struct {
		took, memory Verdict
		slow, heavy  string
	}
	refused := map[string]int{}
	var differ []string
	for _, c := range cases {
		verdict := verdictOf(t, j, c.bytes)
		if verdict.Code != *c.Check {
			differ = append(differ, c.Test+": judged "+quote(verdict.Code)+", every host "+quote(*c.Check))
		}
		if verdict.Code != "" {
			refused[verdict.Code]++
		}
		if verdict.Took > worst.took.Took {
			worst.took, worst.slow = verdict, c.Test
		}
		if verdict.Memory > worst.memory.Memory {
			worst.memory, worst.heavy = verdict, c.Test
		}
	}
	if len(differ) != 0 {
		t.Fatalf("%d cases differ:\n%s", len(differ), strings.Join(differ, "\n"))
	}
	t.Logf("%d cases judged as every host judged them, %d refused by code: %v", len(cases), sumOf(refused), refused)
	t.Logf("the slowest, %s: %v; the heaviest, %s: the runner's memory at %d KiB",
		worst.slow, worst.took.Took.Round(time.Microsecond), worst.heavy, worst.memory.Memory>>10)
}

func quote(code string) string { return `"` + code + `"` }

func sumOf(counts map[string]int) int {
	sum := 0
	for _, n := range counts {
		sum += n
	}
	return sum
}
