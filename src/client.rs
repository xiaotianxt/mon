use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use reqwest::blocking::Client;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderValue;
use reqwest::header::ACCEPT;
use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::USER_AGENT;
use reqwest::redirect::Policy;
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

        if response.status().as_u16() == 403 {
            return Ok(LoginResult::MfaRequired);
        }

        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("<missing Location header>")
                .to_owned();
            anyhow::bail!(
                "login endpoint redirected to {location}; update MonarchClient::BASE_URL"
            );
        }

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            anyhow::bail!("login failed with HTTP {status}: {text}");
        }

        let parsed: LoginResponse = response.json().context("failed to parse login response")?;
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
        if status.is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("<missing Location header>")
                .to_owned();
            anyhow::bail!(
                "GraphQL {operation} redirected to {location}; update MonarchClient::BASE_URL"
            );
        }

        let value: Value = response
            .json()
            .with_context(|| format!("failed to parse GraphQL response for {operation}"))?;

        if !status.is_success() {
            anyhow::bail!("GraphQL {operation} failed with HTTP {status}: {value}");
        }
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
