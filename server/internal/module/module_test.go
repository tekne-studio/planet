package module

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"slices"
	"strings"
	"testing"
	"time"

	"google.golang.org/protobuf/encoding/protowire"
	"google.golang.org/protobuf/proto"

	"github.com/tekne-studio/planet/server/internal/fields"
	pb "github.com/tekne-studio/planet/server/internal/protocol"
	"github.com/tekne-studio/planet/server/internal/protocol/cells"
	bridge "github.com/tekne-studio/planet/server/internal/protocol/module"
	"github.com/tekne-studio/planet/server/internal/world"
)

func loaded(t *testing.T) *Module {
	t.Helper()
	m, err := Load(context.Background(), nil, fields.Open(""))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { m.Close(context.Background()) })
	return m
}

// chat is the one plugin this version carries, as the core hosts it.
func chat(t *testing.T, m *Module) world.Plugin {
	t.Helper()
	for _, in := range m.Plugins() {
		if in.Plugin.Name() == "chat" {
			return in.Plugin
		}
	}
	t.Fatal("the module carries no chat")
	return nil
}

func TestTheModuleSaysWhatItCarries(t *testing.T) {
	carried := loaded(t).Plugins()
	// The cells come first: a system of the core, on in every world, whose
	// ops a builder asks and whose look anyone does.
	cells := carried[0]
	if cells.Plugin.Name() != "cells" || !cells.Fixed || !cells.On || len(cells.Plugin.Ops()) != 6 {
		t.Fatalf("the core's cells are carried as an owner: %v %v", cells, cells.Plugin.Ops())
	}
	// Then the parts, the second system of the core: placed things, and
	// the chart's home and favourites, an account's.
	parts := carried[1]
	ops := parts.Plugin.Ops()
	if parts.Plugin.Name() != "parts" || !parts.Fixed || !parts.On || len(ops) != 9 {
		t.Fatalf("the core's parts are carried as an owner: %v %v", parts, ops)
	}
	if mine := ops[7:]; mine[0] != (world.Op{Kind: "favour", Level: world.LevelSignedIn}) || mine[1] != (world.Op{Kind: "home", Level: world.LevelSignedIn}) {
		t.Fatalf("a home and a favourite are an account's: %v", mine)
	}
	// Then the life of a world: a system of the core a world's admin
	// switches, on until turned off, whose pets anyone keeps.
	life := carried[2]
	if life.Plugin.Name() != "gaia" || life.Fixed || !life.On {
		t.Fatalf("the life of a world is carried, switched and on: %v", life)
	}
	var kinds []string
	for _, op := range life.Plugin.Ops() {
		if op.Level != world.LevelAnonymous {
			t.Fatalf("a pet is anyone's to keep: %v", op)
		}
		kinds = append(kinds, op.Kind)
	}
	if got := strings.Join(kinds, " "); got != "tame name leave call release pets keep" {
		t.Fatalf("life offers a pet's ops: %s", got)
	}
	plugins := carried[3:]
	if len(plugins) != 5 || plugins[0].Plugin.Name() != "chat" || plugins[0].Plugin.Version() != 1 || !plugins[0].On || plugins[0].Fixed {
		t.Fatalf("this version carries chat, on, then building, items, ride and flashlight: %v", plugins)
	}
	if ops := plugins[0].Plugin.Ops(); len(ops) != 1 || ops[0] != (world.Op{Kind: "say", Level: world.LevelAnonymous}) {
		t.Fatalf("chat offers one op, to every level: %v", ops)
	}
	// Building is a hand's, and a hand is in the client: its world half
	// offers the world no op, and the gesture is the cells'.
	build := plugins[1].Plugin
	if build.Name() != "build" || build.Version() != 1 || !plugins[1].On || len(build.Ops()) != 0 {
		t.Fatalf("building is on and offers no op: %s %d %v", build.Name(), build.Version(), build.Ops())
	}
	// So is items: placing is a hand's, and the parts are the core's.
	items := plugins[2].Plugin
	if items.Name() != "items" || items.Version() != 1 || !plugins[2].On || len(items.Ops()) != 0 {
		t.Fatalf("items is on and offers no op: %s %d %v", items.Name(), items.Version(), items.Ops())
	}
	// And ride: a body in a way moves in the client, and goes out in its
	// stance.
	ride := plugins[3].Plugin
	if ride.Name() != "ride" || ride.Version() != 1 || !plugins[3].On || len(ride.Ops()) != 0 {
		t.Fatalf("ride is on and offers no op: %s %d %v", ride.Name(), ride.Version(), ride.Ops())
	}
	// And the flashlight: what the body's hand grasps goes out in its
	// stance too.
	flashlight := plugins[4].Plugin
	if flashlight.Name() != "flashlight" || flashlight.Version() != 1 || !plugins[4].On || len(flashlight.Ops()) != 0 {
		t.Fatalf("the flashlight is on and offers no op: %s %d %v", flashlight.Name(), flashlight.Version(), flashlight.Ops())
	}
}

// Chat's wire, written by its numbers: the server has no schema of a plugin.
const (
	scopeNear  = 0
	scopeWorld = 1
)

func say(scope uint64, text string) []byte {
	var line []byte
	line = protowire.AppendTag(line, 1, protowire.VarintType)
	line = protowire.AppendVarint(line, scope)
	line = protowire.AppendTag(line, 2, protowire.BytesType)
	return protowire.AppendString(line, text)
}

// text is the line a Said carries, its third field.
func text(t *testing.T, said []byte) string {
	t.Helper()
	for len(said) > 0 {
		number, kind, n := protowire.ConsumeTag(said)
		if n < 0 {
			t.Fatal("a said that is no message")
		}
		said = said[n:]
		if number == 3 && kind == protowire.BytesType {
			value, _ := protowire.ConsumeString(said)
			return value
		}
		said = said[protowire.ConsumeFieldValue(number, kind, said):]
	}
	return ""
}

// room is a world as a plugin sees it, keeping what each session heard.
type room struct {
	now   time.Time
	here  []world.Who
	heard map[uint32][]string
	// The recipe's params, as JSON. Empty is every default.
	params string
	// The code the last op was refused with.
	refused string
}

func (r *room) Now() time.Time     { return r.now }
func (r *room) Refuse(code string) { r.refused = code }
func (r *room) Recipe() world.Recipe {
	recipe, _ := world.NewRecipe("00000000deadbeef", 3, []byte(r.params))
	return recipe
}
func (r *room) Here() []world.Who { return r.here }
func (r *room) Tell(kind string, payload []byte, to func(world.Who) bool) {
	for _, who := range r.here {
		if to(who) {
			r.heard[who.Session] = append(r.heard[who.Session], kind+":"+string(payload))
		}
	}
}

const speaker, neighbour, far = 1, 2, 3

// three is the speaker, a neighbour sixty blocks along and someone seventy
// blocks along: one inside chat's reach, one outside it.
func three() (*room, world.Who) {
	at := func(session uint32, u float32) world.Who {
		return world.Who{Session: session, Stance: &pb.Stance{Sector: 2, U: u, V: 32768}}
	}
	who := at(speaker, 32768)
	return &room{
		now:   time.Unix(1790000000, 0),
		here:  []world.Who{who, at(neighbour, 32768+60), at(far, 32768+70)},
		heard: map[uint32][]string{},
	}, who
}

func TestALineCrossesTheBridgeAndReachesItsScope(t *testing.T) {
	m := loaded(t)
	plugin := chat(t, m)
	r, who := three()

	plugin.Do(r, who, "say", say(scopeNear, "hi"))
	for _, session := range []uint32{speaker, neighbour} {
		if heard := r.heard[session]; len(heard) != 1 || !strings.HasPrefix(heard[0], "said:") {
			t.Fatalf("a near line reaches the speaker and a neighbour: %v", r.heard)
		}
	}
	if len(r.heard[far]) != 0 {
		t.Fatalf("the far one hears no near line: the world half measures by the recipe: %v", r.heard[far])
	}
	plugin.Do(r, who, "say", say(scopeWorld, "all"))
	if heard := r.heard[far]; len(heard) != 1 {
		t.Fatalf("the far one hears the world line: %v", heard)
	}

	// The world half keeps its count between calls: the sixth line in a
	// burst is dropped, and a session that left is counted from nothing.
	for range 6 {
		plugin.Do(r, who, "say", say(scopeWorld, "again"))
	}
	if heard := len(r.heard[far]); heard != 1+3 {
		t.Fatalf("five lines in five seconds, the two above among them: %d", heard)
	}
	plugin.Gone(speaker)
	plugin.Do(r, who, "say", say(scopeWorld, "back"))
	if heard := len(r.heard[far]); heard != 1+3+1 {
		t.Fatalf("a session that left is counted from nothing: %d", heard)
	}
	if m.born != 1 {
		t.Fatalf("and all of it in one instance: %d", m.born)
	}
}

func TestARecipeTheGeneratorCannotReadStillHostsItsPlugins(t *testing.T) {
	// The server takes any object as a recipe's params. One the generator has
	// no shape for founds a world no client can draw, and its plugins are
	// hosted all the same: a line is still heard.
	plugin := chat(t, loaded(t))
	r, who := three()
	r.params = `{"source":"nowhere","sea":[1,2,3]}`
	plugin.Do(r, who, "say", say(scopeWorld, "anyone?"))
	if len(r.heard[far]) != 1 {
		t.Fatalf("a line in a world of a recipe nobody reads: %v", r.heard)
	}
}

// A module of two functions and a page of memory, written by hand: `reserve`
// answers address zero, and `call` is the body given.
func faulty(call ...byte) []byte {
	wasm := []byte{
		0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // \0asm, version 1
		0x01, 0x09, 0x02, 0x60, 0x01, 0x7f, 0x01, 0x7f, 0x60, 0x00, 0x00, // types: (i32) -> i32, () -> ()
		0x03, 0x03, 0x02, 0x00, 0x01, // functions: reserve, call
		0x05, 0x03, 0x01, 0x00, 0x01, // memory: one page
		0x07, 0x1b, 0x03, // exports
		0x06, 'm', 'e', 'm', 'o', 'r', 'y', 0x02, 0x00,
		0x07, 'r', 'e', 's', 'e', 'r', 'v', 'e', 0x00, 0x00,
		0x04, 'c', 'a', 'l', 'l', 0x00, 0x01,
	}
	reserve := []byte{0x04, 0x00, 0x41, 0x00, 0x0b} // i32.const 0
	body := append([]byte{byte(len(call) + 2), 0x00}, append(call, 0x0b)...)
	code := append([]byte{0x02}, append(reserve, body...)...)
	return append(wasm, append([]byte{0x0a, byte(len(code))}, code...)...)
}

func TestAFaultReplacesTheInstanceAndTheServerLives(t *testing.T) {
	describe := &bridge.Call{Call: &bridge.Call_Describe{Describe: &bridge.Describe{}}}
	for name, body := range map[string][]byte{
		"a world half that panics":        {0x00},                         // unreachable
		"a world half that never returns": {0x03, 0x40, 0x0c, 0x00, 0x0b}, // loop, br 0, end
	} {
		m, err := open(context.Background(), faulty(body...))
		if err != nil {
			t.Fatalf("%s: %v", name, err)
		}
		began := time.Now()
		if _, err := m.call(context.Background(), describe); err == nil {
			t.Fatalf("%s: the call is an error", name)
		}
		if took := time.Since(began); took > 4*deadline {
			t.Fatalf("%s: stopped within the deadline: %v", name, took)
		}
		if m.born != 2 || m.instance == nil || m.started != nil {
			t.Fatalf("%s: a new instance stands in its place, and the world starts again in it: born %d", name, m.born)
		}
		m.Close(context.Background())
	}
}

func TestAReplacedInstanceHostsTheWorldAgain(t *testing.T) {
	m := loaded(t)
	plugin := chat(t, m)
	r, who := three()
	plugin.Do(r, who, "say", say(scopeWorld, "before"))

	m.mu.Lock()
	if err := m.replace(context.Background()); err != nil {
		t.Fatal(err)
	}
	m.mu.Unlock()
	plugin.Do(r, who, "say", say(scopeWorld, "after"))
	heard := r.heard[far]
	if len(heard) != 2 || text(t, []byte(strings.TrimPrefix(heard[1], "said:"))) != "after" {
		t.Fatalf("the world is started in the new instance before its next op: %v", heard)
	}
	if !slices.Equal([]int{m.born}, []int{2}) {
		t.Fatalf("born %d", m.born)
	}
}

// What one line costs through the bridge, in a room of a hundred.
func BenchmarkALineInARoomOfAHundred(b *testing.B) {
	m, err := Load(context.Background(), nil, fields.Open(""))
	if err != nil {
		b.Fatal(err)
	}
	defer m.Close(context.Background())
	plugin := m.Plugins()[0].Plugin
	r := &room{now: time.Unix(1790000000, 0), heard: map[uint32][]string{}}
	for session := uint32(1); session <= 100; session++ {
		r.here = append(r.here, world.Who{Session: session, Stance: &pb.Stance{Sector: 2, U: 32768 + float32(session), V: 32768}})
	}
	line := say(scopeNear, "a line of chat, forty bytes or so long!!")
	for b.Loop() {
		// A second a line: the rate lets every one through.
		r.now = r.now.Add(time.Second)
		clear(r.heard)
		plugin.Do(r, r.here[0], "say", line)
	}
}

// A world on the Earth, as the module starts it: the recipe, and the field
// this checkout carries.
func onTheEarth(b testing.TB) (*Module, world.Recipe) {
	b.Helper()
	carried := fields.Open("../../../assets/fields")
	m, err := Load(context.Background(), nil, carried)
	if err != nil {
		b.Fatal(err)
	}
	b.Cleanup(func() { m.Close(context.Background()) })
	sidecar, err := os.ReadFile("../../../assets/fields/earth.json")
	if err != nil {
		b.Fatal(err)
	}
	var field struct{ ID string }
	if err := json.Unmarshal(sidecar, &field); err != nil {
		b.Fatal(err)
	}
	recipe, err := world.NewRecipe("0000000000000001", 3, []byte(`{"source":{"field":"`+field.ID+`"}}`))
	if err != nil {
		b.Fatal(err)
	}
	return m, recipe
}

// opens asks the module for a volume over the plot a plot's width apart
// from one by Aconcagua, and says whether it stands: an op nothing answers
// has landed.
func opens(t testing.TB, m *Module, east, north int) bool {
	t.Helper()
	u, v := 47008+float64(east)*64, 10272+float64(north)*64
	open, err := proto.Marshal(&cells.Open{Seat: &cells.Seat{Seat: &cells.Seat_Sector{Sector: 4}}, U: u, V: v})
	if err != nil {
		t.Fatal(err)
	}
	m.owner = "cells"
	op := &bridge.Op{Plugin: "cells", Kind: "open", Payload: open, Session: 1, NowMs: 1, Here: []*bridge.Who{{Session: 1, Level: pb.Level_LEVEL_ADMIN}}}
	replies, err := m.call(context.Background(), &bridge.Call{Call: &bridge.Call_Op{Op: op}})
	if err != nil {
		t.Fatal(err)
	}
	return !slices.ContainsFunc(replies, func(r *bridge.Reply) bool { return r.GetAnswered() != nil })
}

func TestANewInstanceStartsOnTheFieldAgain(t *testing.T) {
	m, recipe := onTheEarth(t)
	ctx := context.Background()
	if err := m.start(ctx, recipe); err != nil || !opens(t, m, 0, 0) {
		t.Fatalf("a volume stands over the Earth: %v", err)
	}
	if err := m.replace(ctx); err != nil {
		t.Fatal(err)
	}
	if err := m.start(ctx, recipe); err != nil || !opens(t, m, 1, 0) {
		t.Fatalf("the new instance is handed the field again: %v", err)
	}
}

// lost is a version whose field went before its world started, counting
// how often it is asked for it.
type lost struct{ asked int }

func (l *lost) For(world.Recipe) ([]byte, error) {
	l.asked++
	return nil, fields.ErrMissing
}

func TestAWorldThatDidNotStartRefusesEveryOp(t *testing.T) {
	gone := &lost{}
	m, err := Load(context.Background(), nil, gone)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { m.Close(context.Background()) })
	_, recipe := onTheEarth(t)
	r, who := three()
	r.params = string(recipe.Params)
	for range 3 {
		r.refused = ""
		chat(t, m).Do(r, who, "say", say(scopeWorld, "anyone?"))
		// Refused, the asker takes it back: an op nothing refuses has landed.
		if r.refused != "fault" || len(r.heard[far]) != 0 || m.started != nil {
			t.Fatalf("an op in a world that did not start: refused %q, heard %v", r.refused, r.heard)
		}
	}
	if gone.asked != 1 {
		t.Fatalf("a start that failed is not tried again: the field was asked for %d times", gone.asked)
	}
	if err := m.Start(context.Background(), recipe); !errors.Is(err, fields.ErrMissing) {
		t.Fatalf("it says why it did not start: %v", err)
	}
}

// What a start on the Earth costs: a new instance, as after a fault, and
// the field crossing into it to be made into ground. What the instance
// holds then is the module's memory in a world of a field.
func BenchmarkTheEarthStarts(b *testing.B) {
	m, recipe := onTheEarth(b)
	ctx := context.Background()
	for b.Loop() {
		if err := m.replace(ctx); err != nil {
			b.Fatal(err)
		}
		if err := m.start(ctx, recipe); err != nil {
			b.Fatal(err)
		}
	}
	b.ReportMetric(float64(m.instance.Memory().Size())/(1<<20), "MiB-held")
}

// What seating a volume on the Earth costs: the ground under a plot read in
// the module, a new plot each time, on the slopes by Aconcagua.
func BenchmarkAVolumeIsSeatedOverTheEarth(b *testing.B) {
	m, recipe := onTheEarth(b)
	ctx := context.Background()
	if err := m.start(ctx, recipe); err != nil {
		b.Fatal(err)
	}
	for plot := 0; b.Loop(); plot++ {
		// A volume opened again stands as it stood, with no ground read: a
		// new instance once the grid of plots is walked keeps every one new.
		if plot > 0 && plot%(64*64) == 0 {
			b.StopTimer()
			if err := m.replace(ctx); err != nil {
				b.Fatal(err)
			}
			if err := m.start(ctx, recipe); err != nil {
				b.Fatal(err)
			}
			b.StartTimer()
		}
		if !opens(b, m, plot%64, plot/64%64) {
			b.Fatal("a volume is seated")
		}
	}
}
