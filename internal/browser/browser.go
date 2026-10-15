// Package browser runs Monarch GraphQL inside a logged-in browser tab via bro.
package browser

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/xiaotianxt/mon/internal/graphql"
	"github.com/xiaotianxt/mon/internal/jsonx"
)

const monarchAppPrefix = "https://app.monarch.com/"

const (
	// fetchTimeout aborts the in-page fetch before bro's 30s tool-call
	// timeout, so a slow request surfaces as a browser error with context.
	fetchTimeout      = 25 * time.Second
	pageReadyTimeout  = 20 * time.Second
	pageReadyInterval = 100 * time.Millisecond
)

type Options struct {
	// TabID selects an explicit tab; nil discovers or opens a Monarch tab.
	TabID         *uint64
	BrowserID     string
	MCPURL        string
	SettingsFile  string
	ClientVersion string
}

type Client struct {
	mcp       *mcp
	tabID     uint64
	browserID string
	// ownsTab is true only for a tab this client opened; only owned tabs are closed.
	ownsTab bool
}

func Connect(opts Options) (*Client, error) {
	m, err := connectMCP(opts.MCPURL, opts.SettingsFile, opts.ClientVersion)
	if err != nil {
		return nil, err
	}
	if opts.TabID != nil {
		return &Client{mcp: m, tabID: *opts.TabID, browserID: opts.BrowserID}, nil
	}
	return findMonarchTab(m, opts.BrowserID)
}

// Run executes op and then closes any tab this client opened. Cleanup failure
// is only a warning so it never changes the outcome of a mutation.
func (c *Client) Run(op func(*Client) error) error {
	err := op(c)
	if c.ownsTab {
		if closeErr := c.closeTab(); closeErr != nil {
			fmt.Fprintf(os.Stderr, "mon: warning: could not close owned tab %d: %v\n", c.tabID, closeErr)
		}
	}
	return err
}

func (c *Client) TabID() uint64 { return c.tabID }

// BrowserID is empty when bro did not report one.
func (c *Client) BrowserID() string { return c.browserID }

func (c *Client) GraphQL(operation, query string, variables any) (any, error) {
	return c.GraphQLFullOrData(operation, query, variables, false)
}

// fetchScript runs one GraphQL request in the page and resolves to its outcome.
// bro awaits the promise, so a request costs a single MCP round trip.
const fetchScript = `(async () => {
  const request = __REQUEST__;
  const headers = {
    accept: "application/json",
    "content-type": "application/json",
    "client-platform": "web"
  };
  const csrfMatch = document.cookie.match(/(?:^|;\s*)csrftoken=([^;]+)/);
  if (csrfMatch) headers["x-csrftoken"] = decodeURIComponent(csrfMatch[1]);
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), __TIMEOUT_MS__);
  try {
    const response = await fetch("https://api.monarch.com/graphql", {
      method: "POST",
      credentials: "include",
      headers,
      body: JSON.stringify(request),
      signal: controller.signal
    });
    const text = await response.text();
    let body = null;
    try { body = JSON.parse(text); } catch (_) {}
    return {
      state: "done",
      ok: response.ok,
      status: response.status,
      statusText: response.statusText,
      body,
      text: body ? undefined : text.slice(0, 4000)
    };
  } catch (error) {
    return { state: "error", error: error && error.message ? error.message : String(error) };
  } finally {
    clearTimeout(timer);
  }
})()`

// GraphQLFullOrData returns the whole response body when full is set, and its
// data member otherwise. Top-level GraphQL errors are returned as *graphql.ResponseError.
func (c *Client) GraphQLFullOrData(operation, query string, variables any, full bool) (any, error) {
	request, err := json.Marshal(map[string]any{
		"operationName": operation,
		"variables":     variables,
		"query":         query,
	})
	if err != nil {
		return nil, fmt.Errorf("failed to encode GraphQL request: %w", err)
	}
	code := strings.NewReplacer(
		"__REQUEST__", string(request),
		"__TIMEOUT_MS__", strconv.FormatInt(fetchTimeout.Milliseconds(), 10),
	).Replace(fetchScript)

	value, err := c.evaluateJSON(code, true)
	if err != nil {
		return nil, fmt.Errorf("browser GraphQL %s failed: %w", operation, err)
	}

	state, _ := jsonx.String(jsonx.Get(value, "state"))
	switch state {
	case "done":
	case "error":
		message, ok := jsonx.String(jsonx.Get(value, "error"))
		if !ok {
			message = "unknown browser error"
		}
		return nil, fmt.Errorf("browser GraphQL %s failed before response: %s", operation, message)
	default:
		return nil, fmt.Errorf("browser GraphQL %s ended in unexpected state: %s", operation, jsonx.Compact(value))
	}

	status := 0
	if s, ok := jsonx.Uint(jsonx.Get(value, "status")); ok && s <= 0xFFFF {
		status = int(s)
	}
	body, hasBody := jsonx.Lookup(value, "body")

	if hasBody {
		responseErr, err := graphql.ParseResponseError(operation, status, body)
		if err != nil {
			return nil, err
		}
		if responseErr != nil {
			return nil, responseErr
		}
	}

	if ok, _ := jsonx.Bool(jsonx.Get(value, "ok")); !ok {
		statusText, _ := jsonx.String(jsonx.Get(value, "statusText"))
		detail, found := jsonx.String(jsonx.Get(value, "body", "detail"))
		if !found {
			detail, found = jsonx.String(jsonx.Get(value, "text"))
		}
		if !found {
			detail = "no response detail"
		}
		return nil, fmt.Errorf("browser GraphQL %s failed with HTTP %d %s: %s. Refresh or log in to Monarch in the browser, then retry.",
			operation, status, statusText, detail)
	}

	if !hasBody {
		return nil, errors.New("browser GraphQL response did not contain a JSON body")
	}
	if full {
		return body, nil
	}
	data, ok := jsonx.Lookup(body, "data")
	if !ok {
		return nil, fmt.Errorf("browser GraphQL %s response missing data", operation)
	}
	return data, nil
}

func (c *Client) evaluateJSON(code string, awaitPromise bool) (any, error) {
	args := c.tabArgs()
	args["code"] = code
	if awaitPromise {
		args["awaitPromise"] = true
	}

	text, err := c.mcp.callToolText("javascript_tool", args)
	if err != nil {
		return nil, err
	}
	value, err := jsonx.Decode([]byte(text))
	if err != nil {
		return nil, fmt.Errorf("javascript_tool returned non-JSON text for tab %d: %w", c.tabID, err)
	}
	return value, nil
}

// waitForMonarchPage polls until a freshly opened tab has committed a Monarch
// document; evaluating earlier would run on about:blank or a page about to be
// replaced. Evaluation errors while the tab navigates are expected and retried.
func (c *Client) waitForMonarchPage() error {
	const probe = `({ href: location.href, readyState: document.readyState })`
	deadline := time.Now().Add(pageReadyTimeout)
	var lastErr error
	for {
		value, err := c.evaluateJSON(probe, false)
		if err == nil {
			href, _ := jsonx.String(jsonx.Get(value, "href"))
			readyState, _ := jsonx.String(jsonx.Get(value, "readyState"))
			if isMonarchURL(href) && readyState != "loading" {
				return nil
			}
			lastErr = fmt.Errorf("tab is at %q (%s)", href, readyState)
		} else {
			lastErr = err
		}
		if time.Now().After(deadline) {
			return fmt.Errorf("timed out waiting for Monarch tab %d to load: %w", c.tabID, lastErr)
		}
		time.Sleep(pageReadyInterval)
	}
}

func (c *Client) closeTab() error {
	_, err := c.mcp.callToolText("tabs_close", c.tabArgs())
	return err
}

func (c *Client) tabArgs() map[string]any {
	args := map[string]any{"tabId": c.tabID}
	if c.browserID != "" {
		args["browserId"] = c.browserID
	}
	return args
}

// findMonarchTab prefers an existing Monarch tab and otherwise opens an owned
// background tab that Run closes when the command finishes.
func findMonarchTab(m *mcp, requestedBrowserID string) (*Client, error) {
	contextArgs := map[string]any{"all": true}
	if requestedBrowserID != "" {
		contextArgs["browserId"] = requestedBrowserID
	}
	text, err := m.callToolText("tabs_context", contextArgs)
	if err != nil {
		return nil, err
	}

	browserID := requestedBrowserID
	if browserID == "" {
		browserID = parseBrowserID(text)
	}
	if tab, ok := parseTabsContext(text); ok {
		return &Client{mcp: m, tabID: tab.id, browserID: browserID}, nil
	}

	createArgs := map[string]any{"url": monarchAppPrefix + "dashboard", "active": false}
	if browserID != "" {
		createArgs["browserId"] = browserID
	}
	const hint = "open https://app.monarch.com/dashboard in your browser (Helium or Chrome) and retry"
	created, err := m.callToolText("tabs_create", createArgs)
	if err != nil {
		return nil, fmt.Errorf("no Monarch browser tab found and opening one failed (%w); %s", err, hint)
	}
	tabID, ok := parseCreatedTabID(created)
	if !ok {
		return nil, fmt.Errorf("no Monarch browser tab found and bro returned an unrecognized tabs_create result %q; %s", created, hint)
	}

	client := &Client{mcp: m, tabID: tabID, browserID: browserID, ownsTab: true}
	if err := client.waitForMonarchPage(); err != nil {
		if closeErr := client.closeTab(); closeErr != nil {
			return nil, fmt.Errorf("%w; closing the opened tab also failed: %v", err, closeErr)
		}
		return nil, err
	}
	return client, nil
}

type browserTab struct {
	id     uint64
	url    string
	active bool
}

func parseCreatedTabID(text string) (uint64, bool) {
	rest, ok := strings.CutPrefix(strings.TrimSpace(text), "Created tab: ")
	if !ok {
		return 0, false
	}
	fields := strings.Fields(rest)
	if len(fields) == 0 {
		return 0, false
	}
	id, err := strconv.ParseUint(fields[0], 10, 64)
	return id, err == nil
}

func parseBrowserID(text string) string {
	for line := range strings.Lines(text) {
		if value, ok := strings.CutPrefix(strings.TrimSpace(line), "browserId: "); ok {
			if value = strings.TrimSpace(value); value != "" {
				return value
			}
		}
	}
	return ""
}

// parseTabsContext returns the active Monarch tab, else the first Monarch tab.
func parseTabsContext(text string) (browserTab, bool) {
	var first browserTab
	found := false
	for line := range strings.Lines(text) {
		tab, ok := parseTabListLine(line)
		if !ok || !isMonarchURL(tab.url) {
			continue
		}
		if tab.active {
			return tab, true
		}
		if !found {
			first, found = tab, true
		}
	}
	return first, found
}

func parseTabListLine(line string) (browserTab, bool) {
	trimmed := strings.TrimLeft(line, " \t")
	candidate, active := strings.CutPrefix(trimmed, "*")
	rest, ok := strings.CutPrefix(strings.TrimLeft(candidate, " \t"), "[")
	if !ok {
		return browserTab{}, false
	}
	idText, rest, ok := strings.Cut(rest, "]")
	if !ok {
		return browserTab{}, false
	}
	id, err := strconv.ParseUint(idText, 10, 64)
	if err != nil {
		return browserTab{}, false
	}
	fields := strings.Fields(rest)
	if len(fields) == 0 {
		return browserTab{}, false
	}
	return browserTab{id: id, url: fields[0], active: active}, true
}

func isMonarchURL(url string) bool {
	return strings.HasPrefix(url, monarchAppPrefix)
}
