use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use reqwest::blocking::Client;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::ACCEPT;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;

use crate::paths;

const DEFAULT_MCP_URL: &str = "http://127.0.0.1:3500/mcp";
const MONARCH_APP_PREFIX: &str = "https://app.monarch.com/";
const RESULT_ROOT: &str = "__monGraphqlResults";

#[derive(Debug, Clone, Default)]
pub struct BrowserOptions {
    pub tab_id: Option<u64>,
    pub browser_id: Option<String>,
    pub mcp_url: Option<String>,
    pub settings_file: Option<PathBuf>,
}

pub struct BrowserMonarchClient {
    mcp: OpenBrowserMcp,
    tab_id: u64,
    browser_id: Option<String>,
}

struct OpenBrowserMcp {
    http: Client,
    endpoint: String,
    token: String,
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct OpenBrowserSettings {
    token: Option<String>,
}

#[derive(Debug)]
struct RpcResponse {
    session_id: Option<String>,
    body: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowserTab {
    id: u64,
    url: String,
    active: bool,
}

impl BrowserMonarchClient {
    pub fn connect(options: BrowserOptions) -> Result<Self> {
        let mcp = OpenBrowserMcp::connect(options.mcp_url, options.settings_file)?;

        let selection = match options.tab_id {
            Some(tab_id) => BrowserTabSelection {
                tab_id,
                browser_id: options.browser_id,
            },
            None => find_monarch_tab(&mcp, options.browser_id.clone())?,
        };

        Ok(Self {
            mcp,
            tab_id: selection.tab_id,
            browser_id: selection.browser_id,
        })
    }

    pub fn tab_id(&self) -> u64 {
        self.tab_id
    }

    pub fn browser_id(&self) -> Option<&str> {
        self.browser_id.as_deref()
    }

    pub fn graphql(&self, operation: &str, query: &str, variables: Value) -> Result<Value> {
        self.graphql_full_or_data(operation, query, variables, false)
    }

    pub fn graphql_full_or_data(
        &self,
        operation: &str,
        query: &str,
        variables: Value,
        full: bool,
    ) -> Result<Value> {
        let key = format!("mon-{}", request_nonce()?);
        let request = json!({
            "operationName": operation,
            "variables": variables,
            "query": query,
        });

        let key_literal = serde_json::to_string(&key).context("failed to encode browser key")?;
        let request_literal =
            serde_json::to_string(&request).context("failed to encode GraphQL request")?;
        let start_code = format!(
            r#"(function() {{
  const key = {key_literal};
  const request = {request_literal};
  window.{RESULT_ROOT} = window.{RESULT_ROOT} || Object.create(null);
  window.{RESULT_ROOT}[key] = {{ state: "running" }};
  const csrfMatch = document.cookie.match(/(?:^|;\s*)csrftoken=([^;]+)/);
  const headers = {{
    accept: "application/json",
    "content-type": "application/json",
    "client-platform": "web"
  }};
  const finish = (value) => {{
    const current = window.{RESULT_ROOT} && window.{RESULT_ROOT}[key];
    if (current && current.state === "cancelled") {{
      delete window.{RESULT_ROOT}[key];
      return;
    }}
    window.{RESULT_ROOT}[key] = value;
  }};
  if (csrfMatch) headers["x-csrftoken"] = decodeURIComponent(csrfMatch[1]);
  fetch("https://api.monarch.com/graphql", {{
    method: "POST",
    credentials: "include",
    headers,
    body: JSON.stringify(request)
  }})
    .then(async (response) => {{
      const text = await response.text();
      let body = null;
      try {{ body = JSON.parse(text); }} catch (_) {{}}
      finish({{
        state: "done",
        ok: response.ok,
        status: response.status,
        statusText: response.statusText,
        body,
        text: body ? undefined : text.slice(0, 4000)
      }});
    }})
    .catch((error) => {{
      finish({{
        state: "error",
        error: error && error.message ? error.message : String(error)
      }});
    }});
  return {{ state: "started", key }};
}})()"#
        );

        let started = self.evaluate_json(&start_code)?;
        if started["state"].as_str() != Some("started") {
            anyhow::bail!("failed to start browser GraphQL request for {operation}: {started}");
        }

        let value = self.poll_graphql_result(operation, &key, &key_literal);
        self.cleanup_result(&key_literal);

        let value = value?;
        if value["state"].as_str() == Some("error") {
            let error = value["error"].as_str().unwrap_or("unknown browser error");
            anyhow::bail!("browser GraphQL {operation} failed before response: {error}");
        }

        if value["state"].as_str() != Some("done") {
            anyhow::bail!("browser GraphQL {operation} ended in unexpected state: {value}");
        }

        if !value["ok"].as_bool().unwrap_or(false) {
            let status = value["status"].as_u64().unwrap_or(0);
            let status_text = value["statusText"].as_str().unwrap_or("");
            let detail = value["body"]["detail"]
                .as_str()
                .or_else(|| value["text"].as_str())
                .unwrap_or("no response detail");
            anyhow::bail!(
                "browser GraphQL {operation} failed with HTTP {status} {status_text}: {detail}. Refresh or log in to Monarch in the browser, then retry."
            );
        }

        let body = value
            .get("body")
            .cloned()
            .context("browser GraphQL response did not contain a JSON body")?;

        if body.get("errors").is_some() {
            anyhow::bail!(
                "browser GraphQL {operation} returned errors: {}",
                body["errors"]
            );
        }

        if full {
            Ok(body)
        } else {
            body.get("data")
                .cloned()
                .with_context(|| format!("browser GraphQL {operation} response missing data"))
        }
    }

    fn poll_graphql_result(&self, operation: &str, key: &str, key_literal: &str) -> Result<Value> {
        let poll_code = format!(
            r#"(window.{RESULT_ROOT} && window.{RESULT_ROOT}[{key_literal}]) || {{ state: "missing" }}"#
        );
        let deadline = Instant::now() + Duration::from_secs(30);

        loop {
            sleep(Duration::from_millis(250));
            let value = self.evaluate_json(&poll_code)?;
            match value["state"].as_str() {
                Some("done") | Some("error") => return Ok(value),
                Some("running") | Some("missing") => {
                    if Instant::now() >= deadline {
                        anyhow::bail!(
                            "timed out waiting for browser GraphQL {operation} result key {key}"
                        );
                    }
                }
                Some(other) => anyhow::bail!(
                    "browser GraphQL {operation} returned unexpected poll state {other}: {value}"
                ),
                None => anyhow::bail!(
                    "browser GraphQL {operation} returned poll result without state: {value}"
                ),
            }
        }
    }

    fn evaluate_json(&self, code: &str) -> Result<Value> {
        let mut args = json!({
            "tabId": self.tab_id,
            "code": code,
        });
        if let Some(browser_id) = &self.browser_id {
            args["browserId"] = json!(browser_id);
        }

        let text = self.mcp.call_tool_text("javascript_tool", args)?;
        serde_json::from_str(&text).with_context(|| {
            format!(
                "javascript_tool returned non-JSON text for tab {}",
                self.tab_id
            )
        })
    }

    fn cleanup_result(&self, key_literal: &str) {
        let code = format!(
            r#"(function() {{
  if (!window.{RESULT_ROOT} || !window.{RESULT_ROOT}[{key_literal}]) return true;
  if (window.{RESULT_ROOT}[{key_literal}].state === "running") {{
    window.{RESULT_ROOT}[{key_literal}] = {{ state: "cancelled" }};
  }} else {{
    delete window.{RESULT_ROOT}[{key_literal}];
  }}
  return true;
}})()"#
        );
        let _ = self.evaluate_json(&code);
    }
}

impl OpenBrowserMcp {
    fn connect(mcp_url: Option<String>, settings_file: Option<PathBuf>) -> Result<Self> {
        let endpoint = mcp_url
            .or_else(|| std::env::var("OPENBROWSERMCP_MCP_URL").ok())
            .unwrap_or_else(|| DEFAULT_MCP_URL.to_owned());
        let token = read_openbrowser_token(settings_file)?;
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to build OpenBrowserMCP HTTP client")?;

        let mut mcp = Self {
            http,
            endpoint,
            token,
            session_id: String::new(),
        };

        let init = mcp.rpc(
            None,
            1,
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "mon",
                    "version": env!("CARGO_PKG_VERSION"),
                },
            }),
        )?;
        mcp.session_id = init
            .session_id
            .context("OpenBrowserMCP did not return an MCP session id")?;
        Ok(mcp)
    }

    fn call_tool_text(&self, name: &str, arguments: Value) -> Result<String> {
        let body = self
            .rpc(
                Some(&self.session_id),
                2,
                "tools/call",
                json!({
                    "name": name,
                    "arguments": arguments,
                }),
            )?
            .body;

        if let Some(error) = body.get("error") {
            anyhow::bail!("OpenBrowserMCP {name} returned RPC error: {error}");
        }

        let result = body
            .get("result")
            .context("OpenBrowserMCP response missing result")?;
        let text = result
            .get("content")
            .and_then(Value::as_array)
            .and_then(|items| {
                items.iter().find_map(|item| {
                    if item["type"].as_str() == Some("text") {
                        item["text"].as_str().map(ToOwned::to_owned)
                    } else {
                        None
                    }
                })
            })
            .context("OpenBrowserMCP response missing text content")?;

        if result["isError"].as_bool().unwrap_or(false) {
            anyhow::bail!("OpenBrowserMCP {name} failed: {text}");
        }

        Ok(text)
    }

    fn rpc(
        &self,
        session_id: Option<&str>,
        id: u64,
        method: &str,
        params: Value,
    ) -> Result<RpcResponse> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/event-stream"),
        );
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.token))
                .context("invalid OpenBrowserMCP token header")?,
        );
        if let Some(session_id) = session_id {
            headers.insert(
                "mcp-session-id",
                HeaderValue::from_str(session_id).context("invalid MCP session id")?,
            );
        }

        let response = self
            .http
            .post(&self.endpoint)
            .headers(headers)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }))
            .send()
            .with_context(|| format!("failed to call OpenBrowserMCP {method}"))?;

        let session_id = response
            .headers()
            .get("mcp-session-id")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let status = response.status();
        let text = response
            .text()
            .with_context(|| format!("failed to read OpenBrowserMCP {method} response"))?;

        if !status.is_success() {
            anyhow::bail!("OpenBrowserMCP {method} failed with HTTP {status}: {text}");
        }

        Ok(RpcResponse {
            session_id,
            body: parse_sse_or_json(&text)
                .with_context(|| format!("failed to parse OpenBrowserMCP {method} response"))?,
        })
    }
}

struct BrowserTabSelection {
    tab_id: u64,
    browser_id: Option<String>,
}

fn find_monarch_tab(
    mcp: &OpenBrowserMcp,
    requested_browser_id: Option<String>,
) -> Result<BrowserTabSelection> {
    let mut args = json!({ "all": true });
    if let Some(browser_id) = &requested_browser_id {
        args["browserId"] = json!(browser_id);
    }

    let text = mcp.call_tool_text("tabs_context", args)?;
    let parsed_browser_id = parse_browser_id(&text);
    let tab = parse_tabs_context(&text).with_context(|| {
        "no Monarch browser tab found; open https://app.monarch.com/dashboard in Helium and retry with --browser"
    })?;

    Ok(BrowserTabSelection {
        tab_id: tab.id,
        browser_id: requested_browser_id.or(parsed_browser_id),
    })
}

fn read_openbrowser_token(settings_file: Option<PathBuf>) -> Result<String> {
    let path = match settings_file {
        Some(path) => paths::expand_tilde(path)?,
        None => {
            if let Some(path) = std::env::var_os("OPENBROWSERMCP_SETTINGS") {
                paths::expand_tilde(PathBuf::from(path))?
            } else {
                paths::home_dir()?.join("openbrowsermcp/settings.json")
            }
        }
    };

    let bytes =
        std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let settings: OpenBrowserSettings = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let token = settings
        .token
        .context("OpenBrowserMCP settings file does not contain a token")?;
    if token.trim().is_empty() {
        anyhow::bail!("OpenBrowserMCP token is empty");
    }
    Ok(token)
}

fn parse_browser_id(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("browserId: "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_tabs_context(text: &str) -> Option<BrowserTab> {
    let tabs = text.lines().filter_map(parse_tab_list_line);
    let mut first_monarch = None;

    for tab in tabs {
        if !is_monarch_url(&tab.url) {
            continue;
        }
        if tab.active {
            return Some(tab);
        }
        if first_monarch.is_none() {
            first_monarch = Some(tab);
        }
    }

    first_monarch
}

fn parse_tab_list_line(line: &str) -> Option<BrowserTab> {
    let trimmed = line.trim_start();
    let active = trimmed.starts_with('*');
    let candidate = trimmed.strip_prefix('*').unwrap_or(trimmed).trim_start();
    let rest = candidate.strip_prefix('[')?;
    let (id, rest) = rest.split_once(']')?;
    let id = id.parse::<u64>().ok()?;
    let url = rest.split_whitespace().next()?.to_owned();
    Some(BrowserTab { id, url, active })
}

fn is_monarch_url(url: &str) -> bool {
    url.starts_with(MONARCH_APP_PREFIX)
}

fn parse_sse_or_json(text: &str) -> Result<Value> {
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if data.trim() == "[DONE]" {
                continue;
            }
            return serde_json::from_str(data).context("failed to parse SSE data line");
        }
    }
    serde_json::from_str(text).context("failed to parse JSON body")
}

fn request_nonce() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_browser_id() {
        assert_eq!(
            parse_browser_id("browserId: abc\nTabs (1):"),
            Some("abc".to_owned())
        );
    }

    #[test]
    fn parses_active_monarch_tab_from_context() {
        let text = r#"browserId: b1
Active tab: https://app.monarch.com/dashboard (ID: 42)
Tabs (2):
  * [42] https://app.monarch.com/dashboard
    [99] https://example.com
"#;
        assert_eq!(
            parse_tabs_context(text),
            Some(BrowserTab {
                id: 42,
                url: "https://app.monarch.com/dashboard".to_owned(),
                active: true,
            })
        );
    }

    #[test]
    fn prefers_monarch_tab_when_active_tab_is_elsewhere() {
        let text = r#"browserId: b1
Tabs (2):
  * [42] https://example.com
    [99] https://app.monarch.com/transactions
"#;
        assert_eq!(
            parse_tabs_context(text),
            Some(BrowserTab {
                id: 99,
                url: "https://app.monarch.com/transactions".to_owned(),
                active: false,
            })
        );
    }

    #[test]
    fn parses_sse_response_data() {
        let parsed =
            parse_sse_or_json("event: message\ndata: {\"result\":{\"ok\":true}}\n\n").unwrap();
        assert_eq!(parsed["result"]["ok"].as_bool(), Some(true));
    }
}
