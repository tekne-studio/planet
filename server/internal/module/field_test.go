package module_test

import (
	"encoding/json"
	"net/http"
	"os"
	"testing"

	pb "github.com/tekne-studio/planet/server/internal/protocol"
	"github.com/tekne-studio/planet/server/internal/protocol/cells"
	"github.com/tekne-studio/planet/server/internal/world"
)

// A volume over the Earth is seated the same on both sides of the seam: by
// the module in wazero here, from the field this server found, and by the
// client natively (crates/seat/tests/earth.rs). Both read the same world.
const earthWorld = "../../../crates/seat/tests/earth.json"

// stand is a place over the Earth and where a volume there starts and ends,
// or why none stands there.
type stand struct {
	Place   string
	Sector  uint32
	U, V    float64
	Low     int32
	Height  uint32
	Refused string
}

func TestAVolumeOverTheEarthStandsWhereTheClientSeatsIt(t *testing.T) {
	raw, err := os.ReadFile(earthWorld)
	if err != nil {
		t.Fatal(err)
	}
	var over struct {
		Seed             string
		GeneratorVersion int `json:"generator_version"`
		Stands           []stand
	}
	if err := json.Unmarshal(raw, &over); err != nil {
		t.Fatal(err)
	}
	sidecar, err := os.ReadFile("../../../assets/fields/earth.json")
	if err != nil {
		t.Fatal(err)
	}
	var field struct{ ID string }
	if err := json.Unmarshal(sidecar, &field); err != nil {
		t.Fatal(err)
	}
	web := instance(t, t.TempDir(), world.LevelAdmin)
	earth := map[string]any{"seed": over.Seed, "generator_version": over.GeneratorVersion, "params": map[string]any{"source": map[string]string{"field": field.ID}}}
	if status, why := call(t, "POST", web.URL+"/api/world", earth); status != http.StatusOK {
		t.Fatalf("a world is founded on the Earth: %d %v", status, why)
	}

	a, _ := dial(t, web)
	for i, place := range over.Stands {
		id := uint32(i + 1)
		// The asker stands by the volume, and so is told where it stands.
		a.say(&pb.ClientMessage{Message: &pb.ClientMessage_Stance{Stance: &pb.Stance{
			Sector: place.Sector, U: float32(place.U), V: float32(place.V), HeightM: float32(place.Low+8) / 2, FacingZ: -1,
		}}})
		a.asks(id, "open", &cells.Open{Seat: &cells.Seat{Seat: &cells.Seat_Sector{Sector: place.Sector}}, U: place.U, V: place.V})
		if place.Refused != "" {
			if code := a.answered(id); code != place.Refused {
				t.Errorf("%s: refused %q, the client says %q", place.Place, code, place.Refused)
			}
			continue
		}
		var opened cells.Opened
		a.told("opened", &opened)
		if code := a.answered(id); code != "" {
			t.Fatalf("%s: refused %q", place.Place, code)
		}
		if got := opened.Stood; got.Low != place.Low || got.Height != place.Height {
			t.Errorf("%s: stands from %d, %d high; the client seats it from %d, %d high", place.Place, got.Low, got.Height, place.Low, place.Height)
		}
	}
}
