// Package fields is the fields a version carries: the ground of a world
// shaped by one, which the server hands the module to seat a volume by.
// The image carries them with the page's asset set, the files the browser
// fetches.
package fields

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"sync"

	"github.com/tekne-studio/planet/server/internal/world"
)

var (
	// ErrMissing is a field this version does not carry.
	ErrMissing = errors.New("this version does not carry the field")
	// ErrDamaged is a file whose bytes are not the ground its id names.
	ErrDamaged = errors.New("the field's bytes are not the ground its id names")
)

// Where a field file keeps its id: the 32 bytes after its 8 of magic, which
// its id is the hash of with them zeroed, as the page's `contentId` reads
// them (apps/web/src/lib/fields.ts).
const idStart, idEnd = 8, 40

// Fields is the fields in one folder. It holds the one its world's module
// is started with, so a new instance starts from memory after a fault.
type Fields struct {
	dir string

	mu   sync.Mutex
	id   string
	file []byte
}

// Open is the fields in a folder. An empty one, or one that does not exist,
// carries none.
func Open(dir string) *Fields {
	return &Fields{dir: dir}
}

// For is the file of the field the world's recipe names, checked against
// its id, and held from then on. Nil for a recipe that names none;
// ErrMissing or ErrDamaged, wrapped with the id and where it was looked
// for, for one this version cannot shape.
func (f *Fields) For(recipe world.Recipe) ([]byte, error) {
	return f.lookup(recipe, true)
}

// Check says whether this version can shape the ground a recipe names, as
// For does, holding nothing: a recipe asked to found a world may never be
// its world's.
func (f *Fields) Check(recipe world.Recipe) error {
	_, err := f.lookup(recipe, false)
	return err
}

func (f *Fields) lookup(recipe world.Recipe, hold bool) ([]byte, error) {
	id, named := recipe.Field()
	if !named {
		return nil, nil
	}
	f.mu.Lock()
	defer f.mu.Unlock()
	if f.id == id {
		return f.file, nil
	}
	file, err := f.find(id)
	if err == nil && hold {
		f.id, f.file = id, file
	}
	return file, err
}

// find reads the field the id names by the page's rule: the sidecar beside
// each `.field` says the id of its ground (apps/web/src/lib/fields.ts). The
// page reads the sidecars of the shapes it offers; the server reads every
// one in the folder. A file named by the id whose bytes are not its ground
// is passed over for another that is; with none, the first one says why.
// When no sidecar names the id, those that did not read say so with it.
func (f *Fields) find(id string) ([]byte, error) {
	missing := fmt.Errorf("the field %s in %q: %w", id, f.dir, ErrMissing)
	if f.dir == "" {
		return nil, missing
	}
	entries, err := os.ReadDir(f.dir)
	if errors.Is(err, fs.ErrNotExist) {
		return nil, missing
	}
	if err != nil {
		return nil, fmt.Errorf("the fields in %q: %w", f.dir, err)
	}
	var failed error
	var unread []string
	for _, entry := range entries {
		name := entry.Name()
		if entry.IsDir() || filepath.Ext(name) != ".json" {
			continue
		}
		named, err := sidecar(filepath.Join(f.dir, name))
		if err != nil {
			unread = append(unread, err.Error())
			continue
		}
		if named != id {
			continue
		}
		path := filepath.Join(f.dir, strings.TrimSuffix(name, ".json")+".field")
		file, err := os.ReadFile(path)
		switch {
		case errors.Is(err, fs.ErrNotExist):
			err = fmt.Errorf("the field %s at %q: %w", id, path, ErrMissing)
		case err != nil:
			err = fmt.Errorf("the field %s: %w", id, err)
		case contentID(file) != id:
			err = fmt.Errorf("the field %s at %q: %w", id, path, ErrDamaged)
		default:
			return file, nil
		}
		if failed == nil {
			failed = err
		}
	}
	if failed != nil {
		return nil, failed
	}
	if len(unread) > 0 {
		return nil, fmt.Errorf("%w; %s", missing, strings.Join(unread, "; "))
	}
	return nil, missing
}

// sidecar is the id a field's sidecar names, read by its key exactly as the
// page reads it, or nothing for one that names none.
func sidecar(path string) (string, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	var keys map[string]json.RawMessage
	if err := json.Unmarshal(raw, &keys); err != nil {
		return "", fmt.Errorf("%q: %w", path, err)
	}
	var id string
	if json.Unmarshal(keys["id"], &id) != nil {
		return "", nil
	}
	return id, nil
}

// contentID is a field's id, as hex: the sha256 of its file with the id's
// own bytes zeroed, so it covers the magic and every plane: a format that
// reads the same planes by another rule takes another id. The same rule as
// the page's `contentId`.
func contentID(file []byte) string {
	if len(file) < idEnd {
		return ""
	}
	sum := sha256.New()
	sum.Write(file[:idStart])
	sum.Write(make([]byte, idEnd-idStart))
	sum.Write(file[idEnd:])
	return hex.EncodeToString(sum.Sum(nil))
}
