//! Connection to a service provider's hub (`HUB_URL` + `HUB_TOKEN`).
//!
//! The server keeps one outgoing HTTPS long-poll connection to the hub: it reports its
//! state (version, computers, titles of active messages, names of notification groups)
//! and takes over the hub's messages according to the `hub_messages` setting
//! (publish, hold for approval, or off). No inbound port is needed.
//!
//! Protocol: `POST {HUB_URL}/api/hub/sync?rev=<n>&wait=<s>` with `Authorization: Bearer
//! <HUB_TOKEN>` and a [`Report`] as JSON body. The hub answers `{rev, hub, messages}`,
//! holding the request until something changes when `rev` is current. See docs/HUB.md.

use crate::store::{self, now, Incident};
use crate::{admin, clean, refresh, Api, App};
use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::HashSet, time::Duration};
use tracing::{info, warn};

/// Messages for customers carry this source on the customer instance.
pub const SOURCE: &str = "hub";
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
    /// Names of the customer's notification groups, so the hub can address them.
    pub teams: Vec<String>,
    /// Computers with the client, seen in the last 30 days (no user names).
    pub computers: Vec<ReportComputer>,
    /// How hub messages are taken over here: publish, approve or off.
    pub accepts: String,
    /// What happened here to the hub's messages, so the hub can show it per customer.
    pub states: Vec<ReportState>,
}

/// State of one hub message on the customer instance.
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct ReportState {
    /// The hub's message id.
    pub id: String,
    /// pending (waiting for approval), shown, resolved or discarded.
    pub state: String,
    /// Who approved, resolved or discarded it here (empty when nobody did).
    pub by: String,
    pub at: i64,
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct ReportComputer {
    pub computer: String,
    pub version: String,
    pub last_seen: i64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct HubMessage {
    pub id: String,
    pub title: String,
    pub message: String,
    pub severity: String,
    pub status: String,
    /// Notification groups at the customer; empty means everyone there.
    pub teams: Vec<String>,
}

#[derive(Deserialize)]
struct SyncReply {
    rev: u64,
    #[serde(default)]
    hub: String,
    #[serde(default)]
    messages: Vec<HubMessage>,
}

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
    let computers: Vec<ReportComputer> = { store::computers(&app.db.lock().unwrap(), now() - ACTIVE_WINDOW) }
        .unwrap_or_default()
        .into_iter()
        .map(|c| ReportComputer { computer: c.computer, version: c.version, last_seen: c.last_seen })
        .collect();
    let clients = computers.len() as i64;
    let live = app.live.read().unwrap().clone();
    let incidents = live
        .iter()
        .filter(|i| i.status == "active")
        .map(|i| ReportIncident {
            title: i.title.clone(),
            severity: i.severity.clone(),
            source: match i.source.as_str() {
                "manual" | "webhook" | crate::alarm::SOURCE | SOURCE => i.source.clone(),
                _ => "monitoring".into(),
            },
        })
        .collect();
    let teams = app.teams.read().unwrap().iter().map(|t| t.name.clone()).collect();
    let accepts = app.settings.read().unwrap().hub_messages.clone();
    let mut seen = HashSet::new();
    let states = { store::states_by_source(&app.db.lock().unwrap(), SOURCE) }
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| {
            let id = i.ext_key.clone()?;
            if !seen.insert(id.clone()) {
                return None; // only the newest local copy counts
            }
            let (state, by, at) = match i.status.as_str() {
                "draft" => ("pending", None, i.created_at),
                "active" => ("shown", i.approved_by.clone(), i.starts_at),
                "discarded" => ("discarded", i.resolved_by.clone(), i.resolved_at.unwrap_or(i.created_at)),
                _ => ("resolved", i.resolved_by.clone(), i.resolved_at.unwrap_or(i.created_at)),
            };
            let by = by.filter(|b| !b.starts_with("hub:")).unwrap_or_default();
            Some(ReportState { id, state: state.into(), by, at })
        })
        .collect();
    Report { version: env!("CARGO_PKG_VERSION").into(), name, clients, incidents, teams, computers, accepts, states }
}

/// Mirrors the hub's messages into local incidents. Returns whether anything changed.
fn apply(app: &App, hub: &str, messages: &[HubMessage]) -> Result<bool, rusqlite::Error> {
    let mode = app.settings.read().unwrap().hub_messages.clone();
    let actor = format!("hub:{}", clean(if hub.is_empty() { "service provider" } else { hub }, 70));
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
                        targets: m.teams.iter().take(20).map(|t| format!("team:{}", clean(t, 60))).collect(),
                        status: if mode == "approve" { "draft".into() } else { "active".into() },
                        source: SOURCE.into(),
                        ext_key: Some(id),
                        created_at: t,
                        starts_at: t,
                        ends_at: None,
                        resolved_at: None,
                        created_by: actor.clone(),
                        approved_by: None,
                        resolved_by: None,
                    },
                )?;
                changed = true;
            }
            Some(local) if !active && local.status == "active" => {
                store::resolve_incident(&db, &local.id, &actor)?;
                changed = true;
            }
            Some(local) if !active && local.status == "draft" => {
                store::delete_incident(&db, &local.id)?;
                changed = true;
            }
            Some(local) if !active && local.status == "discarded" && local.ends_at.is_none() => {
                store::close_discarded(&db, &local.id)?;
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
                store::resolve_incident(&db, &local.id, &actor)?;
            }
            changed = true;
        }
    }
    for local in store::discarded_open_by_source(&db, SOURCE)? {
        if local.ext_key.as_ref().is_some_and(|key| !listed.contains(key)) {
            store::close_discarded(&db, &local.id)?;
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
        let exchange = async {
            match request.send().await.and_then(|r| r.error_for_status()) {
                Ok(response) => response.json::<SyncReply>().await.map_err(|e| e.to_string()),
                Err(e) => Err(match e.status() {
                    Some(StatusCode::UNAUTHORIZED) => {
                        "hub rejected the token (customer removed or token wrong)".to_string()
                    }
                    Some(status) => format!("hub answered {status}"),
                    None => e.without_url().to_string(),
                }),
            }
        };
        let result = tokio::select! {
            result = exchange => result,
            // Something the report covers changed here (e.g. a hub message was discarded):
            // drop the waiting long-poll and report again right away. The hub stores the
            // report before it waits, so nothing is lost.
            _ = app.uplink_wake.notified() => continue,
        };
        match result {
            Ok(reply) => {
                if failures > 0 {
                    info!("hub connection restored");
                }
                failures = 0;
                rev = Some(reply.rev);
                match apply(&app, &reply.hub, &reply.messages) {
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
