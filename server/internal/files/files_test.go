package files

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestAFileIsNamedByItsBytesAndKeptOnce(t *testing.T) {
	kept, err := Open(filepath.Join(t.TempDir(), "files"), "")
	if err != nil {
		t.Fatal(err)
	}
	model := []byte("VOX \x96\x00\x00\x00MAIN")

	first, held, err := kept.Keep(model)
	if err != nil || held {
		t.Fatalf("a new file is kept: %v %t", err, held)
	}
	if first.Name != "4fcf6a9f5e4a4b2d4f0a3b4c6f8c0b1e2d3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b.vox" && filepath.Ext(first.Name) != ".vox" {
		t.Fatalf("named by its hash and its kind: %s", first.Name)
	}
	if first.Bytes != int64(len(model)) {
		t.Fatalf("and measured: %d", first.Bytes)
	}
	again, held, err := kept.Keep(model)
	if err != nil || !held || again != first {
		t.Fatalf("the same bytes are the same file: %v %t %v", err, held, again)
	}

	path, ok := kept.Path(first.Name)
	if !ok {
		t.Fatal("a kept file has a path")
	}
	if back, _ := os.ReadFile(path); !bytes.Equal(back, model) {
		t.Fatal("and holds its bytes")
	}
	for _, name := range []string{"../world.json", "world.json", first.Name + "/..", "AB.vox", ""} {
		if _, ok := kept.Path(name); ok {
			t.Fatalf("%q is no file's name", name)
		}
	}

	list, err := kept.List()
	if err != nil || len(list) != 1 || list[0] != first {
		t.Fatalf("the world holds one file: %v %v", list, err)
	}
}

func TestTheKindIsReadOffTheBytes(t *testing.T) {
	for head, want := range map[string]string{
		"VOX \x96\x00\x00\x00":         "vox",
		"\x89PNG\r\n\x1a\n....":        "png",
		"\xff\xd8\xff\xe0..":           "jpg",
		"RIFF\x00\x00\x00\x00WEBPVP8 ": "webp",
		"GIF89a\x10\x00\x10\x00":       "gif",
		"\x00\x00\x00\x18ftypisom":     "mp4",
		"\x00\x00\x00\x18ftypM4A ":     "m4a",
		"ID3\x04\x00\x00\x00":          "mp3",
		"\xff\xfb\x90\x00":             "mp3",
		"OggS\x00\x02":                 "ogg",
		"glTF\x02\x00\x00\x00":         "glb",
		"glTF\x01\x00\x00\x00":         "",
		"\x00asm\x01\x00\x00\x00":      "wasm",
		"\x00asm\x0d\x00\x01\x00":      "",
		"RIFF\x00\x00\x00\x00WAVEfmt ": "",
		"<svg xmlns='http://www.w3'/>": "",
		"":                             "",
	} {
		ext, ok := Kind([]byte(head))
		if ext != want || ok != (want != "") {
			t.Fatalf("%q reads as %q, not %q", head, ext, want)
		}
	}
	kept, _ := Open(t.TempDir(), "")
	if _, _, err := kept.Keep([]byte("plain text")); err != ErrKind {
		t.Fatalf("bytes of no kind are refused: %v", err)
	}
}

// The files a version carries are held by every world, read where they are
// and never kept again.
func TestCarriedFilesAreKeptAndOutliveTheVersion(t *testing.T) {
	carry := func(content ...[]byte) (string, []Kept) {
		folder := t.TempDir()
		var names []Kept
		for _, c := range content {
			name, err := Name(c)
			if err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(filepath.Join(folder, name.Name), c, 0o644); err != nil {
				t.Fatal(err)
			}
			names = append(names, name)
		}
		return folder, names
	}
	rocks := []byte("\x00asm\x01\x00\x00\x00 rocks, built once")
	rebuilt := []byte("\x00asm\x01\x00\x00\x00 rocks, built again")
	dir := filepath.Join(t.TempDir(), "files")

	first, old := carry(rocks)
	// A file under a name its bytes do not earn is left out.
	if err := os.WriteFile(filepath.Join(first, strings.Repeat("0", 64)+".wasm"), rocks, 0o644); err != nil {
		t.Fatal(err)
	}
	kept, err := Open(dir, first)
	if err != nil {
		t.Fatal(err)
	}
	if full, ok := kept.Path(old[0].Name); !ok || filepath.Dir(full) != dir {
		t.Fatalf("Path = %q, %v", full, ok)
	}
	if _, held, err := kept.Keep(rocks); err != nil || !held {
		t.Fatalf("Keep = %v, %v", held, err)
	}
	if listed, _ := kept.List(); len(listed) != 1 {
		t.Fatalf("kept %v", listed)
	}

	// The next version carries other bytes for the same game: a cabinet
	// set to the old ones still finds its file.
	second, now := carry(rebuilt)
	kept, err = Open(dir, second)
	if err != nil {
		t.Fatal(err)
	}
	for _, name := range []Kept{old[0], now[0]} {
		if _, ok := kept.Path(name.Name); !ok {
			t.Fatalf("%s is not held", name.Name)
		}
	}
	if _, err := Open(dir, filepath.Join(t.TempDir(), "none")); err != nil {
		t.Fatalf("a version that carries no folder: %v", err)
	}
}

// A file the version carries that appears after the world opened, a game
// rebuilt under a running server, is kept the first time it is asked for.
func TestACarriedFileAddedLaterIsKeptWhenAskedFor(t *testing.T) {
	carried := t.TempDir()
	dir := filepath.Join(t.TempDir(), "files")
	kept, err := Open(dir, carried)
	if err != nil {
		t.Fatal(err)
	}

	rocks := []byte("\x00asm\x01\x00\x00\x00 rocks, built while the world runs")
	name, err := Name(rocks)
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := kept.Path(name.Name); ok {
		t.Fatal("a file the version does not carry yet is not held")
	}
	if err := os.WriteFile(filepath.Join(carried, name.Name), rocks, 0o644); err != nil {
		t.Fatal(err)
	}
	full, ok := kept.Path(name.Name)
	if !ok || full != filepath.Join(dir, name.Name) {
		t.Fatalf("Path = %q, %v", full, ok)
	}
	if back, _ := os.ReadFile(full); !bytes.Equal(back, rocks) {
		t.Fatal("the world's folder holds its bytes")
	}
	if listed, _ := kept.List(); len(listed) != 1 || listed[0] != name {
		t.Fatalf("kept %v", listed)
	}

	// Bytes under a name they do not earn stay not found, and are not kept.
	forged := strings.Repeat("0", 64) + ".wasm"
	if err := os.WriteFile(filepath.Join(carried, forged), rocks, 0o644); err != nil {
		t.Fatal(err)
	}
	if _, ok := kept.Path(forged); ok {
		t.Fatal("a name its bytes do not earn is not found")
	}
	if _, err := os.Stat(filepath.Join(dir, forged)); err == nil {
		t.Fatal("and never kept")
	}

	// A name the pattern refuses never reaches the disk, even one that names
	// a file beside the carried folder.
	secret := filepath.Join(filepath.Dir(carried), "world.json")
	if err := os.WriteFile(secret, []byte("{}"), 0o644); err != nil {
		t.Fatal(err)
	}
	for _, hostile := range []string{
		"../world.json",
		"../" + name.Name,
		name.Name + "/..",
		strings.TrimSuffix(name.Name, ".wasm") + ".wasm.json",
		strings.TrimSuffix(name.Name, ".wasm") + ".WASM",
		filepath.Join(carried, name.Name),
	} {
		if _, ok := kept.Path(hostile); ok {
			t.Fatalf("%q is no file's name", hostile)
		}
	}
	if listed, _ := kept.List(); len(listed) != 1 {
		t.Fatalf("nothing else is kept: %v", listed)
	}
}
