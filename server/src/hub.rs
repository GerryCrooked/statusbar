//! Service-provider hub.
//!
//! * Hub side (`HUB_ENABLED=yes`): customers are registered with their own token. Each
//!   customer instance reports its state and long-polls for messages. Messages whose
//!   targets include `customer:<name pattern>` are forwarded to matching customers and
//!   are never shown on the hub's own desktops.
//! * Customer side (`HUB_URL` + `HUB_TOKEN`): the instance keeps one outgoing connection
//!   to the hub, takes over its messages according to the `hub_messages` setting
//!   (publish, hold for approval, or off) and reports version, desktop count and the
//!   titles of active messages. No inbound port is needed at the customer.

use crate::store::{self, now, Incident};
use crate::{admin, bad, bearer, clean, glob, internal, ok, random_token, refresh, sha256_hex, Api, App, HttpError};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, time::Duration};
use tracing::{info, warn};

/// Messages for customers carry this source on the customer instance.
const SOURCE: &str = "hub";
/// A customer counts as online if it contacted the hub this recently (long-poll is 25 s).
const ONLINE_SECONDS: i64 = 90;
/// Desktops seen within this window count as active.
pub const ACTIVE_WINDOW: i64 = 30 * 86_400;

#[derive(Default)]
pub struct UplinkState {
    pub last_ok: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct ReportIncident {
    pub title: String,
    pub severity: String,
    pub source: String,
}

/// What a customer instance tells the hub on every call.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Report {
    pub version: String,
    pub name: String,
    pub clients: i64,
    pub incidents: Vec<ReportIncident>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct HubMessage {
    pub id: String,
    pub title: String,
    pub message: String,
    pub severity: String,
    pub status: String,
}

#[derive(Deserialize)]
struct SyncReply {
    rev: u64,
    #[serde(default)]
    messages: Vec<HubMessage>,
}

/// True if any `customer:` target matches this customer name.
pub fn for_customer(i: &Incident, name: &str) -> bool {
    i.targets.iter().any(|t| t.strip_prefix("customer:").is_some_and(|pattern| glob::matches(pattern, name)))
}

fn require_hub(app: &App) -> Result<(), HttpError> {
    if app.cfg.hub_enabled {
        Ok(())
    } else {
        Err((StatusCode::NOT_FOUND, "This server is not a hub (HUB_ENABLED=yes)".into()))
    }
}

// ---- hub side --------------------------------------------------------------------

pub async fn list(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    require_hub(&app)?;
    let list = { store::customers(&app.db.lock().unwrap()) }.map_err(internal)?;
    let t = now();
    let customers: Vec<Value> = list
        .into_iter()
        .map(|c| {
            let online = c.last_seen.is_some_and(|seen| t - seen < ONLINE_SECONDS);
            let mut v = json!(c);
            v["online"] = json!(online);
            v
        })
        .collect();
    Ok(Json(json!({"customers": customers, "now": t})))
}

#[derive(Deserialize)]
pub struct NewCustomer {
    name: String,
}

pub async fn create(State(app): State<App>, headers: HeaderMap, Json(form): Json<NewCustomer>) -> Api {
    admin(&app, &headers)?;
    require_hub(&app)?;
    let name = clean(&form.name, 80).replace('\n', " ");
    if name.is_empty() || name.contains(':') || name.contains('*') || name.contains('?') {
        return Err(bad("Name required, without : * ?"));
    }
    let id = uuid::Uuid::new_v4().to_string();
    // Shown once; only the hash is stored.
    let token = format!("sbh_{}", random_token());
    {
        let db = app.db.lock().unwrap();
        if store::customer_name_taken(&db, &name).map_err(internal)? {
            return Err((StatusCode::CONFLICT, "A customer with this name exists".into()));
        }
        store::insert_customer(&db, &id, &name, &sha256_hex(&token)).map_err(internal)?;
    }
    info!(customer = %name, "customer created");
    Ok(Json(json!({"customer": {"id": id, "name": name}, "token": token})))
}

pub async fn remove(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Api {
    admin(&app, &headers)?;
    require_hub(&app)?;
    let n = { store::delete_customer(&app.db.lock().unwrap(), &id) }.map_err(internal)?;
    if n == 0 {
        return Err((StatusCode::NOT_FOUND, "Not found".into()));
    }
    ok()
}

#[derive(Deserialize)]
pub struct SyncQuery {
    rev: Option<u64>,
    wait: Option<u64>,
}

/// Called by customer instances: store their report, then long-poll for message changes.
pub async fn sync(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<SyncQuery>,
    Json(report): Json<Report>,
) -> Api {
    require_hub(&app)?;
    let denied = || (StatusCode::UNAUTHORIZED, "Unknown customer token".to_string());
    let token = bearer(&headers);
    if token.len() < 20 {
        return Err(denied());
    }
    let incidents: Vec<ReportIncident> = report
        .incidents
        .iter()
        .take(50)
        .map(|i| ReportIncident {
            title: clean(&i.title, 120),
            severity: clean(&i.severity, 10),
            source: clean(&i.source, 20),
        })
        .collect();
    let customer = {
        let db = app.db.lock().unwrap();
        let customer = store::customer_by_hash(&db, &sha256_hex(token)).map_err(internal)?.ok_or_else(denied)?;
        let incidents = serde_json::to_string(&incidents).unwrap_or_else(|_| "[]".into());
        store::update_customer(&db, &customer.id, &clean(&report.version, 20), report.clients.max(0), &incidents)
            .map_err(internal)?;
        customer
    };
    let mut rx = app.rev.subscribe();
    let current = *rx.borrow_and_update();
    if q.rev == Some(current) {
        let wait = q.wait.unwrap_or(25).clamp(1, 55);
        let _ = tokio::time::timeout(Duration::from_secs(wait), rx.changed()).await;
    }
    let rev = *app.rev.borrow();
    let live = app.live.read().unwrap().clone();
    let messages: Vec<Value> = live
        .iter()
        .filter(|i| for_customer(i, &customer.name))
        .map(
            |i| json!({"id": i.id, "title": i.title, "message": i.message, "severity": i.severity, "status": i.status}),
        )
        .collect();
    Ok(Json(json!({"rev": rev, "messages": messages})))
}

// ---- customer side -----------------------------------------------------------------

pub async fn uplink_status(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    let state = app.uplink.lock().unwrap();
    let hub = app.cfg.uplink.as_ref().map(|(url, _)| url.clone());
    Ok(Json(json!({
        "configured": hub.is_some(), "hub": hub,
        "last_ok": state.last_ok, "last_error": state.last_error, "now": now(),
    })))
}

fn report(app: &App) -> Report {
    let name = app.settings.read().unwrap().name.clone();
    let clients = { store::active_clients(&app.db.lock().unwrap(), now() - ACTIVE_WINDOW) }.unwrap_or(0);
    let live = app.live.read().unwrap().clone();
    let incidents = live
        .iter()
        .filter(|i| i.status == "active")
        .map(|i| ReportIncident {
            title: i.title.clone(),
            severity: i.severity.clone(),
            source: match i.source.as_str() {
                "manual" | "webhook" | SOURCE => i.source.clone(),
                _ => "monitoring".into(),
            },
        })
        .collect();
    Report { version: env!("CARGO_PKG_VERSION").into(), name, clients, incidents }
}

/// Mirrors the hub's messages into local incidents. Returns whether anything changed.
fn apply(app: &App, messages: &[HubMessage]) -> Result<bool, rusqlite::Error> {
    let mode = app.settings.read().unwrap().hub_messages.clone();
    let db = app.db.lock().unwrap();
    let mut changed = false;
    let mut listed = HashSet::new();
    for m in messages.iter().take(100) {
        let id = clean(&m.id, 64);
        if id.is_empty() {
            continue;
        }
        listed.insert(id.clone());
        let active = m.status == "active";
        match store::latest_by_key(&db, SOURCE, &id)? {
            None if active && mode != "off" => {
                let severity = if ["error", "warning", "info"].contains(&m.severity.as_str()) {
                    m.severity.clone()
                } else {
                    "error".into()
                };
                let t = now();
                store::insert_incident(
                    &db,
                    &Incident {
                        id: uuid::Uuid::new_v4().to_string(),
                        title: clean(&m.title, 120),
                        message: clean(&m.message, 1500),
                        severity,
                        targets: vec![],
                        status: if mode == "approve" { "draft".into() } else { "active".into() },
                        source: SOURCE.into(),
                        ext_key: Some(id),
                        created_at: t,
                        starts_at: t,
                        ends_at: None,
                        resolved_at: None,
                    },
                )?;
                changed = true;
            }
            Some(local) if !active && local.status == "active" => {
                store::resolve_incident(&db, &local.id)?;
                changed = true;
            }
            Some(local) if !active && local.status == "draft" => {
                store::delete_incident(&db, &local.id)?;
                changed = true;
            }
            _ => {}
        }
    }
    // Gone from the hub entirely (deleted there, or the customer was renamed): close locally.
    for local in store::open_by_source(&db, SOURCE)? {
        if local.ext_key.as_ref().is_some_and(|key| !listed.contains(key)) {
            if local.status == "draft" {
                store::delete_incident(&db, &local.id)?;
            } else {
                store::resolve_incident(&db, &local.id)?;
            }
            changed = true;
        }
    }
    Ok(changed)
}

pub async fn uplink_loop(app: App, url: String, token: String) {
    let endpoint = format!("{}/api/hub/sync", url.trim_end_matches('/'));
    let http = match reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent(concat!("statusbar-server/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(http) => http,
        Err(e) => {
            warn!("hub connection disabled: {e}");
            return;
        }
    };
    info!("connecting to hub {url}");
    let mut rev: Option<u64> = None;
    let mut failures: u64 = 0;
    loop {
        let mut request = http.post(&endpoint).bearer_auth(&token).json(&report(&app));
        if let Some(current) = rev {
            request = request.query(&[("rev", current.to_string()), ("wait", "25".to_string())]);
        }
        let result = match request.send().await.and_then(|r| r.error_for_status()) {
            Ok(response) => response.json::<SyncReply>().await.map_err(|e| e.to_string()),
            Err(e) => Err(match e.status() {
                Some(StatusCode::UNAUTHORIZED) => {
                    "hub rejected the token (customer removed or token wrong)".to_string()
                }
                Some(status) => format!("hub answered {status}"),
                None => e.without_url().to_string(),
            }),
        };
        match result {
            Ok(reply) => {
                if failures > 0 {
                    info!("hub connection restored");
                }
                failures = 0;
                rev = Some(reply.rev);
                match apply(&app, &reply.messages) {
                    Ok(true) => refresh(&app),
                    Ok(false) => {}
                    Err(e) => warn!("applying hub messages failed: {e}"),
                }
                {
                    let mut state = app.uplink.lock().unwrap();
                    state.last_ok = Some(now());
                    state.last_error = None;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(problem) => {
                failures += 1;
                if failures == 1 || failures % 30 == 0 {
                    warn!("hub unreachable ({failures}x): {problem}");
                }
                app.uplink.lock().unwrap().last_error = Some(problem);
                rev = None;
                tokio::time::sleep(Duration::from_secs((failures * 5).min(60))).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn incident(targets: &[&str]) -> Incident {
        Incident {
            id: "1".into(),
            title: "t".into(),
            message: "m".into(),
            severity: "error".into(),
            targets: targets.iter().map(|s| s.to_string()).collect(),
            status: "active".into(),
            source: "manual".into(),
            ext_key: None,
            created_at: 0,
            starts_at: 0,
            ends_at: None,
            resolved_at: None,
        }
    }

    #[test]
    fn customer_targets_match_by_name_pattern() {
        assert!(for_customer(&incident(&["customer:*"]), "Acme"));
        assert!(for_customer(&incident(&["customer:acme"]), "Acme"));
        assert!(for_customer(&incident(&["host:PC-*", "customer:Ac*"]), "Acme"));
        assert!(!for_customer(&incident(&["customer:Other*"]), "Acme"));
        assert!(!for_customer(&incident(&[]), "Acme"));
        assert!(!for_customer(&incident(&["host:*"]), "Acme"));
    }
}
