package api

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/tekne-studio/planet/server/internal/fields"
	"github.com/tekne-studio/planet/server/internal/files"
	"github.com/tekne-studio/planet/server/internal/folder"
	"github.com/tekne-studio/planet/server/internal/world"
)

// post sends bytes as a file, with a key.
func post(t *testing.T, web *httptest.Server, key string, content []byte) (int, map[string]any) {
	t.Helper()
	request, _ := http.NewRequest("POST", web.URL+"/api/files", bytes.NewReader(content))
	if key != "" {
		request.Header.Set("Authorization", "Bearer "+key)
	}
	response, err := http.DefaultClient.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	defer response.Body.Close()
	var answer map[string]any
	_ = json.NewDecoder(response.Body).Decode(&answer)
	return response.StatusCode, answer
}

func TestABuilderUploadsAFileAndTheWorldServesItByItsHash(t *testing.T) {
	web, private := hosted(t)
	builder := enter(t, web, token(t, private, "builder"))["key"].(string)
	visitor := enter(t, web, token(t, private, "signed_in"))["key"].(string)
	model := []byte("VOX \x96\x00\x00\x00MAIN\x00\x00\x00\x00\x00\x00\x00\x00")

	if status, why := post(t, web, "", model); status != http.StatusForbidden || why["error"] != "level" {
		t.Fatalf("a guest sends no file: %d %v", status, why)
	}
	if status, why := post(t, web, visitor, model); status != http.StatusForbidden || why["error"] != "level" {
		t.Fatalf("a signed in person with no role sends no file: %d %v", status, why)
	}
	if status, why := post(t, web, "a-key-nobody-holds", model); status != http.StatusUnauthorized || why["error"] != "key" {
		t.Fatalf("an unknown key: %d %v", status, why)
	}
	if status, why := post(t, web, builder, []byte("just text")); status != http.StatusUnsupportedMediaType || why["error"] != "kind" {
		t.Fatalf("bytes of no kind: %d %v", status, why)
	}
	if status, why := post(t, web, builder, append([]byte("VOX "), make([]byte, files.Cap)...)); status != http.StatusRequestEntityTooLarge || why["error"] != "size" {
		t.Fatalf("over the cap: %d %v", status, why)
	}

	status, kept := post(t, web, builder, model)
	name, _ := kept["asset"].(string)
	if status != http.StatusCreated || !strings.HasPrefix(name, "asset://") || !strings.HasSuffix(name, ".vox") || kept["bytes"] != float64(len(model)) {
		t.Fatalf("a builder's file is kept and named: %d %v", status, kept)
	}
	if status, again := post(t, web, builder, model); status != http.StatusOK || again["asset"] != name {
		t.Fatalf("the same bytes are the same file: %d %v", status, again)
	}

	_, about := call(t, "GET", web.URL+"/api/world", "", nil)
	if about["files"] != "/files" {
		t.Fatalf("the world says where its files are: %v", about)
	}
	response, err := http.Get(web.URL + "/files/" + strings.TrimPrefix(name, "asset://"))
	if err != nil {
		t.Fatal(err)
	}
	body, _ := io.ReadAll(response.Body)
	response.Body.Close()
	if response.StatusCode != http.StatusOK || !bytes.Equal(body, model) {
		t.Fatalf("the file is served: %d %q", response.StatusCode, body)
	}
	if response.Header.Get("Cache-Control") != forGood {
		t.Fatalf("for good: %s", response.Header.Get("Cache-Control"))
	}
	for _, path := range []string{"/files/world.json", "/files/..%2Fworld.json", "/files/" + strings.Repeat("0", 64) + ".vox"} {
		response, err := http.Get(web.URL + path)
		if err != nil {
			t.Fatal(err)
		}
		response.Body.Close()
		if response.StatusCode != http.StatusNotFound {
			t.Fatalf("%s is nothing: %d", path, response.StatusCode)
		}
	}

	if status, _ := call(t, "GET", web.URL+"/api/files", visitor, nil); status != http.StatusForbidden {
		t.Fatalf("the list is a builder's: %d", status)
	}
	request, _ := http.NewRequest("GET", web.URL+"/api/files", nil)
	request.Header.Set("Authorization", "Bearer "+builder)
	response, err = http.DefaultClient.Do(request)
	if err != nil {
		t.Fatal(err)
	}
	var held []map[string]any
	_ = json.NewDecoder(response.Body).Decode(&held)
	response.Body.Close()
	if len(held) != 1 || held[0]["asset"] != name {
		t.Fatalf("the world holds one file: %v", held)
	}
}

// counted keeps every file but the ones it says, and counts the files it
// was asked of. One it cannot judge it says nothing of.
type counted struct {
	asked int
	codes map[string]string
	fault error
}

func (c *counted) Judge(_ context.Context, _ string, file []byte) (string, error) {
	c.asked++
	return c.codes[string(file)], c.fault
}

// judged is a world alone whose files `judge` judges.
func judged(t *testing.T, judge Judge) *httptest.Server {
	t.Helper()
	kept, err := folder.Open(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	server, err := New(Config{DevLevel: world.LevelBuilder, Version: "test"}, kept, nil, fields.Open(carried), judge)
	if err != nil {
		t.Fatal(err)
	}
	web := httptest.NewServer(server.Handler())
	t.Cleanup(web.Close)
	return web
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

const model = "VOX \x96\x00\x00\x00MAIN\x00\x00\x00\x00\x00\x00\x00\x00"

func TestAFileTheWorldHoldsIsNotJudgedAgain(t *testing.T) {
	judge := &counted{}
	web := judged(t, judge)
	if status, _ := post(t, web, "", []byte(model)); status != http.StatusCreated || judge.asked != 1 {
		t.Fatalf("a new file is judged, then kept: %d, judged %d", status, judge.asked)
	}
	if status, _ := post(t, web, "", []byte(model)); status != http.StatusOK || judge.asked != 1 {
		t.Fatalf("the same bytes are held, and not judged again: %d, judged %d", status, judge.asked)
	}
	if held := heldBy(t, web); len(held) != 1 {
		t.Fatalf("the world holds the file once: %v", held)
	}
}

func TestAFileTheJudgeRefusesIsNeverKept(t *testing.T) {
	web := judged(t, &counted{codes: map[string]string{model: "nope"}})
	if status, answer := post(t, web, "", []byte(model)); status != http.StatusUnprocessableEntity || answer["error"] != "nope" {
		t.Fatalf("refused with the judge's code: %d %v", status, answer)
	}
	if held := heldBy(t, web); len(held) != 0 {
		t.Fatalf("and the world holds no file: %v", held)
	}
}

func TestAFileTheJudgeCannotJudgeIsNeverKept(t *testing.T) {
	web := judged(t, &counted{fault: errors.New("no verdict")})
	if status, answer := post(t, web, "", []byte(model)); status != http.StatusInternalServerError || answer["error"] != "fault" {
		t.Fatalf("a judgement called off is the server's fault: %d %v", status, answer)
	}
	if held := heldBy(t, web); len(held) != 0 {
		t.Fatalf("and the world holds no file: %v", held)
	}
}
