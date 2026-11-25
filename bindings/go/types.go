package entityres

import "encoding/json"

type FieldConfig struct {
	Compare    string    `json:"compare,omitempty"`
	Normalize  string    `json:"normalize,omitempty"`
	Semantic   bool      `json:"semantic,omitempty"`
	Thresholds []float64 `json:"thresholds,omitempty"`
	Merge      string    `json:"merge,omitempty"`
}

type BlockRule struct {
	Field     string `json:"field"`
	Transform string `json:"transform,omitempty"`
	N         int    `json:"n,omitempty"`
}

type Veto struct {
	Rule  string `json:"rule"`
	Field string `json:"field,omitempty"`
}

type MergeConfig struct {
	Default      string `json:"default,omitempty"`
	UpdatedField string `json:"updated_field,omitempty"`
}

type Config struct {
	Entity   string                 `json:"entity,omitempty"`
	Fields   map[string]FieldConfig `json:"fields,omitempty"`
	Keys     []string               `json:"keys,omitempty"`
	Blocking []BlockRule            `json:"blocking,omitempty"`
	Vetoes   []Veto                 `json:"vetoes,omitempty"`
	Sources  []string               `json:"sources,omitempty"`
	Merge    *MergeConfig           `json:"merge,omitempty"`
	Routing  map[string]float64     `json:"routing,omitempty"`
}

type Record struct {
	ID     string         `json:"id"`
	Source string         `json:"source"`
	Fields map[string]any `json:"fields,omitempty"`
}

type Answer struct {
	ID      string          `json:"id"`
	Answers json.RawMessage `json:"answers"`
}

type Input struct {
	Config  Config   `json:"config"`
	Records []Record `json:"records"`
	Answers []Answer `json:"answers,omitempty"`
}

type Pair struct {
	ID       string             `json:"id"`
	A        string             `json:"a"`
	B        string             `json:"b"`
	P        float64            `json:"p"`
	Judge    *float64           `json:"judge,omitempty"`
	Decision string             `json:"decision"`
	By       string             `json:"by"`
	Unsure   bool               `json:"unsure"`
	Flags    []string           `json:"flags"`
	Weights  map[string]float64 `json:"weights"`
}

type Entity struct {
	ID      string              `json:"id"`
	Records []string            `json:"records"`
	Fields  map[string]any      `json:"fields"`
	From    map[string][]string `json:"from"`
}

type Question struct {
	ID        string          `json:"id"`
	Kind      string          `json:"kind"`
	Priority  float64         `json:"priority"`
	State     json.RawMessage `json:"state"`
	Questions json.RawMessage `json:"questions"`
}

type Conflict struct {
	Pair      string `json:"pair"`
	BlockedBy string `json:"blocked_by"`
}

type Stats struct {
	Records    int      `json:"records"`
	Candidates int      `json:"candidates"`
	Judged     int      `json:"judged"`
	Unsure     int      `json:"unsure"`
	Trusted    []string `json:"trusted"`
}

type FieldModel struct {
	M []float64 `json:"m"`
	U []float64 `json:"u"`
}

type Model struct {
	Lambda float64               `json:"lambda"`
	Fields map[string]FieldModel `json:"fields"`
}

type Output struct {
	Entities  []Entity   `json:"entities"`
	Pairs     []Pair     `json:"pairs"`
	Pending   []Question `json:"pending"`
	Conflicts []Conflict `json:"conflicts"`
	Stats     Stats      `json:"stats"`
	Model     Model      `json:"model"`
}
