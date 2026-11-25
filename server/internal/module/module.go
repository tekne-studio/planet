// Package module runs the server's module: every world half of a version,
// written in Rust, built as one WASM file and run here through wazero. It is
// the one package that knows the module is WASM.
// The core meets each plugin it carries as a world.Plugin, and the bridge
// between the two is the schema of proto/planet/module/v1.
package module

import (
	"bytes"
	"context"
	_ "embed"
	"errors"
	"fmt"
	"log"
	"slices"
	"sync"
	"time"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
	"google.golang.org/protobuf/proto"

	pb "github.com/tekne-studio/planet/server/internal/protocol"
	bridge "github.com/tekne-studio/planet/server/internal/protocol/module"
	"github.com/tekne-studio/planet/server/internal/store"
	"github.com/tekne-studio/planet/server/internal/world"
)

// The module, built from crates/module by `bun run module`.
//
//go:embed world.wasm
var built []byte

const (
	// A call ends within this long, or the instance is replaced: a world
	// half that never returns stops no world.
	deadline = 250 * time.Millisecond
	// A start runs once an instance and makes the ground of a world shaped by
	// a field from its file, many times an op's work: the Earth's takes 43 to
	// 48 ms on a laptop and nears `deadline` on a box five times slower, so
	// it has a deadline of its own.
	startDeadline = time.Second
	// The most a module holds, in pages of 64 KiB: 64 MB. A world on the
	// Earth holds 17.1 MB of it, one with no field 1.2.
	memoryPages = 1024
	// The most a call is answered with: this many replies, each this long.
	// The ceiling of memory is the module's; these are the server's, for what
	// it keeps of what the module says.
	maxReplies   = 1024
	maxReplySize = 1 << 20
	// And a limit on what a call asks of its owner's store: how many
	// questions, how long each is, and how much all the answers weigh.
	maxAsks     = 4096
	maxAskSize  = 1 << 16
	maxRowsSize = 16 << 20
)

// Keeper is where owners keep what they keep: the store of the world folder.
type Keeper interface {
	Get(owner string, key []byte) ([]byte, bool, error)
	Scan(owner string, prefix []byte) ([]store.Row, error)
	Apply(owner string, writes []store.Write) error
}

// Fields is where the field a recipe names is found: the file, checked
// against its id, or nil for a recipe that names none. `fields.Open("")`
// carries none.
type Fields interface {
	For(recipe world.Recipe) ([]byte, error)
}

// lane is an instance of the module and the call it is serving: the
// world's, which serves every op and keeps what world halves hold in
// memory.
type lane struct {
	instance api.Module
	// The owner the running call belongs to, and what it has asked of its
	// store so far.
	owner   string
	asks    int
	weighed int
	// The call in flight: what the module said so far, and what went wrong
	// hearing it.
	replies []*bridge.Reply
	fault   error
}

// laneKey carries the lane a call runs on to the host functions it calls.
type laneKey struct{}

// laneOf is the lane of the call a host function serves.
func laneOf(ctx context.Context) *lane {
	return ctx.Value(laneKey{}).(*lane)
}

// Module is the owners of a version, running: the systems of the core that
// run in the world, and the world halves. One call at a time on the
// world's lane.
type Module struct {
	// Where owners keep what they keep. Nil in a test that keeps nothing.
	keeper Keeper
	// Where a world's field is found.
	fields   Fields
	runtime  wazero.Runtime
	compiled wazero.CompiledModule
	// The world's lane. Its instance is replaced after any fault.
	mu sync.Mutex
	lane
	// How many instances of the world's lane there have been.
	born int
	// The recipe the instance was started with. Nil until a world starts,
	// and again once the instance is replaced.
	started *world.Recipe
	// Why the world did not start, once a Start failed. What failed it, its
	// field, its memory or its deadline, fails it again, so it is not tried
	// twice: every op is refused until the server restarts, and there the
	// Start runs before the world serves and stops it, saying why.
	unstarted error
	// What the module carries, as it said at Load.
	carried []*bridge.Carried
}

// Load compiles the module built into this binary and asks what it carries.
func Load(ctx context.Context, keeper Keeper, fields Fields) (*Module, error) {
	m, err := open(ctx, built)
	if err != nil {
		return nil, err
	}
	m.keeper, m.fields = keeper, fields
	replies, err := m.call(ctx, &bridge.Call{Call: &bridge.Call_Describe{Describe: &bridge.Describe{}}})
	if err != nil {
		return nil, fmt.Errorf("module: asked what it carries: %w", err)
	}
	for _, reply := range replies {
		if statement := reply.GetStatement(); statement != nil {
			m.carried = statement.Plugins
			return m, nil
		}
	}
	return nil, errors.New("module: it said nothing of what it carries")
}

// open compiles a module and starts its first instance.
func open(ctx context.Context, wasm []byte) (*Module, error) {
	m := &Module{}
	// A call ends when its context does, and a module's memory has a ceiling.
	m.runtime = wazero.NewRuntimeWithConfig(ctx, wazero.NewRuntimeConfig().
		WithCloseOnContextDone(true).
		WithMemoryLimitPages(memoryPages))
	if _, err := m.runtime.NewHostModuleBuilder("host").
		NewFunctionBuilder().WithFunc(m.reply).Export("reply").
		NewFunctionBuilder().WithFunc(m.asked).Export("ask").
		Instantiate(ctx); err != nil {
		return nil, fmt.Errorf("module: %w", err)
	}
	compiled, err := m.runtime.CompileModule(ctx, wasm)
	if err != nil {
		return nil, fmt.Errorf("module: %w", err)
	}
	// The two names the server calls, as it calls them, and a memory to
	// write a call in: a file without them is no module.
	exports := compiled.ExportedFunctions()
	i32 := []api.ValueType{api.ValueTypeI32}
	for name, want := range map[string][2][]api.ValueType{"reserve": {i32, i32}, "call": {nil, nil}} {
		got, there := exports[name]
		if !there || !slices.Equal(got.ParamTypes(), want[0]) || !slices.Equal(got.ResultTypes(), want[1]) {
			return nil, fmt.Errorf("module: it does not export `%s` as the bridge calls it", name)
		}
	}
	if len(compiled.ExportedMemories()) == 0 {
		return nil, errors.New("module: it exports no memory")
	}
	m.compiled = compiled
	if err := m.replace(ctx); err != nil {
		return nil, err
	}
	return m, nil
}

// Close lets the runtime go.
func (m *Module) Close(ctx context.Context) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.runtime.Close(ctx)
}

// replace starts a new instance in place of the one there is. What a world
// half held in memory goes with the old one, and the world starts again in
// the new one before its next op.
func (m *Module) replace(ctx context.Context) error {
	if m.instance != nil {
		_ = m.instance.Close(ctx)
	}
	m.instance, m.started = nil, nil
	m.born++
	instance, err := m.runtime.InstantiateModule(ctx, m.compiled,
		wazero.NewModuleConfig().WithName(fmt.Sprintf("world-%d", m.born)))
	if err != nil {
		return fmt.Errorf("module: %w", err)
	}
	m.instance = instance
	return nil
}

// reply is what the module calls to hand over one reply, whole. The bytes
// are read within the module's memory or refused, and copied before the
// module runs again. The first reply that cannot be taken ends the call: the
// instance is closed where it stands, so a module that goes on saying is not
// heard until its deadline.
func (m *Module) reply(ctx context.Context, instance api.Module, at, size uint32) {
	l := laneOf(ctx)
	if l.fault != nil {
		return
	}
	if l.fault = l.take(instance, at, size); l.fault != nil {
		_ = instance.CloseWithExitCode(ctx, 1)
	}
}

func (l *lane) take(instance api.Module, at, size uint32) error {
	if len(l.replies) == maxReplies {
		return fmt.Errorf("more than %d replies to one call", maxReplies)
	}
	if size > maxReplySize {
		return fmt.Errorf("a reply of %d bytes", size)
	}
	view, ok := instance.Memory().Read(at, size)
	if !ok {
		return errors.New("a reply outside the module's memory")
	}
	reply := &bridge.Reply{}
	if err := proto.Unmarshal(bytes.Clone(view), reply); err != nil {
		return fmt.Errorf("a reply that is no message: %w", err)
	}
	l.replies = append(l.replies, reply)
	return nil
}

// asked answers what an owner asks of its store while a call runs: the rows
// are written in the module's inbox, which the module sizes itself, and
// their length is handed back. A question that cannot be taken or answered
// ends the instance, as a reply that cannot be taken does.
func (m *Module) asked(ctx context.Context, instance api.Module, at, size uint32) uint32 {
	l := laneOf(ctx)
	if l.fault != nil {
		return 0
	}
	rows, err := m.answer(l, instance, at, size)
	if err == nil {
		var where []uint64
		where, err = instance.ExportedFunction("reserve").Call(ctx, uint64(len(rows)))
		if err == nil && !instance.Memory().Write(uint32(where[0]), rows) {
			err = errors.New("an inbox outside the module's memory")
		}
	}
	if err != nil {
		l.fault = err
		_ = instance.CloseWithExitCode(ctx, 1)
		return 0
	}
	return uint32(len(rows))
}

func (m *Module) answer(l *lane, instance api.Module, at, size uint32) ([]byte, error) {
	if l.asks++; l.asks > maxAsks {
		return nil, fmt.Errorf("more than %d questions in one call", maxAsks)
	}
	if size > maxAskSize {
		return nil, fmt.Errorf("a question of %d bytes", size)
	}
	view, ok := instance.Memory().Read(at, size)
	if !ok {
		return nil, errors.New("a question outside the module's memory")
	}
	ask := &bridge.Ask{}
	if err := proto.Unmarshal(bytes.Clone(view), ask); err != nil {
		return nil, fmt.Errorf("a question that is no message: %w", err)
	}
	// An owner reads its own store and no other's.
	if ask.Owner != l.owner {
		return nil, fmt.Errorf("%q asks for the store of %q", l.owner, ask.Owner)
	}
	found := &bridge.Rows{}
	if m.keeper != nil {
		switch asked := ask.Ask.(type) {
		case *bridge.Ask_Get:
			value, there, err := m.keeper.Get(ask.Owner, asked.Get)
			if err != nil {
				return nil, err
			}
			if there {
				found.Rows = append(found.Rows, &bridge.Row{Key: asked.Get, Value: value})
			}
		case *bridge.Ask_Scan:
			rows, err := m.keeper.Scan(ask.Owner, asked.Scan)
			if err != nil {
				return nil, err
			}
			for _, row := range rows {
				found.Rows = append(found.Rows, &bridge.Row{Key: row.Key, Value: row.Value})
			}
		}
	}
	rows, err := proto.Marshal(found)
	if err != nil {
		return nil, err
	}
	if l.weighed += len(rows); l.weighed > maxRowsSize {
		return nil, fmt.Errorf("more than %d bytes read in one call", maxRowsSize)
	}
	return rows, nil
}

// call asks the world's lane one thing and gathers what it says back.
// After any fault, a trap, a deadline or a reply that cannot be read, the
// instance is replaced and the call is lost.
func (m *Module) call(ctx context.Context, call *bridge.Call) ([]*bridge.Reply, error) {
	if m.instance == nil {
		if err := m.replace(ctx); err != nil {
			return nil, err
		}
	}
	replies, err := m.lane.run(ctx, call)
	if err != nil {
		if again := m.replace(ctx); again != nil {
			log.Printf("module: no new instance: %v", again)
		}
		return nil, err
	}
	return replies, nil
}

// run asks the lane's instance one thing, within the deadline of its kind,
// and gathers what it says back.
func (l *lane) run(ctx context.Context, call *bridge.Call) ([]*bridge.Reply, error) {
	frame, err := proto.Marshal(call)
	if err != nil {
		return nil, err
	}
	l.replies, l.fault = nil, nil
	l.asks, l.weighed = 0, 0
	within := deadline
	if call.GetStart() != nil {
		within = startDeadline
	}
	ctx, cancel := context.WithTimeout(ctx, within)
	defer cancel()
	if err := l.ask(ctx, frame); err != nil {
		return nil, err
	}
	return l.replies, nil
}

// ask writes a call into the inbox the module sized for it, and runs it
// until the context is done.
func (l *lane) ask(ctx context.Context, frame []byte) error {
	ctx = context.WithValue(ctx, laneKey{}, l)
	at, err := l.instance.ExportedFunction("reserve").Call(ctx, uint64(len(frame)))
	if err != nil {
		return err
	}
	if !l.instance.Memory().Write(uint32(at[0]), frame) {
		return errors.New("an inbox outside the module's memory")
	}
	_, err = l.instance.ExportedFunction("call").Call(ctx)
	// What went wrong hearing a reply says more than the exit it caused.
	if l.fault != nil {
		return l.fault
	}
	return err
}

// Start starts a founded world before anyone is in it, so a world whose
// ground cannot be made says why before it serves.
func (m *Module) Start(ctx context.Context, recipe world.Recipe) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	return m.start(ctx, recipe)
}

// start tells the instance which world this is, once for each instance and
// each recipe, with the file of the field it names: the module makes the
// ground of it.
func (m *Module) start(ctx context.Context, recipe world.Recipe) error {
	if m.unstarted != nil {
		return m.unstarted
	}
	if s := m.started; s != nil && s.Seed == recipe.Seed && s.GeneratorVersion == recipe.GeneratorVersion && bytes.Equal(s.Params, recipe.Params) {
		return nil
	}
	field, err := m.fields.For(recipe)
	if err == nil {
		start := &bridge.Start{Recipe: recipe.Wire(), Field: field}
		_, err = m.call(ctx, &bridge.Call{Call: &bridge.Call_Start{Start: start}})
	}
	if err != nil {
		m.unstarted = fmt.Errorf("module: the world did not start: %w", err)
		return m.unstarted
	}
	m.started = &recipe
	return nil
}

// Plugins is the plugins the module carries, each as the core hosts it, in
// the order the config lists them.
func (m *Module) Plugins() []world.Installed {
	installed := make([]world.Installed, 0, len(m.carried))
	for _, carried := range m.carried {
		ops := make([]world.Op, 0, len(carried.Ops))
		for _, op := range carried.Ops {
			ops = append(ops, world.Op{Kind: op.Kind, Level: world.Level(op.Level)})
		}
		installed = append(installed, world.Installed{
			Plugin: &hosted{module: m, name: carried.Name, version: carried.Version, ops: ops},
			On:     carried.On,
			Fixed:  carried.Fixed,
		})
	}
	return installed
}

// hosted is one plugin's world half, as the core's host of plugins sees it.
type hosted struct {
	module  *Module
	name    string
	version uint32
	ops     []world.Op
}

func (h *hosted) Name() string    { return h.name }
func (h *hosted) Version() uint32 { return h.version }
func (h *hosted) Ops() []world.Op { return h.ops }

// Do hands the op to the world half with the room it is asked in, and tells
// the room what the half said. An op the module faults on, or asked in a
// world that did not start, is refused `fault`: the asker takes it back.
func (h *hosted) Do(room world.Room, who world.Who, kind string, payload []byte) {
	m := h.module
	m.mu.Lock()
	defer m.mu.Unlock()
	ctx := context.Background()
	said := m.unstarted != nil
	if err := m.start(ctx, room.Recipe()); err != nil {
		if !said {
			log.Print(err)
		}
		room.Refuse("fault")
		return
	}
	here := room.Here()
	op := &bridge.Op{
		Plugin:  h.name,
		Kind:    kind,
		Payload: payload,
		Session: who.Session,
		NowMs:   uint64(room.Now().UnixMilli()),
		Here:    make([]*bridge.Who, 0, len(here)),
	}
	for _, other := range here {
		op.Here = append(op.Here, &bridge.Who{
			Session: other.Session,
			User:    other.Identity.UserID,
			Name:    other.Name,
			Level:   pb.Level(other.Identity.Level),
			Stance:  other.Stance,
		})
	}
	m.owner = h.name
	replies, err := m.call(ctx, &bridge.Call{Call: &bridge.Call_Op{Op: op}})
	m.owner = ""
	if err != nil {
		log.Printf("module: %s.%s is dropped: %v", h.name, kind, err)
		room.Refuse("fault")
		return
	}
	// What the op kept is written first, all of it or none: an event tells
	// of nothing the world did not keep. An owner writes its own store.
	var writes []store.Write
	for _, reply := range replies {
		if keep := reply.GetKeep(); keep != nil && keep.Owner == h.name {
			writes = append(writes, store.Write{Key: keep.Key, Value: keep.Value, Forget: keep.Forget})
		}
	}
	if len(writes) != 0 {
		if m.keeper == nil {
			err = errors.New("this world has no store")
		} else {
			err = m.keeper.Apply(h.name, writes)
		}
		if err != nil {
			log.Printf("module: %s.%s is not kept: %v", h.name, kind, err)
			room.Refuse("fault")
			return
		}
	}
	for _, reply := range replies {
		if answered := reply.GetAnswered(); answered != nil {
			room.Refuse(answered.Code)
		}
		// An owner links its own things alone, in the order it said them
		// among its events.
		if link := reply.GetLink(); link != nil && link.Owner == h.name {
			room.Link(link.Thing, link.Sessions)
		}
		tell := reply.GetTell()
		// An owner speaks under its own name alone.
		if tell == nil || tell.Plugin != h.name {
			continue
		}
		to := make(map[uint32]bool, len(tell.To))
		for _, session := range tell.To {
			to[session] = true
		}
		room.Tell(tell.Kind, tell.Payload, func(other world.Who) bool { return to[other.Session] })
	}
}

// Gone tells the world half a session left.
func (h *hosted) Gone(session uint32) {
	m := h.module
	m.mu.Lock()
	defer m.mu.Unlock()
	gone := &bridge.Gone{Plugin: h.name, Session: session}
	if _, err := m.call(context.Background(), &bridge.Call{Call: &bridge.Call_Gone{Gone: gone}}); err != nil {
		log.Printf("module: %s is not told who left: %v", h.name, err)
	}
}
