use reqwest::{cookie::Jar, Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{sync::Arc, time::Duration};

#[derive(Debug, Serialize, Clone, Default)]
pub struct QuotaItem {
    pub id: String,
    pub label: String,
    pub used: Option<f64>,
    pub total: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub reset_at: Option<String>,
    pub unlimited: bool,
    pub is_credit_balance: bool,
    pub currency: Option<String>,
}
#[derive(Debug, Serialize, Clone, Default)]
pub struct ProviderAccount {
    pub connection_id: String,
    pub provider_id: String,
    pub provider_name: String,
    pub account_name: String,
    pub active: bool,
    pub plan: Option<String>,
    pub quotas: Vec<QuotaItem>,
    pub message: Option<String>,
    pub error: Option<String>,
}
#[derive(Debug, Serialize, Clone, Default)]
pub struct UsageSummary {
    pub requests: Option<u64>,
    pub tokens: Option<u64>,
    pub cost: Option<f64>,
}
#[derive(Debug, Serialize, Clone)]
pub struct RouterData {
    pub server_online: bool,
    pub authenticated: bool,
    pub fetched_at: String,
    pub accounts: Vec<ProviderAccount>,
    pub summary: Option<UsageSummary>,
    pub error: Option<String>,
}
impl Default for RouterData {
    fn default() -> Self {
        Self {
            server_online: false,
            authenticated: false,
            fetched_at: chrono::Utc::now().to_rfc3339(),
            accounts: vec![],
            summary: None,
            error: None,
        }
    }
}
#[derive(Debug, Deserialize)]
struct ConnectionsResponse {
    connections: Vec<Value>,
}

pub struct RouterClient {
    client: Client,
    base_url: String,
}
impl RouterClient {
    pub fn new(base_url: &str) -> Result<Self, String> {
        let mut base_url = base_url.trim().trim_end_matches('/').to_string();
        let parsed =
            reqwest::Url::parse(&base_url).map_err(|_| "URL 9Router tidak valid".to_string())?;
        if !matches!(parsed.scheme(), "https" | "http") {
            return Err("URL harus menggunakan HTTP atau HTTPS".into());
        }
        // The dashboard displays its OpenAI-compatible endpoint with `/v1`.
        // Accept that copy-pasted URL while keeping dashboard API calls at the origin.
        if parsed.path().trim_end_matches('/') == "/v1" {
            base_url.truncate(base_url.len() - 3);
        }
        let client = Client::builder()
            .cookie_provider(Arc::new(Jar::default()))
            .timeout(Duration::from_secs(25))
            .user_agent("9Router-Monitor/0.1")
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { client, base_url })
    }
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
    pub async fn login(&self, password: &str) -> Result<(), String> {
        let r = self
            .client
            .post(self.url("/api/auth/login"))
            .json(&serde_json::json!({"password": password}))
            .send()
            .await
            .map_err(network_error)?;
        if r.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err("Terlalu banyak percobaan login. Tunggu beberapa saat.".into());
        }
        if !r.status().is_success() {
            let v: Value = r.json().await.unwrap_or_default();
            return Err(v["error"]
                .as_str()
                .unwrap_or("Password salah atau login ditolak")
                .into());
        }
        Ok(())
    }
    pub async fn logout(&self) {
        let _ = self.client.post(self.url("/api/auth/logout")).send().await;
    }
    pub async fn auth_status(&self) -> Result<bool, String> {
        let r = self
            .client
            .get(self.url("/api/auth/status"))
            .send()
            .await
            .map_err(network_error)?;
        if !r.status().is_success() {
            return Err(format!("Auth status: HTTP {}", r.status()));
        }
        let v: Value = r.json().await.map_err(|e| e.to_string())?;
        Ok(v["authenticated"].as_bool().unwrap_or(false)
            || !v["requireLogin"].as_bool().unwrap_or(true))
    }
    pub async fn fetch(&self, force: bool) -> RouterData {
        let mut out = RouterData {
            server_online: true,
            authenticated: true,
            ..Default::default()
        };
        match self.auth_status().await {
            Ok(false) => {
                out.authenticated = false;
                out.error = Some("Login diperlukan".into());
                return out;
            }
            Err(e) => {
                // A network failure does not mean the saved session is invalid. Keep the
                // authenticated view so a temporarily unavailable instance does not send
                // the user back to the login screen.
                out.server_online = false;
                out.error = Some(e);
                return out;
            }
            _ => {}
        }
        let r = match self.client.get(self.url("/api/providers")).send().await {
            Ok(v) => v,
            Err(e) => {
                out.server_online = false;
                out.error = Some(network_error(e));
                return out;
            }
        };
        if r.status() == StatusCode::UNAUTHORIZED {
            out.authenticated = false;
            out.error = Some("Sesi telah berakhir".into());
            return out;
        }
        let connections: ConnectionsResponse = match r.json().await {
            Ok(v) => v,
            Err(e) => {
                out.error = Some(format!("Respons provider tidak valid: {e}"));
                return out;
            }
        };
        let mut jobs = vec![];
        for c in connections.connections {
            let client = self.client.clone();
            let base = self.base_url.clone();
            jobs.push(async move { fetch_account(client, base, c, force).await });
        }
        out.accounts = futures_util::future::join_all(jobs).await;
        out.summary = self.fetch_summary().await;
        out.fetched_at = chrono::Utc::now().to_rfc3339();
        out
    }
    async fn fetch_summary(&self) -> Option<UsageSummary> {
        let v: Value = self
            .client
            .get(self.url("/api/usage/stats?period=7d"))
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        Some(UsageSummary {
            requests: number_at(&v, &["totalRequests", "requests", "total_requests"])
                .map(|n| n as u64),
            tokens: number_at(&v, &["totalTokens", "tokens", "total_tokens"]).map(|n| n as u64),
            cost: number_at(&v, &["totalCost", "cost", "total_cost"]),
        })
    }
}
async fn fetch_account(client: Client, base: String, c: Value, force: bool) -> ProviderAccount {
    let id = string_field(&c, &["id"]);
    let provider = string_field(&c, &["provider"]);
    let name = string_field(&c, &["name", "displayName"]);
    let mut a = ProviderAccount {
        connection_id: id.clone(),
        provider_id: provider.clone(),
        provider_name: provider_label(&provider),
        account_name: if name.is_empty() {
            provider_label(&provider)
        } else {
            name
        },
        active: c["isActive"].as_bool().unwrap_or(true),
        ..Default::default()
    };
    let url = format!(
        "{base}/api/usage/{id}{}",
        if force { "?force=1" } else { "" }
    );
    match client.get(url).send().await {
        Ok(r) if r.status() == StatusCode::UNAUTHORIZED => {
            a.error = Some("Sesi telah berakhir".into())
        }
        Ok(r) if r.status().is_success() => match r.json::<Value>().await {
            Ok(v) => {
                a.plan = v["plan"].as_str().map(str::to_string);
                a.message = v["message"].as_str().map(str::to_string);
                a.error = v["error"].as_str().map(str::to_string);
                a.quotas = normalize_quotas(&v)
            }
            Err(e) => a.error = Some(format!("Payload quota tidak valid: {e}")),
        },
        Ok(r) => a.error = Some(format!("HTTP {}", r.status())),
        Err(e) => a.error = Some(network_error(e)),
    };
    a
}
fn normalize_quotas(raw: &Value) -> Vec<QuotaItem> {
    let mut out = vec![];
    if let Some(q) = raw["quotas"].as_object() {
        for (id, v) in q {
            if v.is_object() {
                out.push(quota(id, v));
            }
        }
    }
    if out.is_empty() {
        for key in ["five_hour", "seven_day", "primary", "secondary"] {
            if raw[key].is_object() {
                out.push(quota(key, &raw[key]));
            }
        }
    }
    out
}
fn quota(id: &str, v: &Value) -> QuotaItem {
    let used = number_at(v, &["used", "consumed", "utilization", "percentUsed"]);
    let total = number_at(v, &["total", "limit"]);
    let remaining = number_at(
        v,
        &[
            "remainingPercentage",
            "remaining_percent",
            "remainingPercent",
        ],
    )
    .or_else(|| match (used, total) {
        (Some(u), Some(t)) if t > 0.0 => Some(((t - u) / t * 100.0).clamp(0.0, 100.0)),
        _ => None,
    });
    QuotaItem {
        id: id.into(),
        label: v["displayName"]
            .as_str()
            .or_else(|| v["label"].as_str())
            .unwrap_or(&humanize(id))
            .into(),
        used,
        total,
        remaining_percent: remaining,
        reset_at: v["resetAt"]
            .as_str()
            .or_else(|| v["resets_at"].as_str())
            .map(str::to_string),
        unlimited: v["unlimited"].as_bool().unwrap_or(false),
        is_credit_balance: v["isCreditBalance"]
            .as_bool()
            .or_else(|| v["is_credit_balance"].as_bool())
            .unwrap_or(false),
        currency: v["currency"].as_str().map(str::to_string),
    }
}
fn string_field(v: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|k| v[*k].as_str())
        .unwrap_or_default()
        .into()
}
fn number_at(v: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| v[*k].as_f64())
}
fn humanize(s: &str) -> String {
    s.split(['_', '-'])
        .map(|p| {
            let mut c = p.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}
fn provider_label(s: &str) -> String {
    match s {
        "claude" => "Claude Code".into(),
        "codex" => "Codex".into(),
        "antigravity" => "Antigravity".into(),
        "kiro" => "Kiro".into(),
        "github" => "GitHub Copilot".into(),
        _ => humanize(s),
    }
}
fn network_error(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "Koneksi ke 9Router timeout".into()
    } else {
        format!("Tidak dapat terhubung: {e}")
    }
}
