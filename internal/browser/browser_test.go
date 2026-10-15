package browser

import (
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"

	"github.com/xiaotianxt/mon/internal/graphql"
	"github.com/xiaotianxt/mon/internal/jsonx"
)

type toolCall struct {
	Name      string         `json:"name"`
	Arguments map[string]any `json:"arguments"`
}

// fakeBro answers tools/call with handler and records every call.
type fakeBro struct {
	mu    sync.Mutex
	calls []toolCall
}

func (f *fakeBro) start(t *testing.T, handler func(toolCall) (text string, isError bool)) *httptest.Server {
	t.Helper()
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		raw, _ := io.ReadAll(r.Body)
		var req struct {
			Method string   `json:"method"`
			Params toolCall `json:"params"`
		}
		if err := json.Unmarshal(raw, &req); err != nil {
			t.Errorf("bad request: %v", err)
		}
		if req.Method == "initialize" {
			w.Header().Set("Mcp-Session-Id", "test-session")
			_, _ = io.WriteString(w, `{"result":{}}`)
			return
		}
		f.mu.Lock()
		f.calls = append(f.calls, req.Params)
		f.mu.Unlock()
		text, isError := handler(req.Params)
		w.Header().Set("Content-Type", "text/event-stream")
		body, _ := json.Marshal(map[string]any{"result": map[string]any{
			"content": []any{map[string]any{"type": "text", "text": text}},
			"isError": isError,
		}})
		_, _ = io.WriteString(w, "event: message\ndata: "+string(body)+"\n\n")
	}))
	t.Cleanup(srv.Close)
	return srv
}

func testMCP(endpoint string) *mcp {
	return &mcp{http: http.DefaultClient, endpoint: endpoint, token: "test-only", sessionID: "test-session"}
}

func testClient(endpoint string, owned bool) *Client {
	return &Client{mcp: testMCP(endpoint), tabID: 42, browserID: "test-browser", ownsTab: owned}
}

func TestClosesOwnedTabAfterSuccessAndOperationError(t *testing.T) {
	for _, failOperation := range []bool{false, true} {
		var bro fakeBro
		srv := bro.start(t, func(toolCall) (string, bool) { return "Closed tab", false })
		opErr := errors.New("original operation error")

		err := testClient(srv.URL, true).Run(func(*Client) error {
			if failOperation {
				return opErr
			}
			return nil
		})

		want := error(nil)
		if failOperation {
			want = opErr
		}
		if err != want {
			t.Fatalf("Run returned %v, want %v", err, want)
		}
		if len(bro.calls) != 1 || bro.calls[0].Name != "tabs_close" {
			t.Fatalf("calls = %+v", bro.calls)
		}
		if got := jsonx.Compact(bro.calls[0].Arguments); got != `{"browserId":"test-browser","tabId":42}` {
			t.Fatalf("close args = %s", got)
		}
	}
}

func TestBorrowedTabsAreNotClosed(t *testing.T) {
	var bro fakeBro
	srv := bro.start(t, func(toolCall) (string, bool) { return "", false })
	if err := testClient(srv.URL, false).Run(func(*Client) error { return nil }); err != nil {
		t.Fatal(err)
	}
	if len(bro.calls) != 0 {
		t.Fatalf("unexpected calls %+v", bro.calls)
	}
}

func TestCleanupFailureDoesNotChangeSuccessfulOperation(t *testing.T) {
	srv := httptest.NewServer(http.NotFoundHandler())
	srv.Close()
	if err := testClient(srv.URL, true).Run(func(*Client) error { return nil }); err != nil {
		t.Fatalf("cleanup failure leaked into result: %v", err)
	}
}

func TestGraphQLUsesOneAwaitedEvaluation(t *testing.T) {
	var bro fakeBro
	srv := bro.start(t, func(call toolCall) (string, bool) {
		return `{"state":"done","ok":true,"status":200,"statusText":"OK","body":{"data":{"n":12345678901234567890}}}`, false
	})

	data, err := testClient(srv.URL, false).GraphQL("Example", "query Example { n }", map[string]any{})
	if err != nil {
		t.Fatal(err)
	}
	if got := jsonx.Compact(data); got != `{"n":12345678901234567890}` {
		t.Fatalf("data = %s (number precision must survive)", got)
	}
	if len(bro.calls) != 1 {
		t.Fatalf("expected exactly one bro call, got %d", len(bro.calls))
	}
	args := bro.calls[0].Arguments
	code, _ := args["code"].(string)
	if bro.calls[0].Name != "javascript_tool" || args["awaitPromise"] != true || args["tabId"] != float64(42) {
		t.Fatalf("call = %+v", bro.calls[0])
	}
	if !strings.Contains(code, `"operationName":"Example"`) || strings.Contains(code, "__REQUEST__") {
		t.Fatalf("request not embedded in script: %s", code)
	}
}

func TestGraphQLReturnsTypedResponseErrors(t *testing.T) {
	var bro fakeBro
	srv := bro.start(t, func(toolCall) (string, bool) {
		return `{"state":"done","ok":false,"status":400,"statusText":"Bad Request","body":{"errors":[{"message":"Something went wrong"}]}}`, false
	})
	_, err := testClient(srv.URL, false).GraphQL("Example", "query Example { n }", map[string]any{})
	var responseErr *graphql.ResponseError
	if !errors.As(err, &responseErr) || responseErr.Class() != graphql.SchemaIncompatible {
		t.Fatalf("err = %v", err)
	}
}

func TestGraphQLReportsHTTPAndBrowserFailures(t *testing.T) {
	for _, tc := range []struct{ reply, want string }{
		{`{"state":"done","ok":false,"status":401,"statusText":"Unauthorized","body":{"detail":"Not logged in"}}`, "HTTP 401 Unauthorized: Not logged in"},
		{`{"state":"error","error":"Failed to fetch"}`, "failed before response: Failed to fetch"},
		{`{"state":"weird"}`, "unexpected state"},
	} {
		var bro fakeBro
		srv := bro.start(t, func(toolCall) (string, bool) { return tc.reply, false })
		_, err := testClient(srv.URL, false).GraphQL("Example", "query Example { n }", map[string]any{})
		if err == nil || !strings.Contains(err.Error(), tc.want) {
			t.Fatalf("err = %v, want %q", err, tc.want)
		}
	}
}

func TestToolErrorsSurface(t *testing.T) {
	var bro fakeBro
	srv := bro.start(t, func(toolCall) (string, bool) { return "tool call timed out", true })
	_, err := testClient(srv.URL, false).GraphQL("Example", "query Example { n }", map[string]any{})
	if err == nil || !strings.Contains(err.Error(), "bro javascript_tool failed: tool call timed out") {
		t.Fatalf("err = %v", err)
	}
}

func TestOpensWaitsForAndOwnsTabWhenNoMonarchTabExists(t *testing.T) {
	var bro fakeBro
	probes := 0
	srv := bro.start(t, func(call toolCall) (string, bool) {
		switch call.Name {
		case "tabs_context":
			return "browserId: b1\nTabs (1):\n  * [7] https://example.com\n", false
		case "tabs_create":
			return "Created tab: 99 (window 1)", false
		case "javascript_tool":
			probes++
			if probes == 1 {
				return "Execution context was destroyed.", true
			}
			if probes == 2 {
				return `{"href":"about:blank","readyState":"complete"}`, false
			}
			return `{"href":"https://app.monarch.com/dashboard","readyState":"interactive"}`, false
		}
		return "", true
	})

	client, err := findMonarchTab(testMCP(srv.URL), "")
	if err != nil {
		t.Fatal(err)
	}
	if client.tabID != 99 || !client.ownsTab || client.browserID != "b1" || probes != 3 {
		t.Fatalf("client = %+v probes = %d", client, probes)
	}
	if active, _ := bro.calls[1].Arguments["active"].(bool); active || bro.calls[1].Arguments["browserId"] != "b1" {
		t.Fatalf("create args = %+v", bro.calls[1].Arguments)
	}
}

func TestReusesExistingMonarchTab(t *testing.T) {
	var bro fakeBro
	srv := bro.start(t, func(toolCall) (string, bool) {
		return "browserId: b1\nTabs (2):\n  * [42] https://example.com\n    [99] https://app.monarch.com/transactions\n", false
	})
	client, err := findMonarchTab(testMCP(srv.URL), "explicit")
	if err != nil {
		t.Fatal(err)
	}
	if client.tabID != 99 || client.ownsTab || client.browserID != "explicit" || len(bro.calls) != 1 {
		t.Fatalf("client = %+v calls = %+v", client, bro.calls)
	}
}

func TestParsesCreatedTabID(t *testing.T) {
	if id, ok := parseCreatedTabID("Created tab: 1068127084 (window 1068126889)"); !ok || id != 1068127084 {
		t.Fatalf("got %d %v", id, ok)
	}
	if _, ok := parseCreatedTabID("Failed to create"); ok {
		t.Fatal("expected failure")
	}
}

func TestParsesBrowserID(t *testing.T) {
	if got := parseBrowserID("browserId: abc\nTabs (1):"); got != "abc" {
		t.Fatalf("got %q", got)
	}
}

func TestParsesActiveMonarchTabFromContext(t *testing.T) {
	text := "browserId: b1\nActive tab: https://app.monarch.com/dashboard (ID: 42)\nTabs (2):\n  * [42] https://app.monarch.com/dashboard\n    [99] https://example.com\n"
	tab, ok := parseTabsContext(text)
	if !ok || tab != (browserTab{id: 42, url: "https://app.monarch.com/dashboard", active: true}) {
		t.Fatalf("got %+v %v", tab, ok)
	}
}

func TestPrefersMonarchTabWhenActiveTabIsElsewhere(t *testing.T) {
	text := "browserId: b1\nTabs (2):\n  * [42] https://example.com\n    [99] https://app.monarch.com/transactions\n"
	tab, ok := parseTabsContext(text)
	if !ok || tab != (browserTab{id: 99, url: "https://app.monarch.com/transactions"}) {
		t.Fatalf("got %+v %v", tab, ok)
	}
}

func TestParsesSSEResponseData(t *testing.T) {
	parsed, err := parseSSEOrJSON("event: message\ndata: {\"result\":{\"ok\":true}}\n\n")
	if err != nil || jsonx.Get(parsed, "result", "ok") != true {
		t.Fatalf("got %v %v", parsed, err)
	}
}

func TestSkipsSSEPrimingEvent(t *testing.T) {
	parsed, err := parseSSEOrJSON("data: \nid: 0\nretry: 3000\n\ndata: {\"result\":{\"ok\":true}}\n\n")
	if err != nil || jsonx.Get(parsed, "result", "ok") != true {
		t.Fatalf("got %v %v", parsed, err)
	}
}
