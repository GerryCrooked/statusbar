//! SQLite persistence. One connection, WAL mode, integer unix timestamps.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub type DbResult<T> = Result<T, rusqlite::Error>;

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Incident {
    pub id: String,
    pub title: String,
    pub message: String,
    pub severity: String,
    pub targets: Vec<String>,
    pub status: String,
    pub source: String,
    pub ext_key: Option<String>,
    pub created_at: i64,
    pub starts_at: i64,
    pub ends_at: Option<i64>,
    pub resolved_at: Option<i64>,
    /// Who published it: a user name, "api", "monitoring:<integration>", "hub:<hub name>".
    pub created_by: String,
    /// Who approved a held message.
    pub approved_by: Option<String>,
    /// Who resolved it: as created_by, or "schedule".
    pub resolved_by: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub name: String,
    pub subtitle: String,
    pub color: String,
    pub language: String,
    pub position: String,
    pub all_monitors: bool,
    pub height_px: i32,
    pub initial_height_px: i32,
    pub initial_seconds: u64,
    pub resolved_seconds: i64,
    pub over_fullscreen: bool,
    /// Full-screen Remote Desktop: overlay (bar on top), windowed (take it out of full screen), hide (no bar on that monitor).
    pub rdp_mode: String,
    /// Messages from a service-provider hub: publish, approve (hold as draft) or off.
    pub hub_messages: String,
    pub label_error: String,
    pub label_warning: String,
    pub label_info: String,
    pub label_resolved: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            name: "Statusbar".into(),
            subtitle: "Service status notifications".into(),
            color: "#1f3a5f".into(),
            language: "en".into(),
            position: "top".into(),
            all_monitors: true,
            height_px: 44,
            initial_height_px: 88,
            initial_seconds: 5,
            resolved_seconds: 90,
            over_fullscreen: true,
            rdp_mode: "hide".into(),
            hub_messages: "publish".into(),
            label_error: "Outage".into(),
            label_warning: "Degraded".into(),
            label_info: "Notice".into(),
            label_resolved: "Resolved".into(),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Integration {
    pub id: String,
    pub name: String,
    pub unmatched: String,
    pub delay_seconds: i64,
    pub targets: Vec<String>,
    pub created_at: i64,
    pub last_seen: Option<i64>,
    pub events: i64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Rule {
    pub id: String,
    pub pattern: String,
    pub title: String,
    pub message: String,
    pub severity: String,
    pub targets: Vec<String>,
}

const COLS: &str = "id,title,message,severity,targets,status,source,ext_key,created_at,starts_at,ends_at,resolved_at,\
                    created_by,approved_by,resolved_by";

fn list_json(raw: String) -> Vec<String> {
    serde_json::from_str(&raw).unwrap_or_default()
}
fn to_json(list: &[String]) -> String {
    serde_json::to_string(list).unwrap_or_else(|_| "[]".into())
}

fn row_incident(r: &Row) -> rusqlite::Result<Incident> {
    Ok(Incident {
        id: r.get(0)?,
        title: r.get(1)?,
        message: r.get(2)?,
        severity: r.get(3)?,
        targets: list_json(r.get(4)?),
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
    })
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS incidents(
  id TEXT PRIMARY KEY, title TEXT NOT NULL, message TEXT NOT NULL, severity TEXT NOT NULL,
  targets TEXT NOT NULL DEFAULT '[]', status TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'manual', ext_key TEXT,
  created_at INTEGER NOT NULL, starts_at INTEGER NOT NULL, ends_at INTEGER, resolved_at INTEGER,
  created_by TEXT NOT NULL DEFAULT '', approved_by TEXT, resolved_by TEXT);
CREATE INDEX IF NOT EXISTS idx_incidents_open ON incidents(source, ext_key) WHERE status != 'resolved';
CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY, value BLOB NOT NULL);
CREATE TABLE IF NOT EXISTS integrations(
  id TEXT PRIMARY KEY, name TEXT NOT NULL, token_hash TEXT NOT NULL UNIQUE,
  unmatched TEXT NOT NULL, delay_seconds INTEGER NOT NULL, targets TEXT NOT NULL,
  created_at INTEGER NOT NULL, last_seen INTEGER, events INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS teams(
  id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE, targets TEXT NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS clients(
  computer TEXT PRIMARY KEY, user TEXT NOT NULL, last_seen INTEGER NOT NULL, version TEXT NOT NULL DEFAULT '');
CREATE TABLE IF NOT EXISTS rules(
  id TEXT PRIMARY KEY, pattern TEXT NOT NULL, title TEXT NOT NULL, message TEXT NOT NULL,
  severity TEXT NOT NULL, targets TEXT NOT NULL, created_at INTEGER NOT NULL);
";

/// Moves a pre-0.5 database aside (tables keep their data under a `_v04` suffix)
/// and copies branding plus incidents into the new layout.
const MIGRATE_V04: &str = "
ALTER TABLE incidents RENAME TO incidents_v04;
ALTER TABLE branding RENAME TO branding_v04;
";
const MIGRATE_V04_COPY: &str = "
INSERT OR IGNORE INTO incidents(id,title,message,severity,targets,status,source,ext_key,created_at,starts_at,ends_at,resolved_at)
SELECT id, title, message, severity,
  CASE target_type
    WHEN 'all' THEN '[]'
    WHEN 'custom' THEN COALESCE(
      (SELECT json_group_array('group:' || ad_group_dn) FROM custom_group_members WHERE group_id = target_dn
        HAVING COUNT(*) > 0), '[\"group:unknown\"]')
    ELSE json_array(target_type || ':' || target_dn)
  END,
  status, 'manual', NULL,
  COALESCE(CAST(strftime('%s', created_at) AS INTEGER), CAST(strftime('%s','now') AS INTEGER)),
  COALESCE(CAST(strftime('%s', created_at) AS INTEGER), CAST(strftime('%s','now') AS INTEGER)),
  NULL,
  CASE WHEN status = 'resolved'
    THEN COALESCE(CAST(strftime('%s', resolved_at) AS INTEGER), CAST(strftime('%s','now') AS INTEGER)) END
FROM incidents_v04;
";

pub fn open(path: &str) -> DbResult<Connection> {
    let mut conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;")?;
    let legacy: i64 =
        conn.query_row("SELECT COUNT(*) FROM pragma_table_info('incidents') WHERE name='target_dn'", [], |r| r.get(0))?;
    let tx = conn.transaction()?;
    if legacy > 0 {
        tx.execute_batch(MIGRATE_V04)?;
        tx.execute_batch(SCHEMA)?;
        tx.execute_batch(MIGRATE_V04_COPY)?;
        let old = tx
            .query_row("SELECT name,subtitle,color,language,logo,logo_mime FROM branding_v04 WHERE id=1", [], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<Vec<u8>>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                ))
            })
            .optional()?;
        if let Some((name, subtitle, color, language, logo, mime)) = old {
            // Branding only; display values are completed from defaults at startup.
            let partial = serde_json::json!({"name": name, "subtitle": subtitle, "color": color, "language": language});
            tx.execute("INSERT OR REPLACE INTO kv(key,value) VALUES('settings',?1)", [partial.to_string()])?;
            if let (Some(logo), Some(mime)) = (logo, mime) {
                set_logo(&tx, Some((logo.as_slice(), mime.as_str())))?;
            }
        }
    } else {
        tx.execute_batch(SCHEMA)?;
    }
    // Columns added after the first 0.5 databases were created.
    for (table, column, definition) in [
        ("incidents", "created_by", "TEXT NOT NULL DEFAULT ''"),
        ("incidents", "approved_by", "TEXT"),
        ("incidents", "resolved_by", "TEXT"),
        ("clients", "version", "TEXT NOT NULL DEFAULT ''"),
    ] {
        let present: i64 =
            tx.query_row("SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name=?2", params![table, column], |r| {
                r.get(0)
            })?;
        if present == 0 {
            tx.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"))?;
        }
    }
    // Custom groups from 0.4 become notification groups, once.
    let legacy_groups: i64 = tx.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='custom_group_members'",
        [],
        |r| r.get(0),
    )?;
    let imported = tx.query_row("SELECT 1 FROM kv WHERE key='teams_imported'", [], |_| Ok(())).optional()?.is_some();
    if legacy_groups > 0 && !imported {
        tx.execute_batch(
            "INSERT OR IGNORE INTO teams(id,name,targets,created_at)
             SELECT g.id, g.name,
               (SELECT json_group_array('group:' || m.ad_group_dn) FROM custom_group_members m WHERE m.group_id=g.id),
               CAST(strftime('%s','now') AS INTEGER)
             FROM custom_groups g;
             INSERT OR REPLACE INTO kv(key,value) VALUES('teams_imported','1');",
        )?;
    }
    tx.commit()?;
    Ok(conn)
}

// ---- settings / logo -------------------------------------------------------

/// Stored settings as raw JSON; keys may be missing after an upgrade.
pub fn load_settings_raw(conn: &Connection) -> DbResult<Option<serde_json::Value>> {
    let raw: Option<String> =
        conn.query_row("SELECT value FROM kv WHERE key='settings'", [], |r| r.get(0)).optional()?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
}
pub fn save_settings(conn: &Connection, s: &Settings) -> DbResult<()> {
    let raw = serde_json::to_string(s).unwrap_or_default();
    conn.execute(
        "INSERT INTO kv(key,value) VALUES('settings',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [raw],
    )?;
    Ok(())
}
pub fn logo(conn: &Connection) -> DbResult<Option<(Vec<u8>, String)>> {
    let data: Option<Vec<u8>> = conn.query_row("SELECT value FROM kv WHERE key='logo'", [], |r| r.get(0)).optional()?;
    let mime: Option<String> =
        conn.query_row("SELECT value FROM kv WHERE key='logo_mime'", [], |r| r.get(0)).optional()?;
    Ok(data.zip(mime))
}
pub fn set_logo(conn: &Connection, logo: Option<(&[u8], &str)>) -> DbResult<()> {
    conn.execute("DELETE FROM kv WHERE key IN ('logo','logo_mime')", [])?;
    if let Some((data, mime)) = logo {
        conn.execute("INSERT INTO kv(key,value) VALUES('logo',?1)", params![data])?;
        conn.execute("INSERT INTO kv(key,value) VALUES('logo_mime',?1)", params![mime])?;
    }
    Ok(())
}

// ---- incidents -------------------------------------------------------------

pub fn insert_incident(conn: &Connection, i: &Incident) -> DbResult<()> {
    conn.execute(
        &format!("INSERT INTO incidents({COLS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)"),
        params![
            i.id,
            i.title,
            i.message,
            i.severity,
            to_json(&i.targets),
            i.status,
            i.source,
            i.ext_key,
            i.created_at,
            i.starts_at,
            i.ends_at,
            i.resolved_at,
            i.created_by,
            i.approved_by,
            i.resolved_by
        ],
    )?;
    Ok(())
}

/// Newest incidents for the admin view.
pub fn recent_incidents(conn: &Connection, limit: i64) -> DbResult<Vec<Incident>> {
    let mut st = conn.prepare(&format!(
        "SELECT {COLS} FROM incidents ORDER BY (status IN ('resolved','discarded')), created_at DESC LIMIT ?1"
    ))?;
    let rows = st.query_map([limit], row_incident)?;
    rows.collect()
}

/// What clients may currently see: started active incidents plus recently resolved ones.
pub fn live_incidents(conn: &Connection, resolved_seconds: i64) -> DbResult<Vec<Incident>> {
    let now = now();
    let mut st = conn.prepare(&format!(
        "SELECT {COLS} FROM incidents
         WHERE (status='active' AND starts_at<=?1 AND (ends_at IS NULL OR ends_at>?1))
            OR (status='resolved' AND resolved_at>?2)
         ORDER BY (status='resolved'),
                  CASE severity WHEN 'error' THEN 0 WHEN 'warning' THEN 1 ELSE 2 END,
                  created_at DESC"
    ))?;
    let rows = st.query_map(params![now, now - resolved_seconds], row_incident)?;
    rows.collect()
}

pub fn resolve_incident(conn: &Connection, id: &str, by: &str) -> DbResult<usize> {
    conn.execute(
        "UPDATE incidents SET status='resolved', resolved_at=?1, resolved_by=?2 WHERE id=?3 AND status='active'",
        params![now(), by, id],
    )
}
pub fn publish_incident(conn: &Connection, id: &str, by: &str) -> DbResult<usize> {
    conn.execute(
        "UPDATE incidents SET status='active', starts_at=?1, approved_by=?2 WHERE id=?3 AND status='draft'",
        params![now(), by, id],
    )
}

pub fn delete_incident(conn: &Connection, id: &str) -> DbResult<usize> {
    conn.execute("DELETE FROM incidents WHERE id=?1", [id])
}

/// A message from the hub that is turned down here is kept as `discarded`, so the next
/// sync does not bring it back and the hub can be told who declined it.
pub fn discard_incident(conn: &Connection, id: &str, by: &str) -> DbResult<usize> {
    conn.execute(
        "UPDATE incidents SET status='discarded', resolved_at=?1, resolved_by=?2 WHERE id=?3 AND status='draft'",
        params![now(), by, id],
    )
}

/// What happened locally to the messages that came from one source, newest first.
pub fn states_by_source(conn: &Connection, source: &str) -> DbResult<Vec<Incident>> {
    let mut st = conn.prepare(&format!(
        "SELECT {COLS} FROM incidents WHERE source=?1 AND ext_key IS NOT NULL ORDER BY created_at DESC LIMIT 200"
    ))?;
    let rows = st.query_map([source], row_incident)?;
    rows.collect()
}

/// Periodic housekeeping: end scheduled incidents, drop old history.
pub fn housekeeping(conn: &Connection, retention_days: i64) -> DbResult<()> {
    let now = now();
    conn.execute(
        "UPDATE incidents SET status='resolved', resolved_at=ends_at, resolved_by='schedule'
         WHERE status='active' AND ends_at IS NOT NULL AND ends_at<=?1",
        [now],
    )?;
    conn.execute(
        "DELETE FROM incidents WHERE status IN ('resolved','discarded') AND resolved_at<?1",
        [now - retention_days * 86_400],
    )?;
    Ok(())
}

pub fn incident(conn: &Connection, id: &str) -> DbResult<Option<Incident>> {
    conn.query_row(&format!("SELECT {COLS} FROM incidents WHERE id=?1"), [id], row_incident).optional()
}

pub fn open_by_key(conn: &Connection, source: &str, key: &str) -> DbResult<Option<Incident>> {
    conn.query_row(
        &format!("SELECT {COLS} FROM incidents WHERE source=?1 AND ext_key=?2 AND status!='resolved' AND status!='discarded' LIMIT 1"),
        params![source, key],
        row_incident,
    )
    .optional()
}

/// Most recent incident with this source and external key, whatever its state.
pub fn latest_by_key(conn: &Connection, source: &str, key: &str) -> DbResult<Option<Incident>> {
    conn.query_row(
        &format!("SELECT {COLS} FROM incidents WHERE source=?1 AND ext_key=?2 ORDER BY created_at DESC LIMIT 1"),
        params![source, key],
        row_incident,
    )
    .optional()
}

/// Incidents from one source that are not resolved yet (active or draft).
pub fn open_by_source(conn: &Connection, source: &str) -> DbResult<Vec<Incident>> {
    let mut st = conn.prepare(&format!(
        "SELECT {COLS} FROM incidents WHERE source=?1 AND status!='resolved' AND status!='discarded'"
    ))?;
    let rows = st.query_map([source], row_incident)?;
    rows.collect()
}

// ---- desktops seen ------------------------------------------------------------

/// Upserts (computer, user, last_seen) entries collected since the last flush.
/// Upserts (computer, user, client version, last_seen) entries collected since the last flush.
/// An empty version (clients before 0.5.1 do not send one) keeps the stored value.
pub fn record_clients(conn: &Connection, entries: &[(String, String, String, i64)]) -> DbResult<()> {
    let mut st = conn.prepare(
        "INSERT INTO clients(computer,user,version,last_seen) VALUES (?1,?2,?3,?4)
         ON CONFLICT(computer) DO UPDATE SET user=excluded.user,
           version=CASE WHEN excluded.version='' THEN version ELSE excluded.version END,
           last_seen=MAX(last_seen, excluded.last_seen)",
    )?;
    for (computer, user, version, seen) in entries {
        st.execute(params![computer, user, version, seen])?;
    }
    Ok(())
}

#[derive(Serialize, Clone, Debug)]
pub struct Computer {
    pub computer: String,
    pub user: String,
    pub version: String,
    pub last_seen: i64,
}

/// Computers seen since `since`, most recent first.
pub fn computers(conn: &Connection, since: i64) -> DbResult<Vec<Computer>> {
    let mut st = conn.prepare(
        "SELECT computer,user,version,last_seen FROM clients WHERE last_seen>=?1 ORDER BY last_seen DESC LIMIT 5000",
    )?;
    let rows = st.query_map([since], |r| {
        Ok(Computer { computer: r.get(0)?, user: r.get(1)?, version: r.get(2)?, last_seen: r.get(3)? })
    })?;
    rows.collect()
}

// ---- notification groups ---------------------------------------------------------

/// A named set of recipients, addressed as `team:<name>` (also from a hub).
#[derive(Serialize, Clone, Debug)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub targets: Vec<String>,
}

pub fn teams(conn: &Connection) -> DbResult<Vec<Team>> {
    let mut st = conn.prepare("SELECT id,name,targets FROM teams ORDER BY name COLLATE NOCASE")?;
    let rows = st.query_map([], |r| Ok(Team { id: r.get(0)?, name: r.get(1)?, targets: list_json(r.get(2)?) }))?;
    rows.collect()
}
pub fn team_name_taken(conn: &Connection, name: &str, except: &str) -> DbResult<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM teams WHERE name=?1 AND id!=?2)", params![name, except], |r| r.get(0))
}
pub fn save_team(conn: &Connection, t: &Team) -> DbResult<()> {
    conn.execute(
        "INSERT INTO teams(id,name,targets,created_at) VALUES (?1,?2,?3,?4)
         ON CONFLICT(id) DO UPDATE SET name=excluded.name, targets=excluded.targets",
        params![t.id, t.name, to_json(&t.targets), now()],
    )?;
    Ok(())
}
pub fn delete_team(conn: &Connection, id: &str) -> DbResult<usize> {
    conn.execute("DELETE FROM teams WHERE id=?1", [id])
}

// ---- integrations ----------------------------------------------------------

fn row_integration(r: &Row) -> rusqlite::Result<Integration> {
    Ok(Integration {
        id: r.get(0)?,
        name: r.get(1)?,
        unmatched: r.get(2)?,
        delay_seconds: r.get(3)?,
        targets: list_json(r.get(4)?),
        created_at: r.get(5)?,
        last_seen: r.get(6)?,
        events: r.get(7)?,
    })
}
const INTEGRATION_COLS: &str = "id,name,unmatched,delay_seconds,targets,created_at,last_seen,events";

pub fn integrations(conn: &Connection) -> DbResult<Vec<Integration>> {
    let mut st = conn.prepare(&format!("SELECT {INTEGRATION_COLS} FROM integrations ORDER BY name COLLATE NOCASE"))?;
    let rows = st.query_map([], row_integration)?;
    rows.collect()
}
pub fn integration_by_hash(conn: &Connection, hash: &str) -> DbResult<Option<Integration>> {
    conn.query_row(&format!("SELECT {INTEGRATION_COLS} FROM integrations WHERE token_hash=?1"), [hash], row_integration)
        .optional()
}
pub fn insert_integration(conn: &Connection, i: &Integration, token_hash: &str) -> DbResult<()> {
    conn.execute(
        "INSERT INTO integrations(id,name,token_hash,unmatched,delay_seconds,targets,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![i.id, i.name, token_hash, i.unmatched, i.delay_seconds, to_json(&i.targets), i.created_at],
    )?;
    Ok(())
}
pub fn delete_integration(conn: &Connection, id: &str) -> DbResult<usize> {
    conn.execute("DELETE FROM integrations WHERE id=?1", [id])
}
pub fn touch_integration(conn: &Connection, id: &str, events: i64) -> DbResult<()> {
    conn.execute("UPDATE integrations SET last_seen=?1, events=events+?2 WHERE id=?3", params![now(), events, id])?;
    Ok(())
}

// ---- rules -----------------------------------------------------------------

pub fn rules(conn: &Connection) -> DbResult<Vec<Rule>> {
    let mut st = conn.prepare("SELECT id,pattern,title,message,severity,targets FROM rules ORDER BY created_at, id")?;
    let rows = st.query_map([], |r| {
        Ok(Rule {
            id: r.get(0)?,
            pattern: r.get(1)?,
            title: r.get(2)?,
            message: r.get(3)?,
            severity: r.get(4)?,
            targets: list_json(r.get(5)?),
        })
    })?;
    rows.collect()
}
pub fn insert_rule(conn: &Connection, r: &Rule) -> DbResult<()> {
    conn.execute(
        "INSERT INTO rules(id,pattern,title,message,severity,targets,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![r.id, r.pattern, r.title, r.message, r.severity, to_json(&r.targets), now()],
    )?;
    Ok(())
}
pub fn delete_rule(conn: &Connection, id: &str) -> DbResult<usize> {
    conn.execute("DELETE FROM rules WHERE id=?1", [id])
}
