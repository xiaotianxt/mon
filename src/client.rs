use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use reqwest::blocking::Client;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::ACCEPT;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::RETRY_AFTER;
use reqwest::header::USER_AGENT;
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

const BASE_URL: &str = "https://api.monarch.com";
const LOGIN_PATH: &str = "/auth/login/";
const GRAPHQL_PATH: &str = "/graphql";
const USER_AGENT_VALUE: &str = "mon/0.1.0 (https://github.com/xiaotianxt/mon)";

#[derive(Debug)]
pub enum LoginResult {
    Token(String),
    MfaRequired,
}

#[derive(Debug, Clone)]
pub struct MonarchClient {
    http: Client,
    token: Option<String>,
}

#[derive(Debug, Serialize)]
struct LoginRequest<'a> {
    username: &'a str,
    password: &'a str,
    supports_mfa: bool,
    trusted_device: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    totp: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    token: String,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    detail: Option<String>,
    error_code: Option<String>,
}

impl MonarchClient {
    pub fn new(token: Option<String>) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(Policy::none())
            .default_headers(base_headers(token.as_deref())?)
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self { http, token })
    }

    pub fn base_url() -> &'static str {
        BASE_URL
    }

    pub fn login(
        &self,
        email: &str,
        password: &str,
        mfa_code: Option<&str>,
    ) -> Result<LoginResult> {
        let body = LoginRequest {
            username: email,
            password,
            supports_mfa: true,
            trusted_device: false,
            totp: mfa_code,
        };

        let response = self
            .http
            .post(format!("{BASE_URL}{LOGIN_PATH}"))
            .json(&body)
            .send()
            .context("login request failed")?;

        let status = response.status();
        let retry_after = retry_after_header(&response);
        if status.is_redirection() {
            let location = redirect_location(&response);
            anyhow::bail!(
                "login endpoint redirected to {location}; update MonarchClient::BASE_URL"
            );
        }

        let text = response.text().context("failed to read login response")?;

        if status.as_u16() == 403 {
            return Ok(LoginResult::MfaRequired);
        }

        if !status.is_success() {
            anyhow::bail!(
                "{}",
                http_error_message("login", status, retry_after.as_deref(), &text)
            );
        }

        let parsed: LoginResponse =
            serde_json::from_str(&text).context("failed to parse login response")?;
        Ok(LoginResult::Token(parsed.token))
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
        if self.token.as_deref().unwrap_or("").is_empty() {
            anyhow::bail!("missing Monarch token");
        }

        let body = serde_json::json!({
            "operationName": operation,
            "variables": variables,
            "query": query,
        });

        let response = self
            .http
            .post(format!("{BASE_URL}{GRAPHQL_PATH}"))
            .json(&body)
            .send()
            .with_context(|| format!("GraphQL request failed for {operation}"))?;

        let status = response.status();
        let retry_after = retry_after_header(&response);
        if status.is_redirection() {
            let location = redirect_location(&response);
            anyhow::bail!(
                "GraphQL {operation} redirected to {location}; update MonarchClient::BASE_URL"
            );
        }

        let text = response
            .text()
            .with_context(|| format!("failed to read GraphQL response for {operation}"))?;
        if !status.is_success() {
            anyhow::bail!(
                "{}",
                http_error_message(
                    &format!("GraphQL {operation}"),
                    status,
                    retry_after.as_deref(),
                    &text,
                )
            );
        }

        let value: Value = serde_json::from_str(&text)
            .with_context(|| format!("failed to parse GraphQL response for {operation}"))?;

        if value.get("errors").is_some() {
            anyhow::bail!("GraphQL {operation} returned errors: {}", value["errors"]);
        }

        if full {
            Ok(value)
        } else {
            value
                .get("data")
                .cloned()
                .with_context(|| format!("GraphQL {operation} response missing data"))
        }
    }
}

fn base_headers(token: Option<&str>) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert("Client-Platform", HeaderValue::from_static("web"));
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));

    if let Some(token) = token {
        let value = format!("Token {token}");
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&value).context("invalid token header")?,
        );
    }

    Ok(headers)
}

fn redirect_location(response: &reqwest::blocking::Response) -> String {
    response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<missing Location header>")
        .to_owned()
}

fn retry_after_header(response: &reqwest::blocking::Response) -> Option<String> {
    response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn http_error_message(
    context: &str,
    status: StatusCode,
    retry_after: Option<&str>,
    body: &str,
) -> String {
    let parsed = serde_json::from_str::<ApiErrorBody>(body).ok();
    let detail = parsed
        .as_ref()
        .and_then(|error| error.detail.as_deref())
        .unwrap_or(body);
    let code = parsed
        .as_ref()
        .and_then(|error| error.error_code.as_deref());
    let retry_hint = retry_after
        .map(|value| format!(" Retry-After: {value}."))
        .unwrap_or_default();

    if status.as_u16() == 429 {
        if code == Some("CAPTCHA_REQUIRED") {
            return format!(
                "{context} was rate limited by Monarch and now requires CAPTCHA. Stop automated login attempts; use a valid saved session, wait, or complete login in the browser. Detail: {detail}.{retry_hint}"
            );
        }
        return format!(
            "{context} was rate limited by Monarch (HTTP 429). Do not retry in a loop; wait before trying again. Detail: {detail}.{retry_hint}"
        );
    }

    if let Some(code) = code {
        format!("{context} failed with HTTP {status} ({code}): {detail}")
    } else {
        format!("{context} failed with HTTP {status}: {detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_captcha_rate_limit_as_stop_signal() {
        let message = http_error_message(
            "login",
            StatusCode::TOO_MANY_REQUESTS,
            None,
            r#"{"detail":"CAPTCHA is required to proceed.","error_code":"CAPTCHA_REQUIRED"}"#,
        );

        assert!(message.contains("requires CAPTCHA"));
        assert!(message.contains("Stop automated login attempts"));
    }

    #[test]
    fn formats_generic_rate_limit_with_retry_hint() {
        let message = http_error_message(
            "GraphQL GetAccounts",
            StatusCode::TOO_MANY_REQUESTS,
            Some("60"),
            r#"{"detail":"Request was throttled."}"#,
        );

        assert!(message.contains("rate limited"));
        assert!(message.contains("Retry-After: 60"));
    }
}
