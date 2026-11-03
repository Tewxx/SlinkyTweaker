package api

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strconv"
	"time"
)

type Client struct {
	HttpClient *http.Client
	LicenseKey string
	BaseURL    string
}

type Error struct {
	StatusCode int    `json:"-"`
	Message    string `json:"error"`
}

func (e Error) Error() string {
	if e.Message != "" {
		return e.Message
	}
	return fmt.Sprintf("Code %d", e.StatusCode)
}

type response struct {
	Remaining int64 `json:"remaining"`
}

func (c *Client) GetLicenseRemainingDuration(ctx context.Context) (time.Duration, error) {
	var r response
	if err := c.getJSON(ctx, http.MethodGet, "/licenses/info", &r); err != nil {
		return 0, err
	}
	return time.Duration(r.Remaining) * time.Second, nil
}

func (c *Client) GetFile(ctx context.Context, name string, withAuthBlob bool) (io.ReadCloser, int64, error) {
	query := ""
	if withAuthBlob {
		query = "?inject_auth_blob_data=true"
	}
	path := fmt.Sprintf("/files/%s%s", name, query)
	size := int64(-1)
	body, err := c.sendRequest(ctx, http.MethodGet, path, func(resp *http.Response) {
		if v := resp.Header.Get("Content-Length"); v != "" {
			if n, err := strconv.ParseInt(v, 10, 64); err == nil && n > 0 {
				size = n
			}
		}
	})
	return body, size, err
}

func (c *Client) sendRequest(ctx context.Context, method, path string, onHeaders func(*http.Response)) (io.ReadCloser, error) {
	req, err := http.NewRequestWithContext(ctx, method, c.BaseURL+path, nil)
	if err != nil {
		return nil, fmt.Errorf("creating request: %w", err)
	}
	req.Header.Set("Authorization", "Bearer "+c.LicenseKey)
	resp, err := c.HttpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("sending request: %w", err)
	}
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		defer resp.Body.Close()
		return nil, decodeError(resp)
	}
	if onHeaders != nil {
		onHeaders(resp)
	}
	return resp.Body, nil
}

func (c *Client) getJSON(ctx context.Context, method, path string, out any) (err error) {
	body, err := c.sendRequest(ctx, method, path, nil)
	if err != nil {
		return err
	}
	defer func() { err = body.Close() }()
	return json.NewDecoder(body).Decode(out)
}

func decodeError(resp *http.Response) error {
	e := Error{StatusCode: resp.StatusCode}
	_ = json.NewDecoder(resp.Body).Decode(&e)
	return e
}
