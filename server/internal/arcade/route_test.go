package arcade

import (
	"bytes"
	"encoding/json"
	"log"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/tekne-studio/planet/server/internal/api"
	"github.com/tekne-studio/planet/server/internal/fields"
	"github.com/tekne-studio/planet/server/internal/folder"
	"github.com/tekne-studio/planet/server/internal/world"
)

// upload posts `file` as a session of a world alone does.
func upload(t *testing.T, web *httptest.Server, file []byte) (int, map[string]any) {
	t.Helper()
	response, err := http.Post(web.URL+"/api/files", "application/octet-stream", bytes.NewReader(file))
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	var answer map[string]any
	_ = json.NewDecoder(response.Body).Decode(&answer)
	return response.StatusCode, answer
}

// heldBy is the files the world holds.
func heldBy(t *testing.T, web *httptest.Server) []any {
	t.Helper()
	response, err := http.Get(web.URL + "/api/files")
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	var held []any
	if err := json.NewDecoder(response.Body).Decode(&held); response.StatusCode != http.StatusOK || err != nil {
		t.Fatalf("the files the world holds: %d %v", response.StatusCode, err)
	}
	return held
}

// The server judges an upload by the pinned runner before it keeps it: a
// file of the cases that every host refuses is refused with its code, in
// the answer and in the server's log, and never kept; a ROM is kept.
func TestTheUploadRouteKeepsOnlyWhatTheRunnerKeeps(t *testing.T) {
	kept, err := folder.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	server, err := api.New(api.Config{DevLevel: world.LevelBuilder, Version: "test"}, kept, nil,
		fields.Open("../../../assets/fields"), loaded(t))
	if err != nil {
		t.Fatal(err)
	}
	web := httptest.NewServer(server.Handler())
	t.Cleanup(web.Close)

	var said strings.Builder
	log.SetOutput(&said)
	t.Cleanup(func() { log.SetOutput(os.Stderr) })
	refused := 0
	for _, c := range readCases(t) {
		if *c.Check == "" {
			continue
		}
		status, answer := upload(t, web, c.bytes)
		// What is not a WebAssembly module is no kind the world keeps,
		// and is refused before any judge.
		if !strings.HasPrefix(string(c.bytes), "\x00asm\x01\x00\x00\x00") {
			if status != http.StatusUnsupportedMediaType {
				t.Fatalf("%s: %d %v, not of a kind kept", c.Test, status, answer)
			}
			continue
		}
		if status != http.StatusUnprocessableEntity || answer["error"] != *c.Check {
			t.Fatalf("%s: %d %v, not refused as %q", c.Test, status, answer, *c.Check)
		}
		if !strings.Contains(said.String(), "is refused, "+*c.Check+",") {
			t.Fatalf("%s: the server's log says no refusal: %q", c.Test, said.String())
		}
		refused++
	}
	if refused == 0 {
		t.Fatal("no case is a refused cartridge")
	}
	if held := heldBy(t, web); len(held) != 0 {
		t.Fatalf("the world holds no file refused: %v", held)
	}
	rom, err := filepath.Glob(filepath.Join(romFolder, "*.wasm"))
	if err != nil || len(rom) == 0 {
		t.Fatalf("no ROM: %v", err)
	}
	file, err := os.ReadFile(rom[0])
	if err != nil {
		t.Fatal(err)
	}
	if status, answer := upload(t, web, file); status != http.StatusCreated || !strings.HasSuffix(answer["asset"].(string), ".wasm") {
		t.Fatalf("a ROM is kept: %d %v", status, answer)
	}
	if held := heldBy(t, web); len(held) != 1 {
		t.Fatalf("the world holds the ROM alone: %v", held)
	}
	t.Logf("%d cartridges of the cases refused by the route, each with its code", refused)
}
