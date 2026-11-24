//! Silent alarm: a user presses a hotkey on a desktop and the server raises a message for
//! everyone or for chosen recipients. The desktop that triggered it shows nothing.
//!
//! Desktops call `POST /api/client/alarm` with the client token; the message is an ordinary
//! incident (source `alarm`, external key `<computer>/<user>`), so targeting, delivery, history
//! and resolving work as for every other message. Abuse and false alarms are damped by the
//! settings (off by default), a per-desktop cooldown, a global limit and by only accepting
//! computers that have connected before.

use crate::store::{self, now, Incident};
use crate::{bad, bearer, check_targets, clean, internal, refresh, same, Api, App, HttpError};
use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::json;
use tracing::{info, warn};

/// Source of alarm messages.
pub const SOURCE: &str = "alarm";
/// At most this many alarms in [`BURST_WINDOW`] seconds, across all desktops.
pub const BURST_LIMIT: i64 = 5;
pub const BURST_WINDOW: i64 = 600;

/// Normalises a hotkey such as `strg+umschalt+ü` to `Ctrl+Shift+Ü`.
///
/// Modifiers: Ctrl (Strg, Control), Alt, Shift (Umschalt), Win. The key is F1–F24 or one
/// character of the keyboard layout. Plain characters need two modifiers and F-keys one,
/// and at least one modifier must be Ctrl, Alt or Win, so normal typing and shortcuts such
/// as Ctrl+C stay untouched.
pub fn parse_hotkey(raw: &str) -> Option<String> {
    let (mut ctrl, mut alt, mut shift, mut win) = (false, false, false, false);
    let mut key: Option<String> = None;
    for part in raw.split('+').map(str::trim) {
        let lower = part.to_lowercase();
        let flag = match lower.as_str() {
            "ctrl" | "strg" | "control" => &mut ctrl,
            "alt" => &mut alt,
            "shift" | "umschalt" => &mut shift,
            "win" | "windows" => &mut win,
            _ => {
                if key.is_some() {
                    return None;
                }
                let mut chars = part.chars();
                key = match (chars.next(), chars.next()) {
                    (Some(c), None) if !c.is_whitespace() && !c.is_control() => Some(c.to_uppercase().collect()),
                    _ => {
                        let n: u8 = lower.strip_prefix('f')?.parse().ok()?;
                        (1..=24).contains(&n).then(|| format!("F{n}"))
                    }
                };
                key.as_ref()?;
                continue;
            }
        };
        if *flag {
            return None;
        }
        *flag = true;
    }
    let key = key?;
    let mods = [ctrl, alt, shift, win].iter().filter(|m| **m).count();
    let function = key.chars().count() > 1;
    if !(ctrl || alt || win) || mods < if function { 1 } else { 2 } {
        return None;
    }
    let mut out: Vec<&str> = Vec::new();
    for (on, name) in [(ctrl, "Ctrl"), (alt, "Alt"), (shift, "Shift"), (win, "Win")] {
        if on {
            out.push(name);
        }
    }
    out.push(&key);
    Some(out.join("+"))
}

/// Fills `{user}` and `{computer}` into a configured text.
pub fn fill(template: &str, user: &str, computer: &str) -> String {
    template.replace("{user}", user).replace("{computer}", computer)
}

/// The external key that ties an alarm to the desktop session that raised it.
pub fn origin(user: &str, computer: &str) -> String {
    format!("{}/{}", computer.trim().to_lowercase(), user.rsplit('\\').next().unwrap_or("").trim().to_lowercase())
}

/// An alarm is never shown on the computer that raised it, whoever is signed in there and
/// even when that computer is among the recipients: that is what makes it silent.
pub fn hidden_for(i: &Incident, computer: &str) -> bool {
    let here = format!("{}/", computer.trim().to_lowercase());
    i.source == SOURCE && i.ext_key.as_deref().is_some_and(|key| key.starts_with(&here))
}

#[derive(Deserialize)]
pub struct Trigger {
    #[serde(default)]
    user: String,
    #[serde(default)]
    computer: String,
}

/// What happened to one trigger; also returned to the desktop, which only logs it.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    Created(Box<Incident>),
    /// An alarm from this session is still open.
    Open,
    /// Within the cooldown of the previous alarm from this session.
    Cooldown,
    /// Too many alarms on this server in a short time.
    Limited,
}

/// Decides and, when allowed, stores the alarm. Settings and checks are taken as arguments
/// so that the rules can be tested without a running server.
pub fn raise(conn: &rusqlite::Connection, s: &store::Settings, user: &str, computer: &str) -> store::DbResult<Outcome> {
    let key = origin(user, computer);
    let t = now();
    if let Some(last) = store::latest_by_key(conn, SOURCE, &key)? {
        if last.status == "active" || last.status == "draft" {
            return Ok(Outcome::Open);
        }
        if last.created_at > t - s.alarm_cooldown_seconds {
            return Ok(Outcome::Cooldown);
        }
    }
    if store::count_created_since(conn, SOURCE, t - BURST_WINDOW)? >= BURST_LIMIT {
        return Ok(Outcome::Limited);
    }
    let shown_computer = computer.to_uppercase();
    let incident = Incident {
        id: uuid::Uuid::new_v4().to_string(),
        title: clean(&fill(&s.alarm_title, user, &shown_computer), 120).replace('\n', " "),
        message: clean(&fill(&s.alarm_message, user, &shown_computer), 1500),
        severity: SOURCE.into(),
        targets: s.alarm_targets.clone(),
        status: "active".into(),
        source: SOURCE.into(),
        ext_key: Some(key),
        created_at: t,
        starts_at: t,
        ends_at: (s.alarm_minutes > 0).then(|| t + s.alarm_minutes * 60),
        resolved_at: None,
        created_by: format!("{SOURCE}:{user}@{shown_computer}"),
        approved_by: None,
        resolved_by: None,
    };
    store::insert_incident(conn, &incident)?;
    Ok(Outcome::Created(Box::new(incident)))
}

/// Settings checks that need the running server (recipients must exist).
pub fn check_settings(app: &App, s: &mut store::Settings) -> Result<(), HttpError> {
    s.alarm_targets = check_targets(app, &s.alarm_targets)?;
    Ok(())
}

pub async fn trigger(State(app): State<App>, headers: HeaderMap, Json(form): Json<Trigger>) -> Api {
    if !same(bearer(&headers), &app.cfg.client_token) {
        return Err((StatusCode::UNAUTHORIZED, "Client token required".into()));
    }
    let s = app.settings.read().unwrap().clone();
    if !s.alarm_enabled {
        return Err((StatusCode::NOT_FOUND, "Silent alarm is off".into()));
    }
    let user = clean(form.user.rsplit('\\').next().unwrap_or(""), 64).replace('\n', "");
    let computer = clean(&form.computer, 64).replace('\n', "");
    if user.is_empty() || computer.is_empty() {
        return Err(bad("user and computer required"));
    }
    // Where the request came from, for the log (the server usually sits behind a proxy).
    let via = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|v| clean(v, 100))
        .unwrap_or_else(|| "-".into());
    // Only desktops that have connected before: the client token alone, used from a script,
    // cannot raise alarms for invented computers.
    let lower = computer.to_lowercase();
    let known = app.seen.lock().unwrap().contains_key(&lower) || {
        store::client_seen_since(&app.db.lock().unwrap(), &lower, now() - crate::uplink::ACTIVE_WINDOW)
    }
    .map_err(internal)?;
    if !known {
        warn!(user = %user, computer = %computer, via = %via, "silent alarm from an unknown computer refused");
        return Err((StatusCode::FORBIDDEN, "Unknown computer".into()));
    }
    let outcome = { raise(&app.db.lock().unwrap(), &s, &user, &computer) }.map_err(internal)?;
    let result = match &outcome {
        Outcome::Created(i) => {
            warn!(user = %user, computer = %computer, via = %via, id = %i.id, "SILENT ALARM raised");
            refresh(&app);
            app.uplink_wake.notify_one();
            "created"
        }
        Outcome::Open => {
            info!(user = %user, computer = %computer, via = %via, "silent alarm repeated while still open");
            "open"
        }
        Outcome::Cooldown => {
            info!(user = %user, computer = %computer, via = %via, "silent alarm within cooldown, ignored");
            "cooldown"
        }
        Outcome::Limited => {
            warn!(user = %user, computer = %computer, via = %via, "silent alarm refused: too many alarms in 10 minutes");
            "limited"
        }
    };
    Ok(Json(json!({"ok": true, "result": result})))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys_are_normalised_and_guarded() {
        assert_eq!(parse_hotkey("ctrl+shift+f12").as_deref(), Some("Ctrl+Shift+F12"));
        assert_eq!(parse_hotkey("Strg + Umschalt + ü").as_deref(), Some("Ctrl+Shift+Ü"));
        assert_eq!(parse_hotkey("Shift+Alt+Ctrl+x").as_deref(), Some("Ctrl+Alt+Shift+X"));
        assert_eq!(parse_hotkey("Win+F9").as_deref(), Some("Win+F9"));
        assert_eq!(parse_hotkey("Ctrl+C"), None, "one modifier is not enough for a character");
        assert_eq!(parse_hotkey("Shift+F12"), None, "needs Ctrl, Alt or Win");
        assert_eq!(parse_hotkey("Ctrl+Shift"), None, "needs a key");
        assert_eq!(parse_hotkey("Ctrl+Shift+A+B"), None);
        assert_eq!(parse_hotkey("Ctrl+Ctrl+A"), None);
        assert_eq!(parse_hotkey("Ctrl+F25"), None);
        assert_eq!(parse_hotkey(""), None);
    }

    #[test]
    fn templates_and_origin() {
        assert_eq!(fill("Help: {user} at {computer}", "anna", "PC-07"), "Help: anna at PC-07");
        assert_eq!(origin("CORP\\Anna", " PC-07 "), "pc-07/anna");
    }

    fn settings() -> store::Settings {
        store::Settings {
            alarm_enabled: true,
            alarm_title: "Help needed: {user} at {computer}".into(),
            alarm_targets: vec!["host:PC-RECEPTION-*".into()],
            ..store::Settings::default()
        }
    }

    #[test]
    fn alarm_is_raised_once_then_damped() {
        let conn = store::open(":memory:").unwrap();
        let s = settings();
        let Outcome::Created(i) = raise(&conn, &s, "anna", "pc-07").unwrap() else { panic!("not created") };
        assert_eq!(i.title, "Help needed: anna at PC-07");
        assert_eq!(i.targets, vec!["host:PC-RECEPTION-*".to_string()]);
        assert_eq!(i.created_by, "alarm:anna@PC-07");
        assert_eq!(i.severity, "alarm", "its own kind, colour and sound");
        assert!(hidden_for(&i, "PC-07"), "silent on the computer where it was raised");
        assert!(!hidden_for(&i, "PC-070"), "other computers see it");
        assert_eq!(raise(&conn, &s, "anna", "pc-07").unwrap(), Outcome::Open);
        store::resolve_incident(&conn, &i.id, "admin").unwrap();
        assert_eq!(raise(&conn, &s, "anna", "pc-07").unwrap(), Outcome::Cooldown);
        let none = store::Settings { alarm_cooldown_seconds: 0, ..s.clone() };
        assert!(matches!(raise(&conn, &none, "anna", "pc-07").unwrap(), Outcome::Created(_)));
    }

    #[test]
    fn many_alarms_hit_the_global_limit() {
        let conn = store::open(":memory:").unwrap();
        let s = settings();
        for n in 0..BURST_LIMIT {
            assert!(matches!(raise(&conn, &s, "u", &format!("pc-{n}")).unwrap(), Outcome::Created(_)));
        }
        assert_eq!(raise(&conn, &s, "u", "pc-99").unwrap(), Outcome::Limited);
    }

    #[test]
    fn automatic_end() {
        let conn = store::open(":memory:").unwrap();
        let s = store::Settings { alarm_minutes: 15, ..settings() };
        let Outcome::Created(i) = raise(&conn, &s, "anna", "pc-07").unwrap() else { panic!("not created") };
        assert_eq!(i.ends_at, Some(i.created_at + 900));
    }
}
