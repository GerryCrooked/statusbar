//! Microsoft 365 service health (`M365_TENANT_ID`, `M365_CLIENT_ID`, `M365_CLIENT_SECRET`).
//!
//! Polls Microsoft Graph (`/admin/serviceAnnouncement/issues`) with an app registration that has
//! the application permission `ServiceHealth.Read.All`, and feeds every open issue through the
//! integration named `M365_INTEGRATION` like a webhook event: rules, policy, delay and recipients
//! apply as for any monitoring system. An issue that is resolved – or no longer selected – gets
//! the all-clear. If Graph cannot be reached nothing is resolved. See integrations/microsoft-365.

use crate::hooks::{self, Event};
use crate::store::{self, now, Integration, Rule};
use crate::{admin, env_get, env_num, refresh, Api, App};
use axum::{extract::State, http::HeaderMap, Json};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{collections::HashSet, time::Duration};
use tracing::{info, warn};

/// Keys of Microsoft 365 issues start with this, followed by the issue ID (`EX123456`).
pub const KEY_PREFIX: &str = "m365-";

#[derive(Clone, Debug)]
pub struct M365Config {
    pub tenant: String,
    pub client_id: String,
    pub secret: String,
    pub integration: String,
    /// Lower-case service names to include; empty = all.
    pub services: Vec<String>,
    /// Lower-case classifications to include (`incident`, `advisory`).
    pub classifications: Vec<String>,
    pub poll_seconds: u64,
    pub graph_url: String,
    pub login_url: String,
}

#[derive(Default)]
pub struct M365State {
    pub last_ok: Option<i64>,
    pub last_error: Option<String>,
    /// Issues selected at the last successful poll.
    pub open: Vec<Value>,
}

fn list(raw: &str) -> Vec<String> {
    raw.split([',', ';']).map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty()).collect()
}

/// Reads the configuration; `None` if `M365_TENANT_ID` is not set.
pub fn load_config() -> Option<M365Config> {
    let tenant = env_get("M365_TENANT_ID", "").trim().to_string();
    if tenant.is_empty() {
        return None;
    }
    let mut secret = env_get("M365_CLIENT_SECRET", "");
    let file = env_get("M365_CLIENT_SECRET_FILE", "");
    if !file.is_empty() {
        secret = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("M365_CLIENT_SECRET_FILE {file}: {e}"));
    }
    let cfg = M365Config {
        tenant,
        client_id: env_get("M365_CLIENT_ID", "").trim().to_string(),
        secret: secret.trim().to_string(),
        integration: env_get("M365_INTEGRATION", "Microsoft 365").trim().to_string(),
        services: list(&env_get("M365_SERVICES", "")),
        classifications: list(&env_get("M365_CLASSIFICATIONS", "incident")),
        poll_seconds: env_num("M365_POLL_SECONDS", 120u64).max(60),
        graph_url: env_get("M365_GRAPH_URL", "https://graph.microsoft.com").trim_end_matches('/').to_string(),
        login_url: env_get("M365_LOGIN_URL", "https://login.microsoftonline.com").trim_end_matches('/').to_string(),
    };
    assert!(!cfg.client_id.is_empty(), "M365_TENANT_ID is set, but M365_CLIENT_ID is missing");
    assert!(!cfg.secret.is_empty(), "M365_TENANT_ID is set, but M365_CLIENT_SECRET(_FILE) is missing");
    Some(cfg)
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("").trim()
}

/// Is this issue one users should hear about, according to the configuration?
pub fn selected(cfg: &M365Config, issue: &Value) -> bool {
    if issue.get("isResolved").and_then(Value::as_bool).unwrap_or(false) {
        return false;
    }
    if s(issue, "status").eq_ignore_ascii_case("falsePositive") {
        return false;
    }
    let service = s(issue, "service").to_ascii_lowercase();
    let class = s(issue, "classification").to_ascii_lowercase();
    (cfg.services.is_empty() || cfg.services.contains(&service)) && cfg.classifications.contains(&class)
}

/// A Graph `serviceHealthIssue` as a problem event.
pub fn issue_event(issue: &Value) -> Option<Event> {
    let id = s(issue, "id");
    if id.is_empty() {
        return None;
    }
    let status = s(issue, "status").to_ascii_lowercase();
    let severity = if status == "serviceinterruption" {
        "error"
    } else if s(issue, "classification").eq_ignore_ascii_case("incident") {
        "warning"
    } else {
        "info"
    };
    let title = s(issue, "title");
    let service = s(issue, "service");
    Some(Event {
        key: format!("{KEY_PREFIX}{id}"),
        problem: true,
        title: if title.is_empty() { format!("{service} ({id})") } else { title.to_string() },
        message: s(issue, "impactDescription").to_string(),
        severity: severity.into(),
        host: "Microsoft 365".into(),
        service: service.to_string(),
    })
}

/// Applies one poll result: a problem for every selected issue, the all-clear for every open
/// Microsoft 365 message of the integration that is no longer among them.
/// Returns (key, outcome) for everything that changed something.
pub fn sync(
    conn: &Connection,
    integration: &Integration,
    rules: &[Rule],
    cfg: &M365Config,
    issues: &[Value],
) -> store::DbResult<Vec<(String, &'static str)>> {
    let mut changed = Vec::new();
    let mut current = HashSet::new();
    for e in issues.iter().filter(|i| selected(cfg, i)).filter_map(issue_event) {
        current.insert(e.key.clone());
        let outcome = hooks::apply(conn, integration, rules, &e)?;
        if outcome != "duplicate" {
            changed.push((e.key, outcome));
        }
    }
    for key in store::open_keys(conn, &integration.id, KEY_PREFIX)? {
        if current.contains(&key) {
            continue;
        }
        let e = Event {
            key: key.clone(),
            problem: false,
            title: String::new(),
            message: String::new(),
            severity: String::new(),
            host: "Microsoft 365".into(),
            service: String::new(),
        };
        changed.push((key, hooks::apply(conn, integration, rules, &e)?));
    }
    Ok(changed)
}

struct Token {
    value: String,
    expires: i64,
}

async fn token(client: &reqwest::Client, cfg: &M365Config) -> Result<Token, String> {
    let url = format!("{}/{}/oauth2/v2.0/token", cfg.login_url, cfg.tenant);
    let scope = format!("{}/.default", cfg.graph_url);
    let res = client
        .post(&url)
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.secret.as_str()),
            ("scope", scope.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("sign-in: {e}"))?;
    let status = res.status();
    let body: Value = res.json().await.map_err(|e| format!("sign-in: {status}: {e}"))?;
    if !status.is_success() {
        // AADSTS codes in error_description tell admins what is wrong (secret expired, wrong tenant …).
        let why = body.get("error_description").and_then(Value::as_str).unwrap_or("").lines().next().unwrap_or("");
        return Err(format!("sign-in failed ({status}): {why}"));
    }
    let value = body.get("access_token").and_then(Value::as_str).ok_or("sign-in: no access_token")?.to_string();
    let lifetime = body.get("expires_in").and_then(Value::as_i64).unwrap_or(3600);
    Ok(Token { value, expires: now() + lifetime - 120 })
}

async fn fetch_issues(client: &reqwest::Client, cfg: &M365Config, token: &str) -> Result<Vec<Value>, String> {
    let mut url =
        Some(format!("{}/v1.0/admin/serviceAnnouncement/issues?$filter=isResolved%20eq%20false", cfg.graph_url));
    let mut issues = Vec::new();
    while let Some(next) = url.take() {
        let res = client.get(&next).bearer_auth(token).send().await.map_err(|e| format!("Graph: {e}"))?;
        let status = res.status();
        let body: Value = res.json().await.map_err(|e| format!("Graph: {status}: {e}"))?;
        if !status.is_success() {
            let why = body.pointer("/error/message").and_then(Value::as_str).unwrap_or("");
            return Err(format!("Graph answered {status}: {why}"));
        }
        issues.extend(body.get("value").and_then(Value::as_array).cloned().unwrap_or_default());
        url = body.get("@odata.nextLink").and_then(Value::as_str).map(str::to_owned);
        if issues.len() > 5000 {
            break;
        }
    }
    Ok(issues)
}

fn poll_result(app: &App, cfg: &M365Config, issues: Vec<Value>) -> Result<(), String> {
    {
        let db = app.db.lock().unwrap();
        let integration = store::integrations(&db)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|i| i.name.eq_ignore_ascii_case(&cfg.integration))
            .ok_or_else(|| format!("no integration named \"{}\" – create it under Monitoring", cfg.integration))?;
        let rules = store::rules(&db).map_err(|e| e.to_string())?;
        let changed = sync(&db, &integration, &rules, cfg, &issues).map_err(|e| e.to_string())?;
        for (key, outcome) in &changed {
            info!(integration = %integration.name, key = %key, outcome, "monitoring event");
        }
        if !changed.is_empty() {
            store::touch_integration(&db, &integration.id, changed.len() as i64).map_err(|e| e.to_string())?;
        }
    }
    let open: Vec<Value> = issues
        .iter()
        .filter(|i| selected(cfg, i))
        .map(
            |i| json!({"id": s(i, "id"), "service": s(i, "service"), "title": s(i, "title"), "status": s(i, "status")}),
        )
        .collect();
    app.m365.lock().unwrap().open = open;
    refresh(app);
    Ok(())
}

pub async fn m365_loop(app: App, cfg: M365Config) {
    let client = reqwest::Client::builder().timeout(Duration::from_secs(30)).build().expect("http client");
    info!(integration = %cfg.integration, every = cfg.poll_seconds, "Microsoft 365 service health enabled");
    let mut cached: Option<Token> = None;
    tokio::time::sleep(Duration::from_secs(5)).await;
    loop {
        let result = async {
            if cached.as_ref().is_none_or(|t| t.expires <= now()) {
                cached = Some(token(&client, &cfg).await?);
            }
            let issues = fetch_issues(&client, &cfg, &cached.as_ref().unwrap().value).await;
            if issues.is_err() {
                cached = None; // a revoked or expired token is fetched anew next time
            }
            poll_result(&app, &cfg, issues?)
        }
        .await;
        {
            let mut state = app.m365.lock().unwrap();
            match result {
                Ok(()) => {
                    if state.last_error.take().is_some() {
                        info!("Microsoft 365 service health: working again");
                    }
                    state.last_ok = Some(now());
                }
                Err(e) => {
                    if state.last_error.as_deref() != Some(e.as_str()) {
                        warn!("Microsoft 365 service health: {e}");
                    }
                    state.last_error = Some(e);
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(cfg.poll_seconds)).await;
    }
}

/// `GET /api/admin/m365`: whether the poller runs, works, and which issues are open.
pub async fn m365_status(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    let state = app.m365.lock().unwrap();
    let cfg = app.m365_cfg();
    Ok(Json(json!({
        "configured": cfg.is_some(),
        "integration": cfg.map(|c| c.integration.clone()),
        "services": cfg.map(|c| c.services.clone()),
        "classifications": cfg.map(|c| c.classifications.clone()),
        "last_ok": state.last_ok, "last_error": state.last_error, "open": state.open, "now": now(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(services: &str, classes: &str) -> M365Config {
        M365Config {
            tenant: "t".into(),
            client_id: "c".into(),
            secret: "s".into(),
            integration: "Microsoft 365".into(),
            services: list(services),
            classifications: list(classes),
            poll_seconds: 120,
            graph_url: String::new(),
            login_url: String::new(),
        }
    }

    fn fixture(name: &str) -> Vec<Value> {
        let file = format!("{}/../integrations/microsoft-365/{name}", env!("CARGO_MANIFEST_DIR"));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&file).expect(&file)).expect(&file);
        v["value"].as_array().cloned().unwrap()
    }

    #[test]
    fn mapping_and_selection() {
        let issues = fixture("test-issues.json");
        let c = cfg("", "incident");
        let picked: Vec<_> = issues.iter().filter(|i| selected(&c, i)).filter_map(issue_event).collect();
        assert_eq!(picked.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(), ["m365-EX1234567", "m365-TM7654321"]);
        let ex = &picked[0];
        assert_eq!(
            (ex.service.as_str(), ex.host.as_str(), ex.severity.as_str()),
            ("Exchange Online", "Microsoft 365", "error")
        );
        assert!(ex.title.starts_with("Users can't send or receive email"));
        assert_eq!(picked[1].severity, "warning");
        let only_teams = cfg("Microsoft Teams", "incident,advisory");
        assert_eq!(issues.iter().filter(|i| selected(&only_teams, i)).count(), 2);
    }

    #[test]
    fn sync_opens_and_resolves() {
        let conn = store::open(":memory:").unwrap();
        let integration = Integration {
            id: "i1".into(),
            name: "Microsoft 365".into(),
            unmatched: "publish".into(),
            delay_seconds: 0,
            targets: vec![],
            created_at: 0,
            last_seen: None,
            events: 0,
        };
        let c = cfg("", "incident");
        let issues = fixture("test-issues.json");
        let first = sync(&conn, &integration, &[], &c, &issues).unwrap();
        assert_eq!(first, [("m365-EX1234567".to_string(), "created"), ("m365-TM7654321".to_string(), "created")]);
        assert!(sync(&conn, &integration, &[], &c, &issues).unwrap().is_empty(), "repeat poll changes nothing");
        // Exchange issue resolved: it disappears from the open list.
        let later: Vec<Value> = issues.into_iter().filter(|i| i["id"] != "EX1234567").collect();
        assert_eq!(sync(&conn, &integration, &[], &c, &later).unwrap(), [("m365-EX1234567".to_string(), "resolved")]);
    }
}
