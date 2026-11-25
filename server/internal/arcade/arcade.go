// Package arcade judges a cartridge before the world keeps it, by the
// arcade's emulator as the engine pins it: its runner, `runner.wasm`, run
// here on wazero, judges a file by its own check, the one every host and the
// page judge by, so the server holds no rule of the contract and no copy of
// one. The server plays no game: the engine it gives the runner loads none
// ([engine]).
package arcade

import (
	"context"
	"crypto/sha256"
	"embed"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"log"
	"math"
	"regexp"
	"slices"
	"strings"
	"time"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
	"github.com/tetratelabs/wazero/sys"
)

// The pinned emulator's manifest and its runner, copied from
// arcade/emulator/ by `bun run module`, the runner under the name the pin
// gives it.
//
//go:embed emulator
var embedded embed.FS

// Kind is the kind of file the arcade judges, as the files package reads it
// off the bytes: a WebAssembly module, a cartridge.
const Kind = "wasm"

// Engine is the version of the imports of module `engine` this host gives,
// the version a runner asks for by its `engine` export and the pin's
// manifest names: another is refused as the server starts.
const Engine = 1

// engineImports are the names of module `engine` at version 1: what a
// host's engine gives the runner to load a game, call it, read its memory
// and its meter, and let it go.
var engineImports = []string{"load", "why_not", "call", "read", "meter", "set_meter", "drop"}

// The runner's exports a judge calls, each as it calls it: its version of
// the engine, room for a file, the check, and where the check's words are.
var judging = map[string][2][]api.ValueType{
	"engine": {nil, {api.ValueTypeI32}},
	"file":   {{api.ValueTypeI32}, {api.ValueTypeI32}},
	"check":  {nil, {api.ValueTypeI32}},
	"out":    {nil, {api.ValueTypeI32}},
}

const (
	// A judgement ends within this long, or the file is refused as Late.
	// The costliest cartridge to prepare, one of direct calls just under
	// the most bytes, is judged in about 60 ms on a laptop (its test logs
	// it), so a board many times slower keeps well within it, and a file
	// past it costs the server no more than this.
	deadline = 2 * time.Second
	// The most the runner's memory grows to while it judges, in pages of
	// 64 KiB: 64 MiB. That cartridge peaks at about 10 MiB (its test logs
	// it, and holds it under half of this); a runner that asks past this is
	// told no, traps, and the file is refused as Trapped.
	memoryPages = 1024
)

// The codes of a refusal of the judge's own, beside the runner's.
const (
	// Late: the runner's check ran past the deadline.
	Late = "late"
	// Trapped: the runner trapped judging the file, its memory at the
	// ceiling among the causes, or broke the calls a judge makes.
	Trapped = "trapped"
)

// noGame is what the engine of a judge says when the runner asks it for
// anything: it loads no game.
var noGame = errors.New("a judge loads no game")

// Judge is the pinned runner, compiled, judging one file at a time, each on
// an instance of its own let go after: what judging one file grew or broke
// is never the next one's.
type Judge struct {
	runtime wazero.Runtime
	runner  wazero.CompiledModule
	// A judgement ends within this long: [deadline], shorter in a test.
	deadline time.Duration
	// The turn to judge, held while a judgement runs.
	turn chan struct{}
	// How many judgements there have been, which names each instance.
	judged int
}

// Verdict is a judgement and what it cost: the code of why not, "" when
// the file keeps the contract, the time it took and the bytes the runner's
// memory grew to, its peak, since a memory never shrinks.
type Verdict struct {
	Code   string
	Took   time.Duration
	Memory uint32
}

// manifest is what a judge reads of the pinned emulator's `emulator.json`.
type manifest struct {
	Engine *uint32 `json:"engine"`
	Runner string  `json:"runner"`
}

// builtName is a built file's name: the sha256 of its bytes and an extension.
var builtName = regexp.MustCompile(`^([0-9a-f]{64})\.wasm$`)

// Load hosts the runner this binary carries: its bytes checked against the
// name the pin gives them, and the version of the engine it asks for
// against this host's.
func Load(ctx context.Context) (*Judge, error) {
	pin, err := fs.Sub(embedded, "emulator")
	if err != nil {
		return nil, fmt.Errorf("arcade: %w", err)
	}
	return pinned(ctx, pin)
}

// pinned hosts the runner the emulator in `pin` names, as [Load] does.
func pinned(ctx context.Context, pin fs.FS) (*Judge, error) {
	read, err := fs.ReadFile(pin, "emulator.json")
	if err != nil {
		return nil, fmt.Errorf("arcade: the pinned emulator's manifest: %w", err)
	}
	var said manifest
	if err := json.Unmarshal(read, &said); err != nil {
		return nil, fmt.Errorf("arcade: the pinned emulator's manifest: %w", err)
	}
	if said.Engine == nil {
		return nil, errors.New("arcade: the pinned emulator's manifest names no engine")
	}
	if *said.Engine != Engine {
		return nil, fmt.Errorf("arcade: the pinned emulator's runner asks for engine %d, and this host gives %d", *said.Engine, Engine)
	}
	named := builtName.FindStringSubmatch(said.Runner)
	if named == nil {
		return nil, fmt.Errorf("arcade: the pinned emulator names its runner %q, not by the hash of a .wasm", said.Runner)
	}
	runner, err := fs.ReadFile(pin, said.Runner)
	if err != nil {
		return nil, fmt.Errorf("arcade: the pinned runner: %w", err)
	}
	if sum := sha256.Sum256(runner); hex.EncodeToString(sum[:]) != named[1] {
		return nil, fmt.Errorf("arcade: the pinned runner %s hashes to %x, not to its name", said.Runner, sum)
	}
	return host(ctx, runner)
}

// host compiles `runner`, gives it the engine of a judge, and asks the
// version of the engine it asks for.
func host(ctx context.Context, runner []byte) (*Judge, error) {
	// A judgement ends when its context does, and the runner's memory has
	// a ceiling.
	runtime := wazero.NewRuntimeWithConfig(ctx, wazero.NewRuntimeConfig().
		WithCloseOnContextDone(true).
		WithMemoryLimitPages(memoryPages))
	j := &Judge{runtime: runtime, deadline: deadline, turn: make(chan struct{}, 1)}
	if err := j.compile(ctx, runner); err != nil {
		_ = runtime.Close(ctx)
		return nil, fmt.Errorf("arcade: %w", err)
	}
	return j, nil
}

func (j *Judge) compile(ctx context.Context, runner []byte) error {
	compiled, err := j.runtime.CompileModule(ctx, runner)
	if err != nil {
		return err
	}
	exports := compiled.ExportedFunctions()
	for name, want := range judging {
		got, there := exports[name]
		if !there || !slices.Equal(got.ParamTypes(), want[0]) || !slices.Equal(got.ResultTypes(), want[1]) {
			return fmt.Errorf("the runner does not export `%s` as a judge calls it", name)
		}
	}
	if len(compiled.ExportedMemories()) == 0 {
		return errors.New("the runner exports no memory")
	}
	if len(compiled.ImportedMemories()) != 0 {
		return errors.New("the runner imports a memory, which no engine gives")
	}
	if err := engine(ctx, j.runtime, compiled); err != nil {
		return err
	}
	j.runner = compiled
	instance, err := j.runtime.InstantiateModule(ctx, compiled, wazero.NewModuleConfig().WithName("asked"))
	if err != nil {
		return err
	}
	defer instance.Close(ctx)
	asks, err := instance.ExportedFunction("engine").Call(ctx)
	if err != nil {
		return err
	}
	if uint32(asks[0]) != Engine {
		return fmt.Errorf("the runner asks for engine %d, and this host gives %d", uint32(asks[0]), Engine)
	}
	return nil
}

// engine gives the runner the imports of module `engine` it asks for, each
// of the type it asks. A judge loads no game, and the runner's check calls
// none of them, so each traps, naming itself: a runner whose check began to
// ask for one would refuse every file, loudly, rather than pass one. An
// import of another module, or past the version's names, is no runner of
// this version.
func engine(ctx context.Context, runtime wazero.Runtime, runner wazero.CompiledModule) error {
	builder := runtime.NewHostModuleBuilder("engine")
	for _, asked := range runner.ImportedFunctions() {
		module, name, _ := asked.Import()
		if module != "engine" || !slices.Contains(engineImports, name) {
			return fmt.Errorf("the runner imports %s.%s, which engine %d does not give", module, name, Engine)
		}
		trap := api.GoModuleFunc(func(context.Context, api.Module, []uint64) {
			panic(fmt.Errorf("%w, and the runner asked engine.%s", noGame, name))
		})
		builder.NewFunctionBuilder().WithGoModuleFunction(trap, asked.ParamTypes(), asked.ResultTypes()).Export(name)
	}
	_, err := builder.Instantiate(ctx)
	return err
}

// Close lets the runtime go.
func (j *Judge) Close(ctx context.Context) error {
	return j.runtime.Close(ctx)
}

// Judge says why `file`, of `kind`, may not be kept: the code of the
// runner's refusal, or [Late] or [Trapped]; "" when it keeps the contract,
// and for a kind the arcade does not judge. It waits its turn as long as
// `ctx` lives. An error is no verdict: the judgement was called off.
func (j *Judge) Judge(ctx context.Context, kind string, file []byte) (string, error) {
	if kind != Kind {
		return "", nil
	}
	verdict, err := j.weigh(ctx, file)
	return verdict.Code, err
}

// weigh judges `file` in its turn, and says what it cost.
func (j *Judge) weigh(ctx context.Context, file []byte) (Verdict, error) {
	select {
	case j.turn <- struct{}{}:
	case <-ctx.Done():
		return Verdict{}, fmt.Errorf("arcade: no turn to judge: %w", ctx.Err())
	}
	defer func() { <-j.turn }()
	j.judged++
	ctx, cancel := context.WithTimeout(ctx, j.deadline)
	defer cancel()
	began := time.Now()
	instance, err := j.runtime.InstantiateModule(ctx, j.runner,
		wazero.NewModuleConfig().WithName(fmt.Sprintf("judge-%d", j.judged)))
	if err != nil {
		return Verdict{}, fmt.Errorf("arcade: %w", err)
	}
	defer instance.Close(context.Background())
	code, err := check(ctx, instance, file)
	verdict := Verdict{Code: code, Took: time.Since(began), Memory: instance.Memory().Size()}
	if err == nil {
		return verdict, nil
	}
	var exit *sys.ExitError
	switch {
	case errors.As(err, &exit) && exit.ExitCode() == sys.ExitCodeDeadlineExceeded:
		log.Printf("arcade: a file of %d bytes is not judged within %v", len(file), j.deadline)
		verdict.Code = Late
	case errors.As(err, &exit) && exit.ExitCode() == sys.ExitCodeContextCanceled:
		return Verdict{}, fmt.Errorf("arcade: the judgement was called off: %w", ctx.Err())
	default:
		// The trap's words, without the stack wazero writes after them.
		words, _, _ := strings.Cut(err.Error(), "\n")
		log.Printf("arcade: the runner trapped judging a file of %d bytes, its memory %d KiB of %d KiB: %s",
			len(file), verdict.Memory>>10, memoryPages<<6, words)
		verdict.Code = Trapped
	}
	return verdict, nil
}

// errOutside is a runner that pointed outside its own memory.
var errOutside = errors.New("the runner pointed outside its memory")

// check writes `file` where the runner's next call reads it and asks the
// runner's check: the code of why not, "" when it keeps the contract.
func check(ctx context.Context, instance api.Module, file []byte) (string, error) {
	// Past 4 GiB a length is any past the most, and refused the same.
	length := min(uint64(len(file)), math.MaxUint32)
	at, err := instance.ExportedFunction("file").Call(ctx, length)
	if err != nil {
		return "", err
	}
	// A file past the most gets no room, a null pointer, and is never
	// copied: the runner refuses it by its size.
	if at[0] != 0 && !instance.Memory().Write(uint32(at[0]), file) {
		return "", errOutside
	}
	said, err := instance.ExportedFunction("check").Call(ctx)
	if err != nil {
		return "", err
	}
	out, err := instance.ExportedFunction("out").Call(ctx)
	if err != nil {
		return "", err
	}
	code, ok := instance.Memory().Read(uint32(out[0]), uint32(said[0]))
	if !ok {
		return "", errOutside
	}
	return string(code), nil
}
