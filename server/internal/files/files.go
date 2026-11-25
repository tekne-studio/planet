// Package files is what a world keeps of heavy files: a voxel model, a 3D
// model, an image, a video, a sound, a cartridge, each named by the hash of
// its content and never changed.
// A session that may build sends one; the world reads its kind off its bytes,
// names it and keeps it. Alone, the files sit in the world folder and the
// server serves them; mundos's bucket takes them when it hands a world one.
// Every world holds the files its version carries too, the games it ships:
// copied into the folder when the world opens, and a file added there later
// the first time it is asked for, so a record that names one still finds it
// after an upgrade carries other bytes.
package files

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io/fs"
	"log"
	"os"
	"path/filepath"
	"regexp"
	"sort"
)

// Cap is the most bytes one file may have: a phone photo is 3 to 8 MB, and
// a model this large is a build, not a model.
const Cap = 20 << 20

// ErrKind is what bytes of no kind the world takes hear.
var ErrKind = errors.New("files: no kind the world takes")

// named is a name a file may have: its hash and its kind, nothing a person
// chose.
var named = regexp.MustCompile(`^[0-9a-f]{64}\.[a-z0-9]{2,5}$`)

// Kind is the extension bytes earn, read off their first bytes and never off
// a name. False for bytes of no kind the world takes.
func Kind(head []byte) (string, bool) {
	switch {
	case bytes.HasPrefix(head, []byte("VOX ")):
		return "vox", true
	// A binary glTF opens with `glTF` and its version, a little endian 2:
	// version 1 is a format of its own no engine reads.
	case bytes.HasPrefix(head, []byte("glTF\x02\x00\x00\x00")):
		return "glb", true
	case bytes.HasPrefix(head, []byte("\x89PNG\r\n\x1a\n")):
		return "png", true
	case bytes.HasPrefix(head, []byte{0xff, 0xd8, 0xff}):
		return "jpg", true
	case len(head) >= 12 && bytes.Equal(head[:4], []byte("RIFF")) && bytes.Equal(head[8:12], []byte("WEBP")):
		return "webp", true
	case bytes.HasPrefix(head, []byte("GIF87a")), bytes.HasPrefix(head, []byte("GIF89a")):
		return "gif", true
	// An MP4 and an M4A open with a box whose type is `ftyp`, after its four
	// byte size; the brand after says which.
	case len(head) >= 12 && bytes.Equal(head[4:8], []byte("ftyp")) && bytes.Equal(head[8:12], []byte("M4A ")):
		return "m4a", true
	case len(head) >= 8 && bytes.Equal(head[4:8], []byte("ftyp")):
		return "mp4", true
	// An MP3 opens with an ID3 tag, or straight with a frame's sync bits.
	case bytes.HasPrefix(head, []byte("ID3")), len(head) >= 2 && head[0] == 0xff && head[1]&0xe0 == 0xe0:
		return "mp3", true
	case bytes.HasPrefix(head, []byte("OggS")):
		return "ogg", true
	// A WebAssembly module opens with `\0asm` and its version, a little
	// endian 1: a cartridge, which the arcade judges before it is kept. A
	// component, of another version, is none.
	case bytes.HasPrefix(head, []byte("\x00asm\x01\x00\x00\x00")):
		return "wasm", true
	}
	return "", false
}

// Kept is one file the world holds.
type Kept struct {
	// `<hash>.<ext>`: what a record names after `asset://`.
	Name  string
	Bytes int64
}

// Files is the folder the files are kept in.
type Files struct {
	dir string
	// carried is the folder of the files the version carries, read only; ""
	// carries none.
	carried string
}

// Open makes the folder when it is missing, and keeps in it every file the
// version carries that it lacks. `carried` is the folder of those files,
// read only; "" carries none. A file there under a name its bytes do not
// earn is left out.
func Open(dir, carried string) (*Files, error) {
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, fmt.Errorf("files: %w", err)
	}
	f := &Files{dir: dir, carried: carried}
	if carried == "" {
		return f, nil
	}
	entries, err := os.ReadDir(carried)
	if errors.Is(err, fs.ErrNotExist) {
		return f, nil
	}
	if err != nil {
		return nil, fmt.Errorf("files: %w", err)
	}
	for _, entry := range entries {
		if !named.MatchString(entry.Name()) {
			continue
		}
		if _, held := f.held(entry.Name()); held {
			continue
		}
		if _, _, err := f.carry(entry.Name()); err != nil {
			return nil, err
		}
	}
	return f, nil
}

// carry keeps the carried file of a name the pattern allows, and says where
// it now is. False, with no error, when the version carries none by that
// name: missing, not a regular file, or bytes that do not earn the name.
func (f *Files) carry(name string) (string, bool, error) {
	if f.carried == "" {
		return "", false, nil
	}
	from := filepath.Join(f.carried, name)
	info, err := os.Lstat(from)
	if errors.Is(err, fs.ErrNotExist) {
		return "", false, nil
	}
	if err != nil {
		return "", false, fmt.Errorf("files: %w", err)
	}
	if !info.Mode().IsRegular() {
		return "", false, nil
	}
	content, err := os.ReadFile(from)
	if err != nil {
		return "", false, fmt.Errorf("files: %w", err)
	}
	if earned, err := Name(content); err != nil || earned.Name != name {
		return "", false, nil
	}
	if _, _, err := f.Keep(content); err != nil {
		return "", false, err
	}
	return filepath.Join(f.dir, name), true, nil
}

// Name is the name bytes earn, their hash and their kind.
func Name(content []byte) (Kept, error) {
	ext, ok := Kind(content)
	if !ok {
		return Kept{}, ErrKind
	}
	sum := sha256.Sum256(content)
	return Kept{Name: hex.EncodeToString(sum[:]) + "." + ext, Bytes: int64(len(content))}, nil
}

// Keep names bytes and keeps them, and says whether the world held them
// already. The file is written whole beside its name and moved into place,
// so a reader sees a file or none.
func (f *Files) Keep(content []byte) (Kept, bool, error) {
	kept, err := Name(content)
	if err != nil {
		return Kept{}, false, err
	}
	if _, held := f.held(kept.Name); held {
		return kept, true, nil
	}
	full := filepath.Join(f.dir, kept.Name)
	temp, err := os.CreateTemp(f.dir, ".keep-*")
	if err != nil {
		return Kept{}, false, fmt.Errorf("files: %w", err)
	}
	if _, err := temp.Write(content); err != nil {
		temp.Close()
		os.Remove(temp.Name())
		return Kept{}, false, fmt.Errorf("files: %w", err)
	}
	if err := temp.Sync(); err != nil {
		temp.Close()
		os.Remove(temp.Name())
		return Kept{}, false, fmt.Errorf("files: %w", err)
	}
	if err := temp.Close(); err != nil {
		os.Remove(temp.Name())
		return Kept{}, false, fmt.Errorf("files: %w", err)
	}
	if err := os.Rename(temp.Name(), full); err != nil {
		os.Remove(temp.Name())
		return Kept{}, false, fmt.Errorf("files: %w", err)
	}
	return kept, false, nil
}

// Holds says whether the world holds these bytes.
func (f *Files) Holds(content []byte) bool {
	kept, err := Name(content)
	if err != nil {
		return false
	}
	_, held := f.Path(kept.Name)
	return held
}

// Path is where a named file is, for one the world holds. A file the
// version carries that the folder lacks, added after the world opened, is
// kept on this first ask. False for a name that is no file's, or one the
// world does not hold: nothing a request says reaches a folder but a hash
// and a kind.
func (f *Files) Path(name string) (string, bool) {
	if !named.MatchString(name) {
		return "", false
	}
	if full, held := f.held(name); held {
		return full, true
	}
	full, carried, err := f.carry(name)
	if err != nil {
		log.Printf("%v", err)
	}
	return full, carried
}

// held is where a file of a name the pattern allows is in the folder.
func (f *Files) held(name string) (string, bool) {
	full := filepath.Join(f.dir, name)
	if info, err := os.Stat(full); err == nil && info.Mode().IsRegular() {
		return full, true
	}
	return "", false
}

// List is every file the world holds, by name.
func (f *Files) List() ([]Kept, error) {
	entries, err := os.ReadDir(f.dir)
	if err != nil {
		return nil, fmt.Errorf("files: %w", err)
	}
	var kept []Kept
	for _, entry := range entries {
		if !named.MatchString(entry.Name()) {
			continue
		}
		info, err := entry.Info()
		if errors.Is(err, fs.ErrNotExist) {
			continue
		}
		if err != nil {
			return nil, fmt.Errorf("files: %w", err)
		}
		kept = append(kept, Kept{Name: entry.Name(), Bytes: info.Size()})
	}
	sort.Slice(kept, func(i, j int) bool { return kept[i].Name < kept[j].Name })
	return kept, nil
}
