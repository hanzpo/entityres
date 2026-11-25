package entityres

import (
	"bytes"
	"context"
	_ "embed"
	"encoding/json"
	"errors"
	"fmt"
	"sync"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
)

//go:embed entityres.wasm
var wasmBytes []byte

type Error struct{ Message string }

func (e *Error) Error() string { return "entityres: " + e.Message }

type Engine struct {
	mu      sync.Mutex
	runtime wazero.Runtime
	mod     api.Module
	alloc   api.Function
	free    api.Function
	run     api.Function
}

func New(ctx context.Context) (*Engine, error) {
	rt := wazero.NewRuntime(ctx)
	mod, err := rt.Instantiate(ctx, wasmBytes)
	if err != nil {
		_ = rt.Close(ctx)
		return nil, fmt.Errorf("entityres: load wasm: %w", err)
	}
	return &Engine{
		runtime: rt,
		mod:     mod,
		alloc:   mod.ExportedFunction("er_alloc"),
		free:    mod.ExportedFunction("er_free"),
		run:     mod.ExportedFunction("er_run"),
	}, nil
}

func (e *Engine) Close(ctx context.Context) error { return e.runtime.Close(ctx) }

func (e *Engine) RunJSON(ctx context.Context, input []byte) ([]byte, error) {
	e.mu.Lock()
	defer e.mu.Unlock()
	res, err := e.alloc.Call(ctx, uint64(len(input)))
	if err != nil {
		return nil, err
	}
	ptr := uint32(res[0])
	if !e.mod.Memory().Write(ptr, input) {
		return nil, errors.New("entityres: input write out of range")
	}
	res, err = e.run.Call(ctx, uint64(ptr), uint64(len(input)))
	if err != nil {
		return nil, err
	}
	if _, err := e.free.Call(ctx, uint64(ptr), uint64(len(input))); err != nil {
		return nil, err
	}
	outPtr, outLen := uint32(res[0]>>32), uint32(res[0])
	buf, ok := e.mod.Memory().Read(outPtr, outLen)
	if !ok {
		return nil, errors.New("entityres: output read out of range")
	}
	out := bytes.Clone(buf)
	if _, err := e.free.Call(ctx, uint64(outPtr), uint64(outLen)); err != nil {
		return nil, err
	}
	return out, nil
}

func (e *Engine) Run(ctx context.Context, input Input) (*Output, error) {
	in, err := json.Marshal(input)
	if err != nil {
		return nil, err
	}
	raw, err := e.RunJSON(ctx, in)
	if err != nil {
		return nil, err
	}
	var failed struct {
		Error *string `json:"error"`
	}
	if err := json.Unmarshal(raw, &failed); err != nil {
		return nil, err
	}
	if failed.Error != nil {
		return nil, &Error{Message: *failed.Error}
	}
	var out Output
	if err := json.Unmarshal(raw, &out); err != nil {
		return nil, err
	}
	return &out, nil
}

type Judge func(ctx context.Context, state, questions json.RawMessage) (json.RawMessage, error)

type ResolveOptions struct {
	MaxRounds   int
	Concurrency int
}

type Resolution struct {
	Output  *Output
	Answers []Answer
}

func (e *Engine) Resolve(ctx context.Context, input Input, judge Judge, opts ResolveOptions) (*Resolution, error) {
	if opts.MaxRounds == 0 {
		opts.MaxRounds = 10
	}
	if opts.Concurrency == 0 {
		opts.Concurrency = 8
	}
	answers := append([]Answer(nil), input.Answers...)
	for round := 0; ; round++ {
		input.Answers = answers
		out, err := e.Run(ctx, input)
		if err != nil {
			return nil, err
		}
		if len(out.Pending) == 0 || round == opts.MaxRounds {
			return &Resolution{Output: out, Answers: answers}, nil
		}
		fresh, err := ask(ctx, out.Pending, judge, opts.Concurrency)
		if err != nil {
			return nil, err
		}
		answers = append(answers, fresh...)
	}
}

func ask(ctx context.Context, pending []Question, judge Judge, concurrency int) ([]Answer, error) {
	out := make([]Answer, len(pending))
	errs := make([]error, len(pending))
	slots := make(chan struct{}, concurrency)
	var wg sync.WaitGroup
	for i, q := range pending {
		wg.Add(1)
		slots <- struct{}{}
		go func() {
			defer wg.Done()
			defer func() { <-slots }()
			a, err := judge(ctx, q.State, q.Questions)
			out[i], errs[i] = Answer{ID: q.ID, Answers: a}, err
		}()
	}
	wg.Wait()
	return out, errors.Join(errs...)
}
