package browser

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"time"

	"github.com/xiaotianxt/mon/internal/jsonx"
	"github.com/xiaotianxt/mon/internal/paths"
)

const defaultMCPURL = "http://127.0.0.1:3500/mcp"

// httpTimeout exceeds bro's 30s extension tool-call timeout so bro reports its
// own timeout instead of the connection being cut mid-call.
const httpTimeout = 40 * time.Second

// mcp is a minimal streamable-HTTP MCP client for bro. It is safe for
// concurrent use once connected.
type mcp struct {
	http      *http.Client
	endpoint  string
	token     string
	sessionID string
	nextID    atomic.Uint64
}

func connectMCP(endpoint, settingsFile, clientVersion string) (*mcp, error) {
	if endpoint == "" {
		endpoint = envFirst("BRO_MCP_URL", "OPENBROWSERMCP_MCP_URL")
	}
	if endpoint == "" {
		endpoint = defaultMCPURL
	}
	token, err := readBroToken(settingsFile)
	if err != nil {
		return nil, err
	}

	m := &mcp{http: &http.Client{Timeout: httpTimeout}, endpoint: endpoint, token: token}
	sessionID, _, err := m.rpc("initialize", map[string]any{
		"protocolVersion": "2024-11-05",
		"capabilities":    map[string]any{},
		"clientInfo":      map[string]any{"name": "mon", "version": clientVersion},
	})
	if err != nil {
		return nil, err
	}
	if sessionID == "" {
		return nil, errors.New("bro did not return an MCP session id")
	}
	m.sessionID = sessionID
	return m, nil
}

// callToolText returns the first text content item of a successful tool call.
func (m *mcp) callToolText(name string, arguments map[string]any) (string, error) {
	_, body, err := m.rpc("tools/call", map[string]any{"name": name, "arguments": arguments})
	if err != nil {
		return "", err
	}

	if rpcErr, ok := jsonx.Lookup(body, "error"); ok {
		return "", fmt.Errorf("bro %s returned RPC error: %s", name, jsonx.Compact(rpcErr))
	}
	result, ok := jsonx.Lookup(body, "result")
	if !ok {
		return "", errors.New("bro response missing result")
	}

	text, found := "", false
	items, _ := jsonx.Get(result, "content").([]any)
	for _, item := range items {
		if kind, _ := jsonx.String(jsonx.Get(item, "type")); kind != "text" {
			continue
		}
		if text, found = jsonx.String(jsonx.Get(item, "text")); found {
			break
		}
	}
	if !found {
		return "", errors.New("bro response missing text content")
	}

	if isError, _ := jsonx.Bool(jsonx.Get(result, "isError")); isError {
		return "", fmt.Errorf("bro %s failed: %s", name, text)
	}
	return text, nil
}

func (m *mcp) rpc(method string, params map[string]any) (sessionID string, body any, err error) {
	payload, err := json.Marshal(map[string]any{
		"jsonrpc": "2.0",
		"id":      m.nextID.Add(1),
		"method":  method,
		"params":  params,
	})
	if err != nil {
		return "", nil, fmt.Errorf("failed to encode bro %s request: %w", method, err)
	}

	req, err := http.NewRequest(http.MethodPost, m.endpoint, bytes.NewReader(payload))
	if err != nil {
		return "", nil, fmt.Errorf("failed to build bro %s request: %w", method, err)
	}
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Accept", "application/json, text/event-stream")
	req.Header.Set("Authorization", "Bearer "+m.token)
	if m.sessionID != "" {
		req.Header.Set("Mcp-Session-Id", m.sessionID)
	}

	resp, err := m.http.Do(req)
	if err != nil {
		return "", nil, fmt.Errorf("failed to call bro %s: %w", method, err)
	}
	defer resp.Body.Close()

	text, err := io.ReadAll(resp.Body)
	if err != nil {
		return "", nil, fmt.Errorf("failed to read bro %s response: %w", method, err)
	}
	if resp.StatusCode < 200 || resp.StatusCode > 299 {
		return "", nil, fmt.Errorf("bro %s failed with HTTP %s: %s", method, resp.Status, text)
	}

	body, err = parseSSEOrJSON(string(text))
	if err != nil {
		return "", nil, fmt.Errorf("failed to parse bro %s response: %w", method, err)
	}
	return resp.Header.Get("Mcp-Session-Id"), body, nil
}

// parseSSEOrJSON returns the first non-empty SSE data event, or the whole body as JSON.
func parseSSEOrJSON(text string) (any, error) {
	for line := range strings.Lines(text) {
		data, ok := strings.CutPrefix(strings.TrimRight(line, "\r\n"), "data: ")
		if !ok {
			continue
		}
		data = strings.TrimSpace(data)
		if data == "" || data == "[DONE]" {
			continue
		}
		v, err := jsonx.Decode([]byte(data))
		if err != nil {
			return nil, fmt.Errorf("failed to parse SSE data line: %w", err)
		}
		return v, nil
	}
	v, err := jsonx.Decode([]byte(text))
	if err != nil {
		return nil, fmt.Errorf("failed to parse JSON body: %w", err)
	}
	return v, nil
}

func readBroToken(settingsFile string) (string, error) {
	path := settingsFile
	if path == "" {
		path = envFirst("BRO_SETTINGS", "OPENBROWSERMCP_SETTINGS")
	}
	if path == "" {
		home, err := paths.HomeDir()
		if err != nil {
			return "", err
		}
		path = filepath.Join(home, ".bro", "settings.json")
	}
	path, err := paths.ExpandTilde(path)
	if err != nil {
		return "", err
	}

	raw, err := os.ReadFile(path)
	if err != nil {
		return "", fmt.Errorf("failed to read %s: %w", path, err)
	}
	var settings struct {
		Token *string `json:"token"`
	}
	if err := json.Unmarshal(raw, &settings); err != nil {
		return "", fmt.Errorf("failed to parse %s: %w", path, err)
	}
	if settings.Token == nil {
		return "", errors.New("bro settings file does not contain a token")
	}
	if strings.TrimSpace(*settings.Token) == "" {
		return "", errors.New("bro token is empty")
	}
	return *settings.Token, nil
}

func envFirst(names ...string) string {
	for _, name := range names {
		if value := os.Getenv(name); value != "" {
			return value
		}
	}
	return ""
}
