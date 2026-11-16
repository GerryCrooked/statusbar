//! Monitoring webhooks: turn whatever a monitoring system sends into
//! problem/recovery events, then into user-facing incidents.
//!
//! Understood natively (auto-detected by payload shape):
//!   * generic JSON  {key,status,host,service,title,message,severity}
//!     (used by the bundled Zabbix, Checkmk, Nagios and Icinga scripts)
//!   * Prometheus Alertmanager / Grafana  {alerts:[{status,labels,annotations,fingerprint}]}
//!   * Uptime Kuma  {heartbeat:{status},monitor:{id,name},msg}

use crate::glob;
use crate::store::{self, DbResult, Incident, Integration, Rule};
use rusqlite::Connection;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub key: String,
    pub problem: bool,
    pub title: String,
    pub message: String,
    pub severity: String,
    pub host: String,
    pub service: String,
}

fn text(v: &Value, names: &[&str]) -> String {
    for name in names {
        match v.get(*name) {
            Some(Value::String(s)) if !s.trim().is_empty() => return s.trim().to_string(),
            Some(Value::Number(n)) => return n.to_string(),
            _ => {}
        }
    }
    String::new()
}

/// Some(true) = problem, Some(false) = recovery, None = not a state change we act on.
fn state(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "problem" | "down" | "critical" | "crit" | "warning" | "warn" | "firing" | "error" | "unknown"
        | "unreachable" | "fail" | "failed" | "failure" | "alerting" | "1" => Some(true),
        "ok" | "up" | "resolved" | "recovery" | "recovered" | "resolve" | "normal" | "0" => Some(false),
        _ => None,
    }
}

fn severity(raw: &str, fallback: &str) -> String {
    let level = match raw.trim().to_ascii_lowercase().as_str() {
        "info" | "information" | "informational" | "not classified" | "none" | "notice" => "info",
        "warning" | "warn" | "average" | "minor" | "degraded" => "warning",
        "error" | "critical" | "crit" | "high" | "disaster" | "major" | "page" | "down" | "unreachable" | "unknown" => {
            "error"
        }
        _ => fallback,
    };
    level.to_string()
}

fn clip(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control() || *c == '\n').take(max).collect()
}

fn generic(v: &Value) -> Option<Event> {
    let status = text(v, &["status", "state", "type"]);
    let problem = state(&status)?;
    let host = text(v, &["host", "hostname", "instance"]);
    let service = text(v, &["service", "check", "trigger"]);
    let mut key = text(v, &["key", "id", "event_id"]);
    if key.is_empty() && !(host.is_empty() && service.is_empty()) {
        key = format!("{host}/{service}");
    }
    let mut title = text(v, &["title", "summary", "subject", "name"]);
    if key.is_empty() {
        key = title.clone();
    }
    if key.is_empty() {
        return None;
    }
    if title.is_empty() {
        title = match (service.is_empty(), host.is_empty()) {
            (false, false) => format!("{service} ({host})"),
            (false, true) => service.clone(),
            _ => host.clone(),
        };
    }
    let fallback = if matches!(status.to_ascii_lowercase().as_str(), "warning" | "warn") { "warning" } else { "error" };
    Some(Event {
        key,
        problem,
        title,
        message: text(v, &["message", "description", "output", "text", "msg"]),
        severity: severity(&text(v, &["severity", "priority"]), fallback),
        host,
        service,
    })
}

fn alertmanager(alert: &Value) -> Option<Event> {
    let labels = alert.get("labels").cloned().unwrap_or(Value::Null);
    let notes = alert.get("annotations").cloned().unwrap_or(Value::Null);
    let problem = state(&text(alert, &["status"]))?;
    let name = text(&labels, &["alertname", "rulename"]);
    let host = text(&labels, &["instance", "host", "hostname", "node"]);
    let service = text(&labels, &["service", "job"]);
    let mut key = text(alert, &["fingerprint"]);
    if key.is_empty() {
        key = format!("{name}/{host}/{service}");
    }
    let mut title = text(&notes, &["summary", "title"]);
    if title.is_empty() {
        title = if host.is_empty() { name.clone() } else { format!("{name} ({host})") };
    }
    Some(Event {
        key,
        problem,
        title,
        message: text(&notes, &["description", "message"]),
        severity: severity(&text(&labels, &["severity", "priority"]), "error"),
        host,
        service: if service.is_empty() { name } else { service },
    })
}

fn uptime_kuma(v: &Value) -> Option<Event> {
    let monitor = v.get("monitor")?;
    let beat = v.get("heartbeat")?;
    let problem = match beat.get("status").and_then(Value::as_i64)? {
        0 => true,
        1 => false,
        _ => return None, // pending / maintenance
    };
    let name = text(monitor, &["name", "pathName"]);
    let id = text(monitor, &["id"]);
    Some(Event {
        key: format!("kuma-{}", if id.is_empty() { name.clone() } else { id }),
        problem,
        title: name.clone(),
        message: text(beat, &["msg"]),
        severity: "error".into(),
        host: name,
        service: text(monitor, &["type"]),
    })
}

pub fn parse(v: &Value) -> Vec<Event> {
    let mut events: Vec<Event> = if let Some(alerts) = v.get("alerts").and_then(Value::as_array) {
        alerts.iter().filter_map(alertmanager).collect()
    } else if v.get("heartbeat").is_some_and(Value::is_object) {
        uptime_kuma(v).into_iter().collect()
    } else if let Some(list) = v.as_array() {
        list.iter().filter_map(generic).collect()
    } else {
        generic(v).into_iter().collect()
    };
    for e in &mut events {
        e.key = clip(&e.key, 300);
        e.title = clip(&e.title, 120);
        e.message = clip(&e.message, 1500);
        e.host = clip(&e.host, 200);
        e.service = clip(&e.service, 200);
    }
    events.truncate(200);
    events
}

pub fn find_rule<'a>(rules: &'a [Rule], e: &Event) -> Option<&'a Rule> {
    let subject = format!("{}/{}", e.host, e.service);
    rules.iter().find(|r| {
        glob::matches(&r.pattern, &subject) || glob::matches(&r.pattern, &e.key) || glob::matches(&r.pattern, &e.title)
    })
}

fn render(template: &str, e: &Event) -> String {
    template
        .replace("{host}", &e.host)
        .replace("{service}", &e.service)
        .replace("{title}", &e.title)
        .replace("{message}", &e.message)
}

/// Applies one event. Returns what happened, for the webhook response and the log.
pub fn apply(conn: &Connection, integration: &Integration, rules: &[Rule], e: &Event) -> DbResult<&'static str> {
    let now = store::now();
    let open = store::open_by_key(conn, &integration.id, &e.key)?;
    if !e.problem {
        return Ok(match open {
            None => "ignored",
            // Never reached a screen (draft, or still inside the delay window): drop silently.
            Some(i) if i.status == "draft" || i.starts_at > now => {
                store::delete_incident(conn, &i.id)?;
                "cancelled"
            }
            Some(i) => {
                store::resolve_incident(conn, &i.id, &format!("monitoring:{}", integration.name))?;
                "resolved"
            }
        });
    }
    if open.is_some() {
        return Ok("duplicate");
    }
    let rule = find_rule(rules, e);
    let (status, title, message, severity, targets) = match rule {
        Some(r) => (
            "active",
            clip(&render(&r.title, e), 120),
            clip(&render(&r.message, e), 1500),
            if r.severity.is_empty() { e.severity.clone() } else { r.severity.clone() },
            r.targets.clone(),
        ),
        None => match integration.unmatched.as_str() {
            "ignore" => return Ok("ignored"),
            "publish" => {
                ("active", e.title.clone(), e.message.clone(), e.severity.clone(), integration.targets.clone())
            }
            _ => ("draft", e.title.clone(), e.message.clone(), e.severity.clone(), integration.targets.clone()),
        },
    };
    let message = if message.trim().is_empty() { title.clone() } else { message };
    store::insert_incident(
        conn,
        &Incident {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            message,
            severity,
            targets,
            status: status.into(),
            source: integration.id.clone(),
            ext_key: Some(e.key.clone()),
            created_at: now,
            starts_at: now + integration.delay_seconds,
            ends_at: None,
            resolved_at: None,
            created_by: format!("monitoring:{}", integration.name),
            approved_by: None,
            resolved_by: None,
        },
    )?;
    Ok(if status == "draft" { "draft" } else { "created" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn generic_problem_and_recovery() {
        let p = parse(&json!({"status":"CRIT","host":"mail01","service":"SMTP","output":"timeout"}));
        assert_eq!(p.len(), 1);
        assert!(p[0].problem);
        assert_eq!(p[0].key, "mail01/SMTP");
        assert_eq!(p[0].title, "SMTP (mail01)");
        assert_eq!(p[0].severity, "error");
        let r = parse(&json!({"status":"OK","host":"mail01","service":"SMTP"}));
        assert!(!r[0].problem);
        assert_eq!(r[0].key, p[0].key);
    }

    #[test]
    fn warning_state_maps_to_warning() {
        let p = parse(&json!({"status":"WARN","host":"fs01","service":"Disk"}));
        assert_eq!(p[0].severity, "warning");
    }

    #[test]
    fn acknowledgements_are_ignored() {
        assert!(parse(&json!({"status":"ACKNOWLEDGEMENT","host":"a","service":"b"})).is_empty());
        assert!(parse(&json!({"msg":"test"})).is_empty());
    }

    #[test]
    fn alertmanager_batch() {
        let p = parse(&json!({"alerts":[
            {"status":"firing","fingerprint":"abc","labels":{"alertname":"HostDown","instance":"db1:9100","severity":"critical"},
             "annotations":{"summary":"db1 is down"}},
            {"status":"resolved","fingerprint":"def","labels":{"alertname":"DiskFull","severity":"warning"}}
        ]}));
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].key.as_str(), p[0].problem, p[0].title.as_str()), ("abc", true, "db1 is down"));
        assert_eq!((p[1].problem, p[1].severity.as_str()), (false, "warning"));
    }

    #[test]
    fn uptime_kuma_states() {
        let down = parse(&json!({"heartbeat":{"status":0,"msg":"timeout"},"monitor":{"id":7,"name":"Intranet"}}));
        assert_eq!((down[0].key.as_str(), down[0].problem), ("kuma-7", true));
        let up = parse(&json!({"heartbeat":{"status":1},"monitor":{"id":7,"name":"Intranet"}}));
        assert!(!up[0].problem);
        assert!(parse(&json!({"heartbeat":null,"monitor":null,"msg":"Test"})).is_empty());
    }

    #[test]
    fn rules_match_host_service() {
        let rules = vec![Rule {
            id: "1".into(),
            pattern: "mail*/*".into(),
            title: "E-mail is disrupted".into(),
            message: "We are working on it ({host}).".into(),
            severity: String::new(),
            targets: vec![],
        }];
        let e = parse(&json!({"status":"down","host":"MAIL01","service":"SMTP"})).remove(0);
        let r = find_rule(&rules, &e).expect("rule");
        assert_eq!(render(&r.message, &e), "We are working on it (MAIL01).");
        let other = parse(&json!({"status":"down","host":"db1","service":"SMTP"})).remove(0);
        assert!(find_rule(&rules, &other).is_none());
    }
}
