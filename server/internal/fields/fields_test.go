package fields

import (
	"encoding/json"
	"errors"
	"io/fs"
	"maps"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/tekne-studio/planet/server/internal/world"
)

// The fields every checkout carries, as the image does.
const carried = "../../../assets/fields"

// earth is the id the Earth's sidecar names, as the page reads it.
func earth(t *testing.T) string {
	t.Helper()
	raw, err := os.ReadFile(filepath.Join(carried, "earth.json"))
	if err != nil {
		t.Fatal(err)
	}
	var sidecar struct{ ID string }
	if err := json.Unmarshal(raw, &sidecar); err != nil {
		t.Fatal(err)
	}
	return sidecar.ID
}

func on(t *testing.T, id string) world.Recipe {
	t.Helper()
	recipe, err := world.NewRecipe("0000000000000001", 3, []byte(`{"source":{"field":"`+id+`"}}`))
	if err != nil {
		t.Fatal(err)
	}
	return recipe
}

func TestTheEarthIsFoundByItsIdAndChecked(t *testing.T) {
	id := earth(t)
	file, err := Open(carried).For(on(t, id))
	if err != nil {
		t.Fatal(err)
	}
	if contentID(file) != id {
		t.Fatalf("the bytes handed over are the ground the id names: %s", contentID(file))
	}
}

func TestARecipeThatNamesNoFieldIsHandedNone(t *testing.T) {
	recipe, _ := world.NewRecipe("0000000000000001", 3, []byte(`{"source":"generated"}`))
	if file, err := Open("").For(recipe); file != nil || err != nil {
		t.Fatalf("%d bytes, %v", len(file), err)
	}
}

func TestAFieldThisVersionDoesNotCarryIsMissing(t *testing.T) {
	other := "00000000000000000000000000000000000000000000000000000000000000aa"
	for name, fields := range map[string]*Fields{"another id": Open(carried), "no folder": Open(""), "an empty folder": Open(t.TempDir())} {
		if _, err := fields.For(on(t, other)); !errors.Is(err, ErrMissing) {
			t.Errorf("%s: %v", name, err)
		}
	}
	if _, err := Open("").For(on(t, earth(t))); !errors.Is(err, ErrMissing) {
		t.Errorf("a version that carries no folder: %v", err)
	}
}

func TestAFileThatIsNotTheGroundItNamesIsDamaged(t *testing.T) {
	file, err := os.ReadFile(filepath.Join(carried, "earth.field"))
	if err != nil {
		t.Fatal(err)
	}
	// One texel of ruggedness changed, the id left as it was.
	file[len(file)-1] ^= 1
	dir := t.TempDir()
	path := beside(t, dir, nil)
	if err := os.WriteFile(path, file, 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := Open(dir).For(on(t, earth(t))); !errors.Is(err, ErrDamaged) {
		t.Fatalf("%v", err)
	}
}

func TestTheFieldIsHeldOnceFound(t *testing.T) {
	path := beside(t, t.TempDir(), nil)
	file, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	fields := Open(filepath.Dir(path))
	if _, err := fields.For(on(t, earth(t))); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	if again, err := fields.For(on(t, earth(t))); err != nil || len(again) != len(file) {
		t.Fatalf("a module started again reads it from memory: %d bytes, %v", len(again), err)
	}
}

// beside writes the Earth and its sidecar into a folder, with what else a
// folder of assets may hold beside them, and says where the Earth is.
func beside(t *testing.T, dir string, others map[string][]byte) string {
	t.Helper()
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	files := map[string][]byte{}
	for _, name := range []string{"earth.field", "earth.json"} {
		bytes, err := os.ReadFile(filepath.Join(carried, name))
		if err != nil {
			t.Fatal(err)
		}
		files[name] = bytes
	}
	maps.Copy(files, others)
	for name, bytes := range files {
		if err := os.WriteFile(filepath.Join(dir, name), bytes, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	return filepath.Join(dir, "earth.field")
}

func TestTheEarthIsFoundAmongFilesThatSayNothing(t *testing.T) {
	// A folder whose name a glob would read as a pattern.
	dir := filepath.Join(t.TempDir(), "fields[1]")
	beside(t, dir, map[string][]byte{
		"notes.json":   []byte(`{"source": "a note, no id"}`),
		"other.json":   []byte(`{"id": "00000000000000000000000000000000000000000000000000000000000000aa"}`),
		"broken.json":  []byte("{"),
		"stray.field":  []byte("PLFIELD2"),
		"preview.webp": nil,
	})
	if err := os.Mkdir(filepath.Join(dir, "old.json"), 0o755); err != nil {
		t.Fatal(err)
	}
	if file, err := Open(dir).For(on(t, earth(t))); err != nil || len(file) == 0 {
		t.Fatalf("a sidecar of no field, or one that does not read, names nothing: %v", err)
	}
}

func TestAFieldIsFoundByItsSidecarAsThePageFindsIt(t *testing.T) {
	dir := t.TempDir()
	path := beside(t, dir, nil)
	// A copy under another name, with no sidecar: the page never fetches it.
	if err := os.Rename(path, filepath.Join(dir, "copy.field")); err != nil {
		t.Fatal(err)
	}
	if _, err := Open(dir).For(on(t, earth(t))); !errors.Is(err, ErrMissing) {
		t.Fatalf("a field no sidecar names: %v", err)
	}

	// A sidecar whose key is not `id` exactly names nothing, as for the page.
	dir = t.TempDir()
	beside(t, dir, nil)
	shouting := []byte(`{"ID": "` + earth(t) + `"}`)
	if err := os.WriteFile(filepath.Join(dir, "earth.json"), shouting, 0o644); err != nil {
		t.Fatal(err)
	}
	if _, err := Open(dir).For(on(t, earth(t))); !errors.Is(err, ErrMissing) {
		t.Fatalf("a sidecar that says ID: %v", err)
	}
}

func TestADamagedCopyIsPassedOverForTheGround(t *testing.T) {
	dir := t.TempDir()
	path := beside(t, dir, nil)
	file, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	sidecar, err := os.ReadFile(filepath.Join(dir, "earth.json"))
	if err != nil {
		t.Fatal(err)
	}
	// A damaged copy of the same ground, named to be read first.
	damaged := append([]byte(nil), file...)
	damaged[len(damaged)-1] ^= 1
	if err := os.WriteFile(filepath.Join(dir, "a.field"), damaged, 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "a.json"), sidecar, 0o644); err != nil {
		t.Fatal(err)
	}
	if found, err := Open(dir).For(on(t, earth(t))); err != nil || contentID(found) != earth(t) {
		t.Fatalf("the ground is found past a damaged copy: %v", err)
	}
}

func TestAFieldThatDoesNotReadSaysWhy(t *testing.T) {
	if os.Getuid() == 0 {
		t.Skip("root reads a locked file")
	}
	dir := t.TempDir()
	path := beside(t, dir, nil)
	if err := os.Chmod(path, 0o000); err != nil {
		t.Fatal(err)
	}
	_, err := Open(dir).For(on(t, earth(t)))
	if errors.Is(err, ErrMissing) || !errors.Is(err, fs.ErrPermission) {
		t.Fatalf("the world's own field, locked: %v", err)
	}

	// A sidecar that does not read is said beside a field found nowhere.
	dir = t.TempDir()
	beside(t, dir, nil)
	if err := os.Chmod(filepath.Join(dir, "earth.json"), 0o000); err != nil {
		t.Fatal(err)
	}
	_, err = Open(dir).For(on(t, earth(t)))
	if !errors.Is(err, ErrMissing) || !strings.Contains(err.Error(), "permission denied") {
		t.Fatalf("its sidecar, locked: %v", err)
	}
}

func TestACheckHoldsNothing(t *testing.T) {
	dir := t.TempDir()
	path := beside(t, dir, nil)
	fields := Open(dir)
	if err := fields.Check(on(t, earth(t))); err != nil {
		t.Fatal(err)
	}
	other := "00000000000000000000000000000000000000000000000000000000000000aa"
	if err := fields.Check(on(t, other)); !errors.Is(err, ErrMissing) {
		t.Fatalf("a field this version does not carry: %v", err)
	}
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	if _, err := fields.For(on(t, earth(t))); !errors.Is(err, ErrMissing) {
		t.Fatalf("a field checked is read again by the world that starts on it: %v", err)
	}

	// What the world holds, a check of another field leaves held.
	beside(t, dir, nil)
	if _, err := fields.For(on(t, earth(t))); err != nil {
		t.Fatal(err)
	}
	_ = fields.Check(on(t, other))
	if err := os.Remove(path); err != nil {
		t.Fatal(err)
	}
	if _, err := fields.For(on(t, earth(t))); err != nil {
		t.Fatalf("the world's field is still held: %v", err)
	}
}
