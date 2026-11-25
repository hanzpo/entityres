package entityres

import (
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"sync/atomic"
	"testing"
)

type fakeSpec struct {
	Pairs  map[string]float64           `json:"pairs"`
	Values map[string]map[string]string `json:"values"`
}

type expectation struct {
	Entities []struct {
		Records []string       `json:"records"`
		Fields  map[string]any `json:"fields"`
	} `json:"entities"`
	MaxCalls *int `json:"max_calls"`
}

func readJSON(t *testing.T, path string, v any) {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(data, v); err != nil {
		t.Fatal(err)
	}
}

func fakeJudge(spec fakeSpec, calls *atomic.Int64) Judge {
	return func(_ context.Context, state, questions json.RawMessage) (json.RawMessage, error) {
		calls.Add(1)
		var s struct {
			ID    string              `json:"id"`
			Field string              `json:"field"`
			A     struct{ ID string } `json:"a"`
			B     struct{ ID string } `json:"b"`
		}
		var q struct {
			Same *json.RawMessage `json:"same"`
			Pick struct {
				Criteria map[string]string `json:"criteria"`
			} `json:"pick"`
		}
		if err := json.Unmarshal(state, &s); err != nil {
			return nil, err
		}
		if err := json.Unmarshal(questions, &q); err != nil {
			return nil, err
		}
		if q.Same != nil {
			return json.Marshal(map[string]any{"same": map[string]any{"type": "noul", "noul": spec.Pairs[s.A.ID+"|"+s.B.ID]}})
		}
		choice := "0"
		for k, v := range q.Pick.Criteria {
			if v == spec.Values[s.ID][s.Field] {
				choice = k
			}
		}
		return json.Marshal(map[string]any{"pick": map[string]any{"type": "choice", "choice": choice}})
	}
}

func TestFixtures(t *testing.T) {
	ctx := context.Background()
	engine, err := New(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer engine.Close(ctx)
	dirs, _ := filepath.Glob("../../fixtures/*/input.json")
	if len(dirs) == 0 {
		t.Fatal("no fixtures")
	}
	for _, path := range dirs {
		dir := filepath.Dir(path)
		t.Run(filepath.Base(dir), func(t *testing.T) {
			var input Input
			var spec fakeSpec
			var want expectation
			readJSON(t, path, &input)
			readJSON(t, filepath.Join(dir, "judge.json"), &spec)
			readJSON(t, filepath.Join(dir, "expected.json"), &want)
			var calls atomic.Int64
			res, err := engine.Resolve(ctx, input, fakeJudge(spec, &calls), ResolveOptions{})
			if err != nil {
				t.Fatal(err)
			}
			var got, exp []string
			byRecords := map[string]Entity{}
			for _, e := range res.Output.Entities {
				key := strings.Join(e.Records, ",")
				got = append(got, key)
				byRecords[key] = e
			}
			for _, e := range want.Entities {
				exp = append(exp, strings.Join(e.Records, ","))
			}
			slices.Sort(got)
			slices.Sort(exp)
			if !slices.Equal(got, exp) {
				t.Fatalf("clusters\n got %v\nwant %v", got, exp)
			}
			for _, e := range want.Entities {
				entity := byRecords[strings.Join(e.Records, ",")]
				for k, v := range e.Fields {
					g, _ := json.Marshal(entity.Fields[k])
					w, _ := json.Marshal(v)
					if string(g) != string(w) {
						t.Errorf("%s field %s: got %s want %s", entity.ID, k, g, w)
					}
				}
			}
			if want.MaxCalls != nil && calls.Load() > int64(*want.MaxCalls) {
				t.Errorf("%d judge calls, max %d", calls.Load(), *want.MaxCalls)
			}
			if len(res.Output.Conflicts) > 0 {
				t.Errorf("conflicts %v", res.Output.Conflicts)
			}
		})
	}
}

func TestBadInput(t *testing.T) {
	ctx := context.Background()
	engine, err := New(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer engine.Close(ctx)
	_, err = engine.Run(ctx, Input{Records: []Record{{ID: "1", Source: "a"}, {ID: "1", Source: "a"}}})
	var e *Error
	if !errors.As(err, &e) || e.Message != "duplicate record a:1" {
		t.Fatalf("got %v", err)
	}
}
