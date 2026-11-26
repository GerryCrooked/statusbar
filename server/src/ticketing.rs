//! Outbound tickets: Statusbar opens a ticket in the ticket system when a message is published
//! (or a silent alarm raised) and adds a note and closes it when the message is resolved.
//!
//! * One ticket system per server, configured under Settings and stored in `kv` (`ticketing`).
//!   Secrets are write-only: the API never returns them.
//! * Delivery is decoupled from requests: [`reconcile`] compares messages with `ticket_links`
//!   and puts work into the SQLite queue `ticket_outbox`; [`ticket_loop`] delivers it with
//!   retries (exponential backoff, at most [`MAX_ATTEMPTS`]).
//! * Loop protection: every ticket carries [`MARKER`] plus the message id, and its key as the
//!   inbound integration would send it (`zammad-<number>`, `glpi-<id>` …) is remembered.
//!   [`own_ticket`] and [`payload_marked`] let the inbound webhooks ignore such tickets.
//!
//! Systems: a generic JSON webhook, Zammad, GLPI, Znuny/OTRS (GenericInterface), Jira Service
//! Management, ServiceNow and TOPdesk. See docs/INTEGRATIONS.md, "Statusbar → ticket system".

use crate::hooks::Event;
use crate::store::{self, now, DbResult, Incident, Settings};
use crate::{admin, bad, clean, internal, Api, App, HttpError};
use axum::{extract::State, http::HeaderMap, Json};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::Duration};
use tracing::{error, info, warn};

/// Written into every ticket (tag, correlation field or text), followed by `:<message id>`.
pub const MARKER: &str = "statusbar-origin";
/// After this many failed attempts an entry is given up.
pub const MAX_ATTEMPTS: i64 = 10;
/// Ticket systems, in the order the settings offer them.
pub const SYSTEMS: &[&str] = &["webhook", "zammad", "glpi", "znuny", "jira", "servicenow", "topdesk"];
/// Where a message comes from, as the settings filter it.
pub const SOURCES: &[&str] = &["manual", "monitoring", "alarm", "hub"];
/// Placeholders in templates. Unknown `{words}` stay as they are.
pub const PLACEHOLDERS: &[&str] = &[
    "title",
    "message",
    "severity",
    "kind",
    "targets",
    "source",
    "created_by",
    "approved_by",
    "starts_at",
    "user",
    "computer",
    "url",
    "id",
    "priority",
    "event",
    "resolved_by",
    "resolved_at",
    "ticket_id",
    "ticket_title",
    "ticket_body",
];

// ---- configuration -------------------------------------------------------------------------

/// Priority in the ticket system per kind of message (empty: the system's default for it).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Priorities {
    pub error: String,
    pub warning: String,
    pub info: String,
    pub alarm: String,
}

impl Priorities {
    fn get(&self, kind: &str) -> &str {
        match kind {
            "error" => &self.error,
            "warning" => &self.warning,
            "alarm" => &self.alarm,
            _ => &self.info,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TicketConfig {
    pub enabled: bool,
    /// One of [`SYSTEMS`].
    pub system: String,
    /// Base address of the ticket system (for `webhook`: the URL that is posted to).
    pub url: String,
    /// With a user name the secret is its password (HTTP Basic); without, it is an API token.
    pub username: String,
    /// Password or API token. Never returned by the API.
    pub secret: String,
    /// GLPI only: the API client's App-Token (optional). Never returned by the API.
    pub app_token: String,
    /// Group (Zammad), category ID (GLPI), queue (Znuny), `serviceDeskId/requestTypeId` (Jira),
    /// assignment group (ServiceNow), category (TOPdesk).
    pub target: String,
    /// Customer (Zammad e-mail, Znuny customer user), caller (ServiceNow, TOPdesk), Jira "on behalf of".
    pub customer: String,
    /// State on closing: Zammad state, GLPI 5/6, Znuny state, Jira transition ID, ServiceNow state,
    /// TOPdesk processing status. Empty: the system's default.
    pub close_status: String,
    /// Triggers: a ticket when a message is published, a ticket for messages resolved without one,
    /// a ticket when a silent alarm is raised (and closed when it ends).
    pub on_publish: bool,
    pub on_resolve: bool,
    pub on_alarm: bool,
    /// Which messages count, see [`SOURCES`].
    pub sources: Vec<String>,
    /// When a message with a ticket is resolved: `close` (note and close), `note` or `none`.
    pub resolve_mode: String,
    /// Templates; empty means the default for the server's language.
    pub title_template: String,
    pub body_template: String,
    pub note_template: String,
    pub webhook_template: String,
    pub priorities: Priorities,
    /// Address of this Statusbar server, for `{url}`.
    pub statusbar_url: String,
    /// Only messages published or resolved from this time on get tickets (set when switched on).
    pub since: i64,
}

impl Default for TicketConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            system: "webhook".into(),
            url: String::new(),
            username: String::new(),
            secret: String::new(),
            app_token: String::new(),
            target: String::new(),
            customer: String::new(),
            close_status: String::new(),
            on_publish: true,
            on_resolve: false,
            on_alarm: true,
            sources: vec!["manual".into(), "monitoring".into(), "alarm".into()],
            resolve_mode: "close".into(),
            title_template: String::new(),
            body_template: String::new(),
            note_template: String::new(),
            webhook_template: String::new(),
            priorities: Priorities::default(),
            statusbar_url: String::new(),
            since: 0,
        }
    }
}

/// The system's own priority values for Outage, Degraded, Notice and Silent alarm.
pub fn default_priorities(system: &str) -> Priorities {
    let p = |e: &str, w: &str, i: &str, a: &str| Priorities {
        error: e.into(),
        warning: w.into(),
        info: i.into(),
        alarm: a.into(),
    };
    match system {
        "zammad" => p("3 high", "2 normal", "1 low", "3 high"),
        // GLPI: 1 very low … 5 very high, 6 major.
        "glpi" => p("5", "3", "2", "6"),
        "znuny" => p("4 high", "3 normal", "2 low", "5 very high"),
        // Only if the request type has the priority field; empty leaves it to Jira.
        "jira" => p("", "", "", ""),
        // Impact and urgency: 1 high, 2 medium, 3 low.
        "servicenow" => p("1", "2", "3", "1"),
        // TOPdesk priorities are named per installation.
        "topdesk" => p("", "", "", ""),
        _ => p("high", "normal", "low", "critical"),
    }
}

/// What closing sets when `close_status` is empty.
pub fn default_close_status(system: &str) -> &'static str {
    match system {
        "zammad" => "closed",
        "glpi" => "5",
        "znuny" => "closed successful",
        "servicenow" => "6",
        _ => "",
    }
}

impl TicketConfig {
    pub fn priority(&self, kind: &str) -> String {
        let own = self.priorities.get(kind).trim();
        if own.is_empty() {
            default_priorities(&self.system).get(kind).to_string()
        } else {
            own.to_string()
        }
    }
    fn close_status(&self) -> String {
        let own = self.close_status.trim();
        if own.is_empty() {
            default_close_status(&self.system).to_string()
        } else {
            own.to_string()
        }
    }
    fn base(&self) -> String {
        self.url.trim().trim_end_matches('/').to_string()
    }
    fn template<'a>(&'a self, own: &'a str, fallback: &'static str) -> &'a str {
        if own.trim().is_empty() {
            fallback
        } else {
            own
        }
    }
}

/// Where a message comes from, as one of [`SOURCES`].
pub fn source_class(i: &Incident) -> &'static str {
    match i.source.as_str() {
        "manual" | "webhook" => "manual",
        crate::alarm::SOURCE => "alarm",
        crate::uplink::SOURCE => "hub",
        _ => "monitoring",
    }
}

/// Does publishing this message open a ticket?
pub fn wants_open(cfg: &TicketConfig, i: &Incident) -> bool {
    let class = source_class(i);
    cfg.sources.iter().any(|s| s == class)
        && if class == "alarm" { cfg.on_alarm } else { cfg.on_publish }
        && !from_this_system(cfg, i)
}

/// Does resolving this message open a ticket when it has none yet (a record of what happened)?
pub fn wants_record(cfg: &TicketConfig, i: &Incident) -> bool {
    let class = source_class(i);
    class != "alarm" && cfg.on_resolve && cfg.sources.iter().any(|s| s == class) && !from_this_system(cfg, i)
}

/// Key prefix the inbound integration of a ticket system uses (`zammad-<number>` …).
pub fn key_prefix(system: &str) -> Option<&'static str> {
    match system {
        "zammad" => Some("zammad-"),
        "glpi" => Some("glpi-"),
        "znuny" => Some("znuny-"),
        "jira" => Some("jira-"),
        "servicenow" => Some("snow-"),
        "topdesk" => Some("topdesk-"),
        _ => None,
    }
}

/// A message that came from a ticket of the same system already has its ticket: no second one.
pub fn from_this_system(cfg: &TicketConfig, i: &Incident) -> bool {
    source_class(i) == "monitoring"
        && key_prefix(&cfg.system).is_some_and(|p| i.ext_key.as_deref().is_some_and(|k| k.starts_with(p)))
}

// ---- templates -----------------------------------------------------------------------------

pub type Vars = BTreeMap<String, String>;

pub struct Templates {
    pub title: &'static str,
    pub body: &'static str,
    pub note: &'static str,
    pub webhook: &'static str,
}

const WEBHOOK_TEMPLATE: &str = r#"{
  "event": "{event}",
  "origin": "statusbar-origin:{id}",
  "id": "{id}",
  "ticket_id": "{ticket_id}",
  "title": "{title}",
  "message": "{message}",
  "kind": "{kind}",
  "severity": "{severity}",
  "priority": "{priority}",
  "targets": "{targets}",
  "source": "{source}",
  "created_by": "{created_by}",
  "approved_by": "{approved_by}",
  "starts_at": "{starts_at}",
  "resolved_by": "{resolved_by}",
  "resolved_at": "{resolved_at}",
  "user": "{user}",
  "computer": "{computer}",
  "url": "{url}",
  "subject": "{ticket_title}",
  "body": "{ticket_body}"
}"#;

/// Default templates for a language (German for `de…`, English otherwise).
pub fn default_templates(language: &str) -> Templates {
    if language.starts_with("de") {
        Templates {
            title: "{severity}: {title}",
            body: "Statusbar hat diese Meldung auf den Arbeitsplätzen angezeigt.\n\n\
                   {title}\n{message}\n\n\
                   Art: {severity}\n\
                   Empfänger: {targets}\n\
                   Herkunft: {source}\n\
                   Erstellt von: {created_by}\n\
                   Freigegeben von: {approved_by}\n\
                   Ausgelöst von: {user} an {computer}\n\
                   Angezeigt seit: {starts_at}\n\
                   {url}",
            note: "Statusbar: Meldung aufgelöst von {resolved_by} am {resolved_at}.",
            webhook: WEBHOOK_TEMPLATE,
        }
    } else {
        Templates {
            title: "{severity}: {title}",
            body: "Statusbar showed this message on users' desktops.\n\n\
                   {title}\n{message}\n\n\
                   Kind: {severity}\n\
                   Recipients: {targets}\n\
                   Source: {source}\n\
                   Created by: {created_by}\n\
                   Approved by: {approved_by}\n\
                   Raised by: {user} at {computer}\n\
                   Shown since: {starts_at}\n\
                   {url}",
            note: "Statusbar: message resolved by {resolved_by} at {resolved_at}.",
            webhook: WEBHOOK_TEMPLATE,
        }
    }
}

/// Inserts the values in one pass: text inside a value is never read as a placeholder.
pub fn render(template: &str, vars: &Vars, escape: fn(&str) -> String) -> String {
    let mut out = String::with_capacity(template.len() + 64);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}').map(|close| (close, &after[..close])) {
            Some((close, name)) if PLACEHOLDERS.contains(&name) => {
                out.push_str(&escape(vars.get(name).map(String::as_str).unwrap_or("")));
                rest = &after[close + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn plain(s: &str) -> String {
    s.to_string()
}

/// A value for use inside a JSON string: quotes, backslashes and control characters escaped.
pub fn json_escape(s: &str) -> String {
    let quoted = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    quoted[1..quoted.len() - 1].to_string()
}

/// Text templates: a line whose placeholders are all empty is left out ("Approved by: " for a
/// message nobody had to approve), so one template fits messages and alarms.
pub fn render_text(template: &str, vars: &Vars) -> String {
    let mut lines = Vec::new();
    for line in template.split('\n') {
        let used: Vec<&&str> = PLACEHOLDERS.iter().filter(|p| line.contains(&format!("{{{p}}}"))).collect();
        if !used.is_empty() && used.iter().all(|p| vars.get(**p).is_none_or(|v| v.trim().is_empty())) {
            continue;
        }
        lines.push(render(line, vars, plain));
    }
    lines.join("\n").trim().to_string()
}

/// `2026-11-26 14:05 UTC`.
pub fn utc(ts: i64) -> String {
    let days = ts.div_euclid(86_400);
    let secs = ts.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", secs / 3600, secs % 3600 / 60)
}

/// The values for one message. `event`: created, resolved or test.
pub fn incident_vars(i: &Incident, s: &Settings, cfg: &TicketConfig, event: &str) -> Vars {
    let de = s.language.starts_with("de");
    let label = match i.severity.as_str() {
        "error" => &s.label_error,
        "warning" => &s.label_warning,
        "alarm" => &s.label_alarm,
        _ => &s.label_info,
    };
    let source = match source_class(i) {
        "manual" => (if de { "manuell" } else { "manual" }).to_string(),
        "alarm" => (if de { "stiller Alarm" } else { "silent alarm" }).to_string(),
        "hub" => (if de { "Dienstleister" } else { "service provider" }).to_string(),
        _ => match i.created_by.strip_prefix("monitoring:") {
            Some(name) => format!("Monitoring ({name})"),
            None => "Monitoring".to_string(),
        },
    };
    let targets =
        if i.targets.is_empty() { (if de { "alle" } else { "everyone" }).to_string() } else { i.targets.join(", ") };
    // Silent alarms are created by `alarm:<user>@<COMPUTER>`.
    let (user, computer) = i
        .created_by
        .strip_prefix("alarm:")
        .and_then(|rest| rest.rsplit_once('@'))
        .map(|(u, c)| (u.to_string(), c.to_string()))
        .unwrap_or_default();
    let base = cfg.statusbar_url.trim().trim_end_matches('/');
    let mut v = Vars::new();
    for (key, value) in [
        ("title", i.title.clone()),
        ("message", if i.message == i.title { String::new() } else { i.message.clone() }),
        ("severity", label.clone()),
        ("kind", i.severity.clone()),
        ("targets", targets),
        ("source", source),
        ("created_by", i.created_by.clone()),
        ("approved_by", i.approved_by.clone().unwrap_or_default()),
        ("starts_at", utc(i.starts_at)),
        ("user", user),
        ("computer", computer),
        ("url", if base.is_empty() { String::new() } else { format!("{base}/#incidents") }),
        ("id", i.id.clone()),
        ("event", event.to_string()),
        ("resolved_by", i.resolved_by.clone().unwrap_or_default()),
        ("resolved_at", i.resolved_at.map(utc).unwrap_or_default()),
    ] {
        v.insert(key.to_string(), value);
    }
    v
}

/// Everything a delivery needs, rendered with the current configuration.
pub struct Rendered {
    pub title: String,
    pub body: String,
    pub note: String,
    pub priority: String,
    pub vars: Vars,
}

pub fn render_all(cfg: &TicketConfig, language: &str, vars: &Vars) -> Rendered {
    let d = default_templates(language);
    let mut vars = vars.clone();
    let kind = vars.get("kind").cloned().unwrap_or_default();
    let priority = cfg.priority(&kind);
    vars.insert("priority".into(), priority.clone());
    let title = clean(&render_text(cfg.template(&cfg.title_template, d.title), &vars), 250).replace('\n', " ");
    let id = vars.get("id").cloned().unwrap_or_default();
    let mut body = render_text(cfg.template(&cfg.body_template, d.body), &vars);
    // The marker is part of every ticket, whatever the template says.
    if !body.contains(MARKER) {
        body.push_str(&format!("\n\n[{MARKER}:{id}]"));
    }
    let mut note = render_text(cfg.template(&cfg.note_template, d.note), &vars);
    if note.is_empty() {
        // Ticket systems refuse empty notes.
        note = (if language.starts_with("de") {
            "Statusbar: Meldung aufgelöst."
        } else {
            "Statusbar: message resolved."
        })
        .to_string();
    }
    vars.insert("ticket_title".into(), title.clone());
    vars.insert("ticket_body".into(), body.clone());
    Rendered { title, body, note, priority, vars }
}

/// The webhook body, checked to be JSON.
pub fn webhook_body(cfg: &TicketConfig, language: &str, vars: &Vars) -> Result<Value, String> {
    let template = cfg.template(&cfg.webhook_template, default_templates(language).webhook);
    let text = render(template, vars, json_escape);
    serde_json::from_str(&text).map_err(|e| format!("webhook template is not valid JSON: {e}"))
}

// ---- storage -------------------------------------------------------------------------------

pub fn load(conn: &Connection) -> DbResult<TicketConfig> {
    Ok(store::kv_get(conn, "ticketing")?.and_then(|raw| serde_json::from_str(&raw).ok()).unwrap_or_default())
}

fn save(conn: &Connection, cfg: &TicketConfig) -> DbResult<()> {
    store::kv_set(conn, "ticketing", &serde_json::to_string(cfg).unwrap_or_default())
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Link {
    pub incident_id: String,
    pub system: String,
    pub ticket_id: Option<String>,
    /// The key the inbound integration of that system uses for this ticket (`zammad-123`).
    pub ticket_ref: Option<String>,
    pub ticket_url: Option<String>,
    /// 0 no ticket wanted, 1 queued, 2 created, 3 failed.
    pub opened: i64,
    /// 0 not resolved yet, 1 resolution handled (queued or nothing to do).
    pub closed: i64,
}

const LINK_COLS: &str = "incident_id,system,ticket_id,ticket_ref,ticket_url,opened,closed";

fn row_link(r: &rusqlite::Row) -> rusqlite::Result<Link> {
    Ok(Link {
        incident_id: r.get(0)?,
        system: r.get(1)?,
        ticket_id: r.get(2)?,
        ticket_ref: r.get(3)?,
        ticket_url: r.get(4)?,
        opened: r.get(5)?,
        closed: r.get(6)?,
    })
}

pub fn link(conn: &Connection, incident_id: &str) -> DbResult<Option<Link>> {
    conn.query_row(&format!("SELECT {LINK_COLS} FROM ticket_links WHERE incident_id=?1"), [incident_id], row_link)
        .optional()
}

/// Ticket links of the given messages, for the admin list.
pub fn links_for(conn: &Connection, ids: &[String]) -> DbResult<BTreeMap<String, Link>> {
    let mut out = BTreeMap::new();
    let mut st = conn.prepare(&format!("SELECT {LINK_COLS} FROM ticket_links WHERE incident_id=?1"))?;
    for id in ids {
        if let Some(l) = st.query_row([id], row_link).optional()? {
            out.insert(id.clone(), l);
        }
    }
    Ok(out)
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Entry {
    pub id: i64,
    pub incident_id: String,
    /// open, close or record (open a ticket for a resolved message, then close it).
    pub action: String,
    pub vars: Vars,
    pub attempts: i64,
    pub next_at: i64,
    pub state: String,
    pub last_error: Option<String>,
    pub created_at: i64,
}

fn row_entry(r: &rusqlite::Row) -> rusqlite::Result<Entry> {
    Ok(Entry {
        id: r.get(0)?,
        incident_id: r.get(1)?,
        action: r.get(2)?,
        vars: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
        attempts: r.get(4)?,
        next_at: r.get(5)?,
        state: r.get(6)?,
        last_error: r.get(7)?,
        created_at: r.get(8)?,
    })
}

const ENTRY_COLS: &str = "id,incident_id,action,vars,attempts,next_at,state,last_error,created_at";

pub fn enqueue(conn: &Connection, incident_id: &str, action: &str, vars: &Vars) -> DbResult<()> {
    let t = now();
    conn.execute(
        "INSERT INTO ticket_outbox(incident_id,action,vars,next_at,created_at) VALUES (?1,?2,?3,?4,?4)",
        params![incident_id, action, serde_json::to_string(vars).unwrap_or_default(), t],
    )?;
    Ok(())
}

/// The oldest entry that is due.
pub fn next_due(conn: &Connection, at: i64) -> DbResult<Option<Entry>> {
    conn.query_row(
        &format!(
            "SELECT {ENTRY_COLS} FROM ticket_outbox WHERE state='pending' AND next_at<=?1 ORDER BY next_at, id LIMIT 1"
        ),
        [at],
        row_entry,
    )
    .optional()
}

pub fn entries(conn: &Connection, limit: i64) -> DbResult<Vec<Entry>> {
    let mut st = conn.prepare(&format!("SELECT {ENTRY_COLS} FROM ticket_outbox ORDER BY id DESC LIMIT ?1"))?;
    let rows = st.query_map([limit], row_entry)?;
    rows.collect()
}

fn count(conn: &Connection, state: &str) -> DbResult<i64> {
    conn.query_row("SELECT COUNT(*) FROM ticket_outbox WHERE state=?1", [state], |r| r.get(0))
}

/// Seconds to wait after the n-th failed attempt: 30 s, doubling, at most an hour.
pub fn backoff(attempts: i64) -> i64 {
    (30i64 << (attempts - 1).clamp(0, 7)).min(3600)
}

/// Finds new work: messages published or resolved since the configuration was switched on.
/// Returns how many entries were queued.
pub fn reconcile(conn: &Connection, cfg: &TicketConfig) -> DbResult<usize> {
    if !cfg.enabled {
        return Ok(0);
    }
    let t = now();
    let mut queued = 0;
    let cols = "i.id,i.title,i.message,i.severity,i.targets,i.status,i.source,i.ext_key,i.created_at,i.starts_at,\
                i.ends_at,i.resolved_at,i.created_by,i.approved_by,i.resolved_by";
    // Published (and started) messages without a link yet.
    let published: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT i.id FROM incidents i WHERE i.status='active' AND i.starts_at<=?1 AND i.starts_at>=?2
             AND NOT EXISTS(SELECT 1 FROM ticket_links l WHERE l.incident_id=i.id) ORDER BY i.starts_at LIMIT 100",
        )?;
        let rows = st.query_map(params![t, cfg.since], |r| r.get(0))?;
        rows.collect::<DbResult<_>>()?
    };
    for id in published {
        let Some(i) = store::incident(conn, &id)? else { continue };
        let wanted = wants_open(cfg, &i);
        let vars = vars_snapshot(conn, &i, cfg)?;
        // The title is kept so that an inbound webhook racing the answer is recognised (see own_ticket).
        let title = if wanted { ticket_title(cfg, &vars) } else { String::new() };
        conn.execute(
            "INSERT OR IGNORE INTO ticket_links(incident_id,system,title,opened,closed,created_at) VALUES (?1,?2,?3,?4,0,?5)",
            params![i.id, cfg.system, title, if wanted { 1 } else { 0 }, t],
        )?;
        if wanted {
            enqueue(conn, &i.id, "open", &vars)?;
            queued += 1;
        }
    }
    // Resolved messages whose resolution has not been handled.
    let resolved: Vec<(Incident, Option<Link>)> = {
        let mut st = conn.prepare(&format!(
            "SELECT {cols} FROM incidents i LEFT JOIN ticket_links l ON l.incident_id=i.id
             WHERE i.status='resolved' AND i.resolved_at>=?1 AND COALESCE(l.closed,0)=0
               AND i.starts_at<=i.resolved_at ORDER BY i.resolved_at LIMIT 100"
        ))?;
        let rows = st.query_map([cfg.since], |r| {
            let incident = Incident {
                id: r.get(0)?,
                title: r.get(1)?,
                message: r.get(2)?,
                severity: r.get(3)?,
                targets: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
                status: r.get(5)?,
                source: r.get(6)?,
                ext_key: r.get(7)?,
                created_at: r.get(8)?,
                starts_at: r.get(9)?,
                ends_at: r.get(10)?,
                resolved_at: r.get(11)?,
                created_by: r.get(12)?,
                approved_by: r.get(13)?,
                resolved_by: r.get(14)?,
            };
            Ok(incident)
        })?;
        let list: Vec<Incident> = rows.collect::<DbResult<_>>()?;
        let mut out = Vec::new();
        for i in list {
            let l = link(conn, &i.id)?;
            out.push((i, l));
        }
        out
    };
    for (i, l) in resolved {
        let vars = vars_snapshot(conn, &i, cfg)?;
        let has_ticket = l.as_ref().is_some_and(|l| l.opened == 1 || l.opened == 2);
        let record = !has_ticket && wants_record(cfg, &i);
        if has_ticket && cfg.resolve_mode != "none" {
            enqueue(conn, &i.id, "close", &vars)?;
            queued += 1;
        } else if record {
            enqueue(conn, &i.id, "record", &vars)?;
            queued += 1;
        }
        let opened = if record { 1 } else { l.as_ref().map_or(0, |l| l.opened) };
        let title =
            if record { ticket_title(cfg, &vars) } else { l.as_ref().map(|_| String::new()).unwrap_or_default() };
        conn.execute(
            "INSERT INTO ticket_links(incident_id,system,title,opened,closed,created_at) VALUES (?1,?2,?3,?4,1,?5)
             ON CONFLICT(incident_id) DO UPDATE SET closed=1, opened=excluded.opened,
               title=CASE WHEN excluded.title='' THEN title ELSE excluded.title END",
            params![i.id, cfg.system, title, opened, t],
        )?;
    }
    // Old history.
    conn.execute("DELETE FROM ticket_outbox WHERE state!='pending' AND created_at<?1", [t - 30 * 86_400])?;
    conn.execute("DELETE FROM ticket_links WHERE created_at<?1", [t - 365 * 86_400])?;
    Ok(queued)
}

fn ticket_title(cfg: &TicketConfig, vars: &Vars) -> String {
    render_all(cfg, vars.get("_language").map(String::as_str).unwrap_or(""), vars).title
}

/// Values of a message at the time it was queued (it may be deleted before delivery).
fn vars_snapshot(conn: &Connection, i: &Incident, cfg: &TicketConfig) -> DbResult<Vars> {
    let settings: Settings =
        store::load_settings_raw(conn)?.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let event = if i.status == "resolved" { "resolved" } else { "created" };
    let mut v = incident_vars(i, &settings, cfg, event);
    v.insert("_language".into(), settings.language);
    Ok(v)
}

/// Result of one delivery attempt.
#[derive(Debug, PartialEq)]
pub enum Fail {
    /// Network trouble, timeouts, 5xx, 429: try again later.
    Retry(String),
    /// Configuration or data the ticket system refuses (4xx): trying again will not help.
    Permanent(String),
}

impl Fail {
    fn text(&self) -> &str {
        match self {
            Fail::Retry(s) | Fail::Permanent(s) => s,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct Created {
    pub id: String,
    pub reference: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    Opened(Created),
    /// Opened for a resolved message; closing follows as a separate step.
    Recorded(Created),
    Closed,
    /// The ticket is still being opened: ask again shortly (no attempt counted).
    Wait,
    /// Nothing to do (no ticket was opened, or resolving is switched off).
    Skipped(&'static str),
    Failed(Fail),
}

/// Stores what an attempt did. Returns a short text for the status.
pub fn finish(conn: &Connection, e: &Entry, outcome: &Outcome, system: &str) -> DbResult<()> {
    let t = now();
    match outcome {
        Outcome::Opened(c) | Outcome::Recorded(c) => {
            conn.execute(
                "INSERT INTO ticket_links(incident_id,system,title,ticket_id,ticket_ref,ticket_url,opened,closed,created_at)
                 VALUES (?1,?2,'',?3,?4,?5,2,0,?6)
                 ON CONFLICT(incident_id) DO UPDATE SET ticket_id=excluded.ticket_id, ticket_ref=excluded.ticket_ref,
                   ticket_url=excluded.ticket_url, opened=2, system=excluded.system",
                params![e.incident_id, system, c.id, c.reference, c.url, t],
            )?;
            if matches!(outcome, Outcome::Recorded(_)) {
                // Closing is its own step with its own retries.
                conn.execute(
                    "UPDATE ticket_outbox SET action='close', attempts=0, next_at=?1, last_error=NULL WHERE id=?2",
                    params![t, e.id],
                )?;
            } else {
                conn.execute("UPDATE ticket_outbox SET state='done', done_at=?1 WHERE id=?2", params![t, e.id])?;
            }
        }
        Outcome::Closed => {
            conn.execute("UPDATE ticket_outbox SET state='done', done_at=?1 WHERE id=?2", params![t, e.id])?;
        }
        Outcome::Wait => {
            conn.execute("UPDATE ticket_outbox SET next_at=?1 WHERE id=?2", params![t + 10, e.id])?;
        }
        Outcome::Skipped(why) => {
            conn.execute(
                "UPDATE ticket_outbox SET state='skipped', done_at=?1, last_error=?2 WHERE id=?3",
                params![t, why, e.id],
            )?;
        }
        Outcome::Failed(f) => {
            let attempts = e.attempts + 1;
            let give_up = matches!(f, Fail::Permanent(_)) || attempts >= MAX_ATTEMPTS;
            conn.execute(
                "UPDATE ticket_outbox SET attempts=?1, next_at=?2, last_error=?3, state=?4, done_at=?5 WHERE id=?6",
                params![
                    attempts,
                    t + backoff(attempts),
                    clean(f.text(), 500),
                    if give_up { "failed" } else { "pending" },
                    give_up.then_some(t),
                    e.id
                ],
            )?;
            if give_up && e.action != "close" {
                conn.execute("UPDATE ticket_links SET opened=3 WHERE incident_id=?1 AND opened!=2", [&e.incident_id])?;
            }
        }
    }
    Ok(())
}

// ---- loop protection -------------------------------------------------------------------------

/// Is this inbound event about a ticket that Statusbar opened itself? Matched by the ticket's key
/// (`zammad-<number>` …) or, while the ticket is still being opened, by its title: some systems
/// call their webhooks before they have answered the request that opened the ticket.
pub fn own_ticket(conn: &Connection, e: &Event) -> DbResult<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM ticket_links WHERE ticket_ref=?1
           OR (ticket_ref IS NULL AND opened=1 AND title!='' AND title=?2 AND created_at>?3))",
        params![e.key, e.title, now() - 600],
        |r| r.get(0),
    )
}

/// Does an inbound payload carry the marker anywhere (tags, text, correlation field)?
pub fn payload_marked(v: &Value) -> bool {
    match v {
        Value::String(s) => s.contains(MARKER),
        Value::Array(list) => list.iter().any(payload_marked),
        Value::Object(map) => map.iter().any(|(k, v)| k.contains(MARKER) || payload_marked(v)),
        _ => false,
    }
}

// ---- delivery --------------------------------------------------------------------------------

type Http = reqwest::Client;

fn snippet(body: &Value) -> String {
    let text = match body {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    text.chars().take(300).collect()
}

/// Sends a request and reads the answer as JSON (or as a string).
async fn call(req: reqwest::RequestBuilder, what: &str) -> Result<Value, Fail> {
    let res = req.header("Accept", "application/json").send().await.map_err(|e| {
        let mut why = e.to_string();
        let mut source = std::error::Error::source(&e);
        while let Some(s) = source {
            why = format!("{why}: {s}");
            source = s.source();
        }
        Fail::Retry(format!("{what}: {why}"))
    })?;
    let status = res.status();
    let text = res.text().await.unwrap_or_default();
    let body = serde_json::from_str(&text).unwrap_or(Value::String(text));
    if status.is_success() {
        return Ok(body);
    }
    let why = format!("{what}: {status} {}", snippet(&body));
    if status.is_server_error() || status.as_u16() == 408 || status.as_u16() == 429 {
        Err(Fail::Retry(why))
    } else {
        Err(Fail::Permanent(why))
    }
}

fn s(v: &Value, pointer: &str) -> String {
    match v.pointer(pointer) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// HTTP Basic with a user name, otherwise the token in the system's usual header.
fn auth(req: reqwest::RequestBuilder, cfg: &TicketConfig, token_scheme: &str) -> reqwest::RequestBuilder {
    if !cfg.username.trim().is_empty() {
        req.basic_auth(cfg.username.trim(), Some(&cfg.secret))
    } else if !cfg.secret.is_empty() {
        let sep = if token_scheme.ends_with('=') { "" } else { " " };
        req.header("Authorization", format!("{token_scheme}{sep}{}", cfg.secret))
    } else {
        req
    }
}

fn web_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    (raw.starts_with("https://") || raw.starts_with("http://")).then(|| raw.to_string())
}

fn html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('\n', "<br>\n")
}

async fn open_ticket(http: &Http, cfg: &TicketConfig, r: &Rendered) -> Result<Created, Fail> {
    let base = cfg.base();
    let id = r.vars.get("id").cloned().unwrap_or_default();
    let marker = format!("{MARKER}:{id}");
    match cfg.system.as_str() {
        "zammad" => {
            let customer = cfg.customer.trim();
            let customer = if customer.contains('@') && !customer.starts_with("guess:") {
                format!("guess:{customer}")
            } else {
                customer.to_string()
            };
            let mut body = json!({
                "title": r.title, "group": cfg.target.trim(), "customer_id": customer, "tags": MARKER,
                "article": {"subject": r.title, "body": r.body, "type": "note", "internal": false, "content_type": "text/plain"},
            });
            if !r.priority.is_empty() {
                body["priority"] = json!(r.priority);
            }
            let req = zammad_auth(http.post(format!("{base}/api/v1/tickets")), cfg).json(&body);
            let v = call(req, "Zammad").await?;
            let id = s(&v, "/id");
            if id.is_empty() {
                return Err(Fail::Permanent(format!("Zammad: no ticket id in {}", snippet(&v))));
            }
            Ok(Created {
                reference: Some(format!("zammad-{}", s(&v, "/number"))),
                url: Some(format!("{base}/#ticket/zoom/{id}")),
                id,
            })
        }
        "glpi" => {
            let session = glpi_session(http, cfg).await?;
            let mut input = json!({"name": r.title, "content": html(&r.body), "type": 1});
            if let Ok(p) = r.priority.parse::<i64>() {
                input["priority"] = json!(p);
                input["urgency"] = json!(p.min(5));
                input["impact"] = json!(p.min(5));
            }
            if let Ok(category) = cfg.target.trim().parse::<i64>() {
                input["itilcategories_id"] = json!(category);
            }
            let result =
                call(glpi(http, cfg, &session, reqwest::Method::POST, "Ticket").json(&json!({"input": input})), "GLPI")
                    .await;
            glpi_end(http, cfg, &session).await;
            let v = result?;
            let id = s(&v, "/id");
            if id.is_empty() {
                return Err(Fail::Permanent(format!("GLPI: no ticket id in {}", snippet(&v))));
            }
            Ok(Created {
                reference: Some(format!("glpi-{id}")),
                url: Some(format!("{}/front/ticket.form.php?id={id}", glpi_root(cfg))),
                id,
            })
        }
        "znuny" => {
            let mut ticket = json!({"Title": r.title, "Queue": cfg.target.trim(), "State": "new",
                "CustomerUser": cfg.customer.trim()});
            if !r.priority.is_empty() {
                ticket["Priority"] = json!(r.priority);
            } else {
                ticket["PriorityID"] = json!(3);
            }
            let body = json!({
                "UserLogin": cfg.username.trim(), "Password": cfg.secret, "Ticket": ticket,
                "Article": {"CommunicationChannel": "Internal", "SenderType": "agent", "IsVisibleForCustomer": 1,
                    "Subject": r.title, "Body": r.body, "ContentType": "text/plain; charset=utf8"},
            });
            let v = call(http.post(format!("{}/Ticket", znuny_ws(cfg))).json(&body), "Znuny").await?;
            znuny_error(&v)?;
            let id = s(&v, "/TicketID");
            if id.is_empty() {
                return Err(Fail::Permanent(format!("Znuny: no TicketID in {}", snippet(&v))));
            }
            Ok(Created {
                reference: Some(format!("znuny-{}", s(&v, "/TicketNumber"))),
                url: Some(format!("{base}/index.pl?Action=AgentTicketZoom;TicketID={id}")),
                id,
            })
        }
        "jira" => {
            let (desk, kind) = cfg.target.trim().split_once('/').unwrap_or(("", ""));
            let mut fields = json!({"summary": r.title.chars().take(250).collect::<String>(), "description": r.body});
            if !r.priority.is_empty() {
                fields["priority"] = json!({"name": r.priority});
            }
            let mut body =
                json!({"serviceDeskId": desk.trim(), "requestTypeId": kind.trim(), "requestFieldValues": fields});
            if !cfg.customer.trim().is_empty() {
                body["raiseOnBehalfOf"] = json!(cfg.customer.trim());
            }
            let v =
                call(auth(http.post(format!("{base}/rest/servicedeskapi/request")), cfg, "Bearer").json(&body), "Jira")
                    .await?;
            let key = s(&v, "/issueKey");
            if key.is_empty() {
                return Err(Fail::Permanent(format!("Jira: no issueKey in {}", snippet(&v))));
            }
            Ok(Created {
                reference: Some(format!("jira-{key}")),
                url: web_url(&s(&v, "/_links/web")).or_else(|| Some(format!("{base}/browse/{key}"))),
                id: key,
            })
        }
        "servicenow" => {
            let mut body = json!({
                "short_description": r.title.chars().take(160).collect::<String>(), "description": r.body,
                "correlation_id": marker, "correlation_display": "Statusbar",
            });
            if !r.priority.is_empty() {
                body["impact"] = json!(r.priority);
                body["urgency"] = json!(r.priority);
            }
            if !cfg.target.trim().is_empty() {
                body["assignment_group"] = json!(cfg.target.trim());
            }
            if !cfg.customer.trim().is_empty() {
                body["caller_id"] = json!(cfg.customer.trim());
            }
            let req = auth(http.post(format!("{base}/api/now/table/incident")), cfg, "Bearer").json(&body);
            let v = call(req, "ServiceNow").await?;
            let id = s(&v, "/result/sys_id");
            if id.is_empty() {
                return Err(Fail::Permanent(format!("ServiceNow: no sys_id in {}", snippet(&v))));
            }
            Ok(Created {
                reference: Some(format!("snow-{}", s(&v, "/result/number"))),
                url: Some(format!("{base}/nav_to.do?uri=incident.do?sys_id={id}")),
                id,
            })
        }
        "topdesk" => {
            let caller = if cfg.customer.trim().is_empty() { "Statusbar" } else { cfg.customer.trim() };
            let mut body = json!({
                "caller": {"dynamicName": caller}, "briefDescription": r.title.chars().take(80).collect::<String>(),
                "request": r.body, "externalNumber": marker.chars().take(60).collect::<String>(),
            });
            if !cfg.target.trim().is_empty() {
                body["category"] = json!({"name": cfg.target.trim()});
            }
            if !r.priority.is_empty() {
                body["priority"] = json!({"name": r.priority});
            }
            let v = call(auth(http.post(format!("{base}/tas/api/incidents")), cfg, "Bearer").json(&body), "TOPdesk")
                .await?;
            let id = s(&v, "/id");
            if id.is_empty() {
                return Err(Fail::Permanent(format!("TOPdesk: no id in {}", snippet(&v))));
            }
            Ok(Created {
                reference: Some(format!("topdesk-{}", s(&v, "/number"))),
                url: Some(format!("{base}/tas/secure/incident?unid={id}")),
                id,
            })
        }
        _ => {
            let language = r.vars.get("_language").cloned().unwrap_or_default();
            let body = webhook_body(cfg, &language, &r.vars).map_err(Fail::Permanent)?;
            let v = call(auth(http.post(&base), cfg, "Bearer").json(&body), "Webhook").await?;
            // An answer such as {"id": 17, "url": "https://…"} is remembered; anything else is fine too.
            let id = ["/id", "/ticket_id", "/number", "/key"].iter().map(|p| s(&v, p)).find(|x| !x.is_empty());
            Ok(Created { id: id.clone().unwrap_or_default(), reference: None, url: web_url(&s(&v, "/url")) })
        }
    }
}

async fn close_ticket(http: &Http, cfg: &TicketConfig, link: &Link, r: &Rendered, close: bool) -> Result<(), Fail> {
    let base = cfg.base();
    let id = link.ticket_id.clone().unwrap_or_default();
    let state = cfg.close_status();
    match cfg.system.as_str() {
        "zammad" => {
            let mut body =
                json!({"article": {"body": r.note, "type": "note", "internal": true, "content_type": "text/plain"}});
            if close && !state.is_empty() {
                body["state"] = json!(state);
            }
            let req = zammad_auth(http.put(format!("{base}/api/v1/tickets/{id}")), cfg);
            call(req.json(&body), "Zammad").await.map(|_| ())
        }
        "glpi" => {
            let session = glpi_session(http, cfg).await?;
            let result = async {
                let input = json!({"input": {"itemtype": "Ticket", "items_id": id, "content": html(&r.note)}});
                if close {
                    // A solution sets the ticket to "solved" (5); 6 closes it as well.
                    call(glpi(http, cfg, &session, reqwest::Method::POST, "ITILSolution").json(&input), "GLPI").await?;
                    if state == "6" {
                        let update = json!({"input": {"status": 6}});
                        call(
                            glpi(http, cfg, &session, reqwest::Method::PUT, &format!("Ticket/{id}")).json(&update),
                            "GLPI",
                        )
                        .await?;
                    }
                } else {
                    let mut followup = input.clone();
                    followup["input"]["is_private"] = json!(1);
                    call(glpi(http, cfg, &session, reqwest::Method::POST, "ITILFollowup").json(&followup), "GLPI")
                        .await?;
                }
                Ok(())
            }
            .await;
            glpi_end(http, cfg, &session).await;
            result
        }
        "znuny" => {
            let mut body = json!({
                "UserLogin": cfg.username.trim(), "Password": cfg.secret,
                "Article": {"CommunicationChannel": "Internal", "SenderType": "agent", "IsVisibleForCustomer": 0,
                    "Subject": r.title, "Body": r.note, "ContentType": "text/plain; charset=utf8"},
            });
            if close && !state.is_empty() {
                body["Ticket"] = json!({"State": state});
            }
            let v = call(http.patch(format!("{}/Ticket/{id}", znuny_ws(cfg))).json(&body), "Znuny").await?;
            znuny_error(&v)
        }
        "jira" => {
            let comment = json!({"body": r.note, "public": false});
            if close && !state.is_empty() {
                let body = json!({"id": state, "additionalComment": {"body": r.note}});
                let req = auth(http.post(format!("{base}/rest/servicedeskapi/request/{id}/transition")), cfg, "Bearer");
                call(req.json(&body), "Jira").await.map(|_| ())
            } else {
                let req = auth(http.post(format!("{base}/rest/servicedeskapi/request/{id}/comment")), cfg, "Bearer");
                call(req.json(&comment), "Jira").await.map(|_| ())
            }
        }
        "servicenow" => {
            let mut body = json!({"work_notes": r.note});
            if close && !state.is_empty() {
                // "6" or "6;Solved (Permanently)": state and resolution code (names differ by release).
                let (state, code) = state.split_once(';').unwrap_or((&state, "Solution provided"));
                body["state"] = json!(state.trim());
                body["close_code"] = json!(code.trim());
                body["close_notes"] = json!(r.note);
            }
            let req = auth(http.patch(format!("{base}/api/now/table/incident/{id}")), cfg, "Bearer");
            call(req.json(&body), "ServiceNow").await.map(|_| ())
        }
        "topdesk" => {
            let mut body = json!({"action": r.note, "actionInvisibleForCaller": true});
            if close {
                body["completed"] = json!(true);
                body["closed"] = json!(true);
                if !state.is_empty() {
                    body["processingStatus"] = json!({"name": state});
                }
            }
            let req = auth(http.patch(format!("{base}/tas/api/incidents/id/{id}")), cfg, "Bearer");
            call(req.json(&body), "TOPdesk").await.map(|_| ())
        }
        _ => {
            let mut vars = r.vars.clone();
            vars.insert("ticket_id".into(), id);
            vars.insert("ticket_body".into(), r.note.clone());
            let language = vars.get("_language").cloned().unwrap_or_default();
            let body = webhook_body(cfg, &language, &vars).map_err(Fail::Permanent)?;
            call(auth(http.post(&base), cfg, "Bearer").json(&body), "Webhook").await.map(|_| ())
        }
    }
}

/// Zammad: an access token (`Token token=…`) or a user with password.
fn zammad_auth(req: reqwest::RequestBuilder, cfg: &TicketConfig) -> reqwest::RequestBuilder {
    auth(req, cfg, "Token token=").header("Accept", "application/json")
}

/// GLPI's legacy REST API lives under `apirest.php` next to the web interface.
fn glpi_root(cfg: &TicketConfig) -> String {
    let base = cfg.base();
    base.strip_suffix("/apirest.php").unwrap_or(&base).to_string()
}

fn glpi(
    http: &Http,
    cfg: &TicketConfig,
    session: &str,
    method: reqwest::Method,
    path: &str,
) -> reqwest::RequestBuilder {
    let mut req =
        http.request(method, format!("{}/apirest.php/{path}", glpi_root(cfg))).header("Session-Token", session);
    if !cfg.app_token.is_empty() {
        req = req.header("App-Token", &cfg.app_token);
    }
    req
}

async fn glpi_session(http: &Http, cfg: &TicketConfig) -> Result<String, Fail> {
    let mut req = http.get(format!("{}/apirest.php/initSession", glpi_root(cfg)));
    req = if cfg.username.trim().is_empty() {
        req.header("Authorization", format!("user_token {}", cfg.secret))
    } else {
        req.basic_auth(cfg.username.trim(), Some(&cfg.secret))
    };
    if !cfg.app_token.is_empty() {
        req = req.header("App-Token", &cfg.app_token);
    }
    let v = call(req, "GLPI sign-in").await?;
    let token = s(&v, "/session_token");
    if token.is_empty() {
        return Err(Fail::Permanent(format!("GLPI sign-in: no session_token in {}", snippet(&v))));
    }
    Ok(token)
}

async fn glpi_end(http: &Http, cfg: &TicketConfig, session: &str) {
    let _ = glpi(http, cfg, session, reqwest::Method::GET, "killSession").send().await;
}

/// The web service `Statusbar` (integrations/znuny-otrs/Statusbar-Webservice.yml).
fn znuny_ws(cfg: &TicketConfig) -> String {
    format!("{}/nph-genericinterface.pl/Webservice/Statusbar", cfg.base())
}

/// Znuny answers errors with status 200 and an `Error` object.
fn znuny_error(v: &Value) -> Result<(), Fail> {
    match v.get("Error") {
        Some(e) => Err(Fail::Permanent(format!("Znuny: {} {}", s(e, "/ErrorCode"), s(e, "/ErrorMessage")))),
        None => Ok(()),
    }
}

/// One attempt for one queue entry.
pub async fn deliver(http: &Http, cfg: &TicketConfig, link: Option<&Link>, e: &Entry) -> Outcome {
    let language = e.vars.get("_language").cloned().unwrap_or_default();
    let r = render_all(cfg, &language, &e.vars);
    match e.action.as_str() {
        "open" | "record" => match open_ticket(http, cfg, &r).await {
            Ok(c) if e.action == "record" && cfg.resolve_mode == "close" => Outcome::Recorded(c),
            Ok(c) => Outcome::Opened(c),
            Err(f) => Outcome::Failed(f),
        },
        _ => {
            let Some(l) = link else { return Outcome::Skipped("no ticket") };
            if l.opened == 1 {
                return Outcome::Wait;
            }
            if l.opened != 2 {
                return Outcome::Skipped("no ticket was opened");
            }
            if cfg.resolve_mode == "none" {
                return Outcome::Skipped("resolving is switched off");
            }
            if l.ticket_id.as_deref().unwrap_or("").is_empty() && cfg.system != "webhook" {
                return Outcome::Skipped("ticket id unknown");
            }
            match close_ticket(http, cfg, l, &r, cfg.resolve_mode == "close").await {
                Ok(()) => Outcome::Closed,
                Err(f) => Outcome::Failed(f),
            }
        }
    }
}

// ---- background loop and status ------------------------------------------------------------

#[derive(Default)]
pub struct TicketState {
    pub last_ok: Option<i64>,
    pub last_ok_what: Option<String>,
    pub last_error: Option<String>,
    pub last_error_at: Option<i64>,
}

fn client() -> Http {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!("Statusbar/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("http client")
}

pub async fn ticket_loop(app: App) {
    let http = client();
    loop {
        tokio::select! {
            _ = app.ticket_wake.notified() => {}
            _ = tokio::time::sleep(Duration::from_secs(15)) => {}
        }
        let cfg = app.ticketing.read().unwrap().clone();
        if !cfg.enabled {
            continue;
        }
        if let Err(e) = reconcile(&app.db.lock().unwrap(), &cfg) {
            error!("tickets: reconcile failed: {e}");
            continue;
        }
        // A bounded batch per round, so a slow ticket system cannot starve the loop.
        for _ in 0..20 {
            let (entry, link) = {
                let db = app.db.lock().unwrap();
                match next_due(&db, now()) {
                    Ok(Some(e)) => {
                        let l = link(&db, &e.incident_id).ok().flatten();
                        (e, l)
                    }
                    _ => break,
                }
            };
            let outcome = deliver(&http, &cfg, link.as_ref(), &entry).await;
            {
                let mut state = app.ticket_state.lock().unwrap();
                match &outcome {
                    Outcome::Opened(c) | Outcome::Recorded(c) => {
                        info!(incident = %entry.incident_id, ticket = %c.id, "ticket opened");
                        state.last_ok = Some(now());
                        state.last_ok_what = Some(format!("opened {}", c.reference.clone().unwrap_or(c.id.clone())));
                    }
                    Outcome::Closed => {
                        info!(incident = %entry.incident_id, "ticket updated on resolve");
                        state.last_ok = Some(now());
                        state.last_ok_what = Some("resolved".into());
                    }
                    Outcome::Failed(f) => {
                        warn!(incident = %entry.incident_id, action = %entry.action, attempt = entry.attempts + 1, "ticket: {}", f.text());
                        state.last_error = Some(f.text().to_string());
                        state.last_error_at = Some(now());
                    }
                    _ => {}
                }
            }
            if let Err(e) = finish(&app.db.lock().unwrap(), &entry, &outcome, &cfg.system) {
                error!("tickets: cannot store result: {e}");
                break;
            }
            if outcome == Outcome::Wait {
                break;
            }
        }
    }
}

/// Configuration as the admin UI sees it: secrets only as "set".
fn public(cfg: &TicketConfig) -> Value {
    let mut v = json!(cfg);
    v["secret"] = json!("");
    v["app_token"] = json!("");
    v["secret_set"] = json!(!cfg.secret.is_empty());
    v["app_token_set"] = json!(!cfg.app_token.is_empty());
    v
}

/// `GET /api/admin/ticketing`: configuration, defaults, queue and last results.
pub async fn status(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    let cfg = app.ticketing.read().unwrap().clone();
    let language = app.settings.read().unwrap().language.clone();
    let (pending, failed, recent) = {
        let db = app.db.lock().unwrap();
        (
            count(&db, "pending").map_err(internal)?,
            count(&db, "failed").map_err(internal)?,
            entries(&db, 15).map_err(internal)?,
        )
    };
    let recent: Vec<Value> = recent
        .into_iter()
        .map(|e| {
            json!({"id": e.id, "incident_id": e.incident_id, "title": e.vars.get("title"), "action": e.action,
                   "state": e.state, "attempts": e.attempts, "next_at": e.next_at, "last_error": e.last_error,
                   "created_at": e.created_at})
        })
        .collect();
    let state = app.ticket_state.lock().unwrap();
    let t = default_templates(&language);
    let defaults: serde_json::Map<String, Value> = SYSTEMS
        .iter()
        .map(|s| (s.to_string(), json!({"priorities": default_priorities(s), "close_status": default_close_status(s)})))
        .collect();
    Ok(Json(json!({
        "config": public(&cfg),
        "templates": {"title": t.title, "body": t.body, "note": t.note, "webhook": t.webhook},
        "defaults": defaults, "placeholders": PLACEHOLDERS,
        "pending": pending, "failed": failed, "recent": recent,
        "last_ok": state.last_ok, "last_ok_what": state.last_ok_what,
        "last_error": state.last_error, "last_error_at": state.last_error_at, "now": now(),
    })))
}

#[derive(Deserialize)]
pub struct ConfigForm {
    #[serde(flatten)]
    cfg: TicketConfig,
    /// Removes the stored secret / app token (an empty field keeps it).
    #[serde(default)]
    clear_secret: bool,
    #[serde(default)]
    clear_app_token: bool,
}

/// Checks and completes a configuration. `old` supplies secrets that were not re-entered.
pub fn sanitize(
    form: TicketConfig,
    old: &TicketConfig,
    clear_secret: bool,
    clear_app: bool,
) -> Result<TicketConfig, String> {
    let mut c = form;
    c.system = c.system.trim().to_ascii_lowercase();
    if !SYSTEMS.contains(&c.system.as_str()) {
        return Err(format!("system must be one of {}", SYSTEMS.join(", ")));
    }
    c.url = c.url.trim().trim_end_matches('/').to_string();
    if !c.url.is_empty() && !(c.url.starts_with("https://") || c.url.starts_with("http://")) || c.url.len() > 500 {
        return Err("URL must start with https:// (or http://)".into());
    }
    c.statusbar_url = c.statusbar_url.trim().trim_end_matches('/').to_string();
    if !c.statusbar_url.is_empty() && web_url(&c.statusbar_url).is_none() {
        return Err("Statusbar address must start with https://".into());
    }
    for (field, max) in
        [(&mut c.username, 200), (&mut c.target, 200), (&mut c.customer, 200), (&mut c.close_status, 100)]
    {
        *field = clean(field, max).replace('\n', " ");
    }
    if clear_secret {
        c.secret.clear();
    } else if c.secret.is_empty() {
        c.secret = old.secret.clone();
    }
    if clear_app {
        c.app_token.clear();
    } else if c.app_token.is_empty() {
        c.app_token = old.app_token.clone();
    }
    c.secret = c.secret.trim().chars().take(2000).collect();
    c.app_token = c.app_token.trim().chars().take(500).collect();
    c.sources.retain(|s| SOURCES.contains(&s.as_str()));
    c.sources.sort_by_key(|s| SOURCES.iter().position(|x| x == s));
    c.sources.dedup();
    if !["close", "note", "none"].contains(&c.resolve_mode.as_str()) {
        c.resolve_mode = "close".into();
    }
    for (field, max) in [
        (&mut c.title_template, 500),
        (&mut c.body_template, 5000),
        (&mut c.note_template, 2000),
        (&mut c.webhook_template, 10_000),
    ] {
        *field = field.replace("\r\n", "\n").chars().filter(|ch| !ch.is_control() || *ch == '\n').take(max).collect();
        if field.trim().is_empty() {
            field.clear();
        }
    }
    for p in [&mut c.priorities.error, &mut c.priorities.warning, &mut c.priorities.info, &mut c.priorities.alarm] {
        *p = clean(p, 60).replace('\n', " ");
    }
    if c.system == "webhook" {
        let sample = sample_vars();
        webhook_body(&c, "en", &sample)?;
    }
    if c.enabled {
        let missing = |what: &str| Err(format!("{what} required for {}", c.system));
        if c.url.is_empty() {
            return missing("URL");
        }
        match c.system.as_str() {
            "zammad" if c.target.is_empty() || c.customer.is_empty() => return missing("Group and customer"),
            "znuny" if c.target.is_empty() || c.customer.is_empty() || c.username.is_empty() => {
                return missing("Queue, customer user and agent login")
            }
            "jira" if !c.target.contains('/') => return missing("Target as serviceDeskId/requestTypeId"),
            "zammad" | "glpi" | "znuny" | "jira" | "servicenow" | "topdesk" if c.secret.is_empty() => {
                return missing("Password or token")
            }
            _ => {}
        }
    }
    // Switched on now: only what happens from here on gets tickets.
    c.since = if c.enabled && !old.enabled { now() } else { old.since };
    Ok(c)
}

fn sample_vars() -> Vars {
    let i = Incident {
        id: "00000000-0000-4000-8000-000000000000".into(),
        title: "Test \"ticket\" \\ C:\\temp".into(),
        message: "Line 1\nLine 2 – ü".into(),
        severity: "warning".into(),
        targets: vec![],
        status: "active".into(),
        source: "manual".into(),
        ext_key: None,
        created_at: now(),
        starts_at: now(),
        ends_at: None,
        resolved_at: None,
        created_by: "admin".into(),
        approved_by: None,
        resolved_by: None,
    };
    incident_vars(&i, &Settings::default(), &TicketConfig::default(), "test")
}

/// `PUT /api/admin/ticketing`.
pub async fn put_config(State(app): State<App>, headers: HeaderMap, Json(form): Json<ConfigForm>) -> Api {
    admin(&app, &headers)?;
    let old = app.ticketing.read().unwrap().clone();
    let cfg = sanitize(form.cfg, &old, form.clear_secret, form.clear_app_token).map_err(|e| bad(&e))?;
    {
        let db = app.db.lock().unwrap();
        save(&db, &cfg).map_err(internal)?;
    }
    *app.ticketing.write().unwrap() = cfg.clone();
    app.ticket_wake.notify_one();
    Ok(Json(public(&cfg)))
}

/// `POST /api/admin/ticketing/test`: opens a test ticket right away and reports the result.
pub async fn test_ticket(State(app): State<App>, headers: HeaderMap) -> Api {
    let by = crate::actor(&app, &headers)?;
    let cfg = app.ticketing.read().unwrap().clone();
    if cfg.url.is_empty() {
        return Err(bad("Save the ticket system first"));
    }
    let settings = app.settings.read().unwrap().clone();
    let de = settings.language.starts_with("de");
    let t = now();
    let i = Incident {
        id: format!("test-{}", uuid::Uuid::new_v4()),
        title: if de { "Testticket von Statusbar" } else { "Test ticket from Statusbar" }.into(),
        message: if de {
            "Dieses Ticket prüft die Verbindung. Es kann gelöscht werden."
        } else {
            "This ticket checks the connection. You can delete it."
        }
        .into(),
        severity: "info".into(),
        targets: vec![],
        status: "active".into(),
        source: "manual".into(),
        ext_key: None,
        created_at: t,
        starts_at: t,
        ends_at: None,
        resolved_at: None,
        created_by: by,
        approved_by: None,
        resolved_by: None,
    };
    let mut vars = incident_vars(&i, &settings, &cfg, "test");
    vars.insert("_language".into(), settings.language.clone());
    let r = render_all(&cfg, &settings.language, &vars);
    {
        // Known before the ticket exists, so an inbound webhook for it is recognised at once.
        let db = app.db.lock().unwrap();
        db.execute(
            "INSERT INTO ticket_links(incident_id,system,title,opened,closed,created_at) VALUES (?1,?2,?3,1,1,?4)",
            params![i.id, cfg.system, r.title, t],
        )
        .map_err(internal)?;
    }
    let result = open_ticket(&client(), &cfg, &r).await;
    let db = app.db.lock().unwrap();
    match result {
        Ok(c) => {
            db.execute(
                "UPDATE ticket_links SET ticket_id=?1, ticket_ref=?2, ticket_url=?3, opened=2 WHERE incident_id=?4",
                params![c.id, c.reference, c.url, i.id],
            )
            .map_err(internal)?;
            let mut state = app.ticket_state.lock().unwrap();
            state.last_ok = Some(now());
            state.last_ok_what = Some(format!("test {}", c.reference.clone().unwrap_or(c.id.clone())));
            info!(ticket = %c.id, "test ticket opened");
            Ok(Json(json!({"ok": true, "ticket_id": c.id, "reference": c.reference, "url": c.url})))
        }
        Err(f) => {
            db.execute("UPDATE ticket_links SET opened=3 WHERE incident_id=?1", [&i.id]).map_err(internal)?;
            let mut state = app.ticket_state.lock().unwrap();
            state.last_error = Some(f.text().to_string());
            state.last_error_at = Some(now());
            Ok(Json(json!({"ok": false, "error": f.text()})))
        }
    }
}

/// `POST /api/admin/ticketing/retry`: failed entries are tried again.
pub async fn retry(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    let n = {
        let db = app.db.lock().unwrap();
        db.execute(
            "UPDATE ticket_links SET opened=1 WHERE opened=3 AND incident_id IN
                    (SELECT incident_id FROM ticket_outbox WHERE state='failed' AND action!='close')",
            [],
        )
        .map_err(internal)?;
        db.execute(
            "UPDATE ticket_outbox SET state='pending', attempts=0, next_at=?1, done_at=NULL WHERE state='failed'",
            [now()],
        )
        .map_err(internal)?
    };
    app.ticket_wake.notify_one();
    Ok(Json(json!({"ok": true, "retried": n})))
}

/// Ticket of each listed message, for the admin list.
pub fn decorate(conn: &Connection, list: &mut [Value]) -> Result<(), HttpError> {
    let ids: Vec<String> = list.iter().filter_map(|v| v.get("id").and_then(Value::as_str).map(str::to_owned)).collect();
    let links = links_for(conn, &ids).map_err(internal)?;
    for v in list.iter_mut() {
        let Some(l) = v.get("id").and_then(Value::as_str).and_then(|id| links.get(id)) else { continue };
        if l.opened == 0 {
            continue;
        }
        let state = match l.opened {
            1 => "queued",
            2 => "open",
            _ => "failed",
        };
        v["ticket"] = json!({"id": l.ticket_id, "reference": l.ticket_ref, "url": l.ticket_url, "state": state,
                             "system": l.system});
    }
    Ok(())
}

/// Loads the configuration at start.
pub fn initial(conn: &Connection) -> TicketConfig {
    load(conn).unwrap_or_else(|e| {
        error!("ticket configuration unreadable, starting switched off: {e}");
        TicketConfig::default()
    })
}

#[cfg(test)]
mod tests;
