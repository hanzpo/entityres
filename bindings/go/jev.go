package entityres

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math/rand/v2"
	"net/http"
	"os"
	"strings"
	"time"
)

type JevOptions struct {
	APIKey     string
	Model      string
	BaseURL    string
	Client     *http.Client
	MaxRetries int
}

type JevError struct {
	Status  int
	Message string
}

func (e *JevError) Error() string { return e.Message }

func Jev(opts JevOptions) (Judge, error) {
	if opts.APIKey == "" {
		opts.APIKey = os.Getenv("TYPESAFE_API_KEY")
	}
	if opts.APIKey == "" {
		return nil, errors.New("entityres: TYPESAFE_API_KEY is not set")
	}
	if opts.BaseURL == "" {
		opts.BaseURL = os.Getenv("TYPESAFE_BASE_URL")
	}
	if opts.BaseURL == "" {
		opts.BaseURL = "https://api.typesafe.ai"
	}
	if opts.Model == "" {
		opts.Model = "jev-latest"
	}
	if opts.Client == nil {
		opts.Client = &http.Client{Timeout: 15 * time.Second}
	}
	if opts.MaxRetries == 0 {
		opts.MaxRetries = 3
	}
	url := strings.TrimRight(opts.BaseURL, "/") + "/v1/systemone"

	return func(ctx context.Context, state, questions json.RawMessage) (json.RawMessage, error) {
		body, err := json.Marshal(map[string]any{"state": state, "model": opts.Model, "questions": questions})
		if err != nil {
			return nil, err
		}
		for attempt := 0; ; attempt++ {
			answers, retry, err := post(ctx, opts, url, body)
			if err == nil || !retry || attempt == opts.MaxRetries {
				return answers, err
			}
			wait := min(4*time.Second, 250*time.Millisecond<<attempt) + time.Duration(rand.IntN(100))*time.Millisecond
			select {
			case <-ctx.Done():
				return nil, ctx.Err()
			case <-time.After(wait):
			}
		}
	}, nil
}

func post(ctx context.Context, opts JevOptions, url string, body []byte) (json.RawMessage, bool, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(body))
	if err != nil {
		return nil, false, err
	}
	req.Header.Set("content-type", "application/json")
	req.Header.Set("authorization", "Bearer "+opts.APIKey)
	res, err := opts.Client.Do(req)
	if err != nil {
		return nil, ctx.Err() == nil, &JevError{Message: "jev request failed: " + err.Error()}
	}
	defer res.Body.Close()
	data, err := io.ReadAll(res.Body)
	if err != nil {
		return nil, true, err
	}
	if res.StatusCode != http.StatusOK {
		retry := res.StatusCode == http.StatusTooManyRequests || res.StatusCode >= 500
		return nil, retry, &JevError{Status: res.StatusCode, Message: fmt.Sprintf("jev %d: %.300s", res.StatusCode, data)}
	}
	var out struct {
		Answers json.RawMessage `json:"answers"`
	}
	if err := json.Unmarshal(data, &out); err != nil {
		return nil, false, err
	}
	return out.Answers, false, nil
}
