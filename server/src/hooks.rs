//! Monitoring webhooks: turn whatever a monitoring system sends into
//! problem/recovery events, then into user-facing incidents.
//!
//! Understood natively (auto-detected by payload shape):
//!   * generic JSON  {key,status,host,service,title,message,severity}
//!     (used by the bundled Zabbix, Checkmk, Nagios and Icinga scripts)
//!   * Prometheus Alertmanager / Grafana  {alerts:[{status,labels,annotations,fingerprint}]}
//!   * Uptime Kuma  {heartbeat:{status},monitor:{id,name},msg}
//!   * Azure Monitor common alert schema  {schemaId,data:{essentials,alertContext}}
//!   * NinjaOne activities  {activityType:"CONDITION",statusCode:"TRIGGERED"|"RESET",...}
//!   * GLPI ticket webhooks (default payload)  {event,item:{id,name,status:{id},priority,...}}
//!   * N-able N-central "Third Party Integration – HTTP" (XML)  <QualitativeNewState>, <DeviceName> …
//!     – flat XML bodies are turned into a JSON object by [`xml_fields`] first

use crate::glob;
use crate::store::{self, DbResult, Incident, Integration, Rule};
use rusqlite::Connection;
use serde_json::Value;

/// Outcome for events about tickets that Statusbar opened itself (see ticketing::own_ticket).
pub const OWN_TICKET: &str = "own-ticket";

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
        "info" | "information" | "informational" | "not classified" | "none" | "notice" | "low" | "lowest" => "info",
        "warning" | "warn" | "average" | "minor" | "moderate" | "medium" | "degraded" => "warning",
        "error" | "critical" | "crit" | "highest" | "high" | "disaster" | "major" | "page" | "down" | "unreachable"
        | "unknown" => "error",
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

/// Last path segment of an Azure resource ID (`/subscriptions/…/virtualMachines/vm1` → `vm1`).
fn resource_name(id: &str) -> String {
    id.trim_end_matches('/').rsplit('/').next().unwrap_or_default().to_string()
}

/// Rough HTML-to-text for Azure Service Health communications.
fn strip_html(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn azure(v: &Value) -> Option<Event> {
    let data = v.get("data")?;
    let ess = data.get("essentials")?;
    let ctx = data.get("alertContext").cloned().unwrap_or(Value::Null);
    let props = ctx.get("properties").cloned().unwrap_or(Value::Null);
    let rule = text(ess, &["alertRule"]);
    let first = |field: &str| -> String {
        ess.get(field)
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let mut host = first("configurationItems");
    if host.is_empty() {
        host = resource_name(&first("alertTargetIDs"));
    }
    match text(ess, &["monitoringService"]).as_str() {
        // Activity log alerts are stateless: every update arrives as "Fired". Service and resource
        // health carry their real state in the alert context.
        "ServiceHealth" => {
            let stage = text(&props, &["stage"]).to_ascii_lowercase();
            let problem =
                !matches!(stage.as_str(), "resolved" | "rca" | "complete" | "completed" | "cancelled" | "canceled")
                    && !text(&ctx, &["status"]).eq_ignore_ascii_case("resolved");
            let id = text(&props, &["trackingId"]);
            let service = text(&props, &["service"]);
            let severity =
                if text(&props, &["incidentType"]).eq_ignore_ascii_case("incident") { "warning" } else { "info" };
            let mut title = text(&props, &["defaultLanguageTitle", "title"]);
            if title.is_empty() {
                title = rule.clone();
            }
            Some(Event {
                key: format!("azure-sh-{}", if id.is_empty() { &title } else { &id }),
                problem,
                title,
                message: strip_html(&text(&props, &["defaultLanguageContent", "communication"])),
                severity: severity.into(),
                host: text(&props, &["region"]),
                service,
            })
        }
        "ResourceHealth" => {
            let health = text(&props, &["currentHealthStatus"]).to_ascii_lowercase();
            let problem = match health.as_str() {
                "available" => false,
                "unavailable" | "degraded" => true,
                _ => return None,
            };
            let target = first("alertTargetIDs");
            Some(Event {
                key: format!("azure-rh-{}", if target.is_empty() { &host } else { &target }),
                problem,
                title: if host.is_empty() { rule.clone() } else { format!("{host} is {health}") },
                message: text(&props, &["title"]),
                severity: if health == "degraded" { "warning" } else { "error" }.into(),
                host,
                service: if rule.is_empty() { "Resource health".into() } else { rule },
            })
        }
        s if s.starts_with("Activity Log") => None,
        _ => {
            let problem = match text(ess, &["monitorCondition"]).to_ascii_lowercase().as_str() {
                "fired" => true,
                "resolved" => false,
                _ => return None,
            };
            let severity = match text(ess, &["severity"]).to_ascii_lowercase().as_str() {
                "sev0" | "sev1" => "error",
                "sev2" => "warning",
                "sev3" | "sev4" => "info",
                _ => "error",
            };
            let summary = ctx.get("annotations").map(|a| text(a, &["summary"])).unwrap_or_default();
            let mut key = text(ess, &["alertId", "originAlertId"]);
            if key.is_empty() {
                key = format!("{rule}/{host}");
            }
            let title = if !summary.is_empty() {
                summary
            } else if host.is_empty() {
                rule.clone()
            } else {
                format!("{rule} ({host})")
            };
            Some(Event {
                key,
                problem,
                title,
                message: text(ess, &["description"]),
                severity: severity.into(),
                host,
                service: rule,
            })
        }
    }
}

fn ninjaone(v: &Value) -> Option<Event> {
    if !text(v, &["activityType"]).eq_ignore_ascii_case("CONDITION") {
        return None;
    }
    let problem = match text(v, &["statusCode"]).to_ascii_uppercase().as_str() {
        "TRIGGERED" => true,
        "RESET" => false,
        _ => return None,
    };
    let device = v.get("device").cloned().unwrap_or(Value::Null);
    let device_id = text(v, &["deviceId"]);
    let mut host = text(&device, &["displayName", "systemName", "dnsName"]);
    if host.is_empty() {
        host = device_id.clone();
    }
    // A condition on a device fires and resets with the same policy element (sourceConfigUid).
    let condition = text(v, &["sourceConfigUid", "seriesUid"]);
    let key =
        if condition.is_empty() { text(v, &["seriesUid", "id"]) } else { format!("ninja-{device_id}-{condition}") };
    if key.is_empty() {
        return None;
    }
    let service = text(v, &["sourceName"]);
    let mut title = text(v, &["subject"]);
    if title.is_empty() {
        title = match (service.is_empty(), host.is_empty()) {
            (false, false) => format!("{service} ({host})"),
            (false, true) => service.clone(),
            _ => host.clone(),
        };
    }
    Some(Event {
        key,
        problem,
        title,
        message: text(v, &["message"]),
        severity: severity(&text(v, &["severity", "priority"]), "error"),
        host,
        service,
    })
}

fn xml_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let c = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if entity.starts_with("#x") => u32::from_str_radix(&entity[2..], 16).ok().and_then(char::from_u32),
            _ if entity.starts_with('#') => entity[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Flat XML (`<Name>text</Name>` elements, any nesting ignored) as a JSON object of strings, for
/// tools that can only send XML. `None` if the body is not XML or has no such elements.
pub fn xml_fields(body: &[u8]) -> Option<Value> {
    let text = std::str::from_utf8(body).ok()?.trim_start_matches('\u{feff}').trim();
    if !text.starts_with('<') {
        return None;
    }
    let mut map = serde_json::Map::new();
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find('>') else { break };
        let tag = &rest[..close];
        rest = &rest[close + 1..];
        if tag.starts_with(['?', '!', '/']) || tag.ends_with('/') {
            continue;
        }
        let name = tag.split_whitespace().next().unwrap_or("");
        let Some(end) = rest.find('<') else { break };
        let value = &rest[..end];
        if rest[end..].starts_with(&format!("</{name}>")) && !name.is_empty() {
            let value = value.trim();
            let value = value.strip_prefix("<![CDATA[").and_then(|v| v.strip_suffix("]]>")).unwrap_or(value);
            map.entry(name.to_string()).or_insert_with(|| Value::String(xml_unescape(value)));
        }
    }
    (!map.is_empty()).then_some(Value::Object(map))
}

/// N-able N-central notification (HTTP-XML), already flattened by [`xml_fields`].
fn ncentral(v: &Value) -> Option<Event> {
    let new_state = text(v, &["QualitativeNewState"]);
    let problem = match new_state.to_ascii_lowercase().as_str() {
        "failed" | "failure" | "warning" => true,
        "normal" => false,
        _ => return None, // Misconfigured, No Data, Disconnected, Stale …
    };
    let host = text(v, &["DeviceName"]);
    let service = text(v, &["AffectedService", "ServiceName"]);
    let customer = text(v, &["CustomerName"]);
    let message = [text(v, &["QuantitativeNewState"]), text(v, &["TaskIdent"])]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" – ");
    Some(Event {
        key: format!("ncentral-{customer}/{host}/{service}"),
        problem,
        title: if service.is_empty() { host.clone() } else { format!("{service} ({host})") },
        message,
        severity: if new_state.eq_ignore_ascii_case("warning") { "warning" } else { "error" }.into(),
        host,
        service,
    })
}

/// GLPI's default webhook payload for tickets: status 1–4 open, 5 solved, 6 closed.
fn glpi(v: &Value) -> Option<Event> {
    let item = v.get("item")?;
    let id = text(item, &["id"]);
    if id.is_empty() {
        return None;
    }
    let event = text(v, &["event"]).to_ascii_lowercase();
    let status = item.get("status").and_then(|s| s.get("id")).and_then(Value::as_i64)?;
    let deleted = item.get("is_deleted").and_then(Value::as_bool).unwrap_or(false);
    let problem = match status {
        _ if deleted || event == "delete" || event == "purge" => false,
        1..=4 => true,
        5 | 6 => false,
        _ => return None,
    };
    let name = |field: &str| item.get(field).map(|o| text(o, &["completename", "name"])).unwrap_or_default();
    let severity = match item.get("priority").and_then(Value::as_i64).unwrap_or(0) {
        5 | 6 => "error",
        3 | 4 => "warning",
        1 | 2 => "info",
        _ => "error",
    };
    Some(Event {
        key: format!("glpi-{id}"),
        problem,
        title: text(item, &["name"]),
        message: strip_html(&text(item, &["content"])),
        severity: severity.into(),
        host: name("entity"),
        service: name("category"),
    })
}

pub fn parse(v: &Value) -> Vec<Event> {
    let mut events: Vec<Event> = if let Some(alerts) = v.get("alerts").and_then(Value::as_array) {
        alerts.iter().filter_map(alertmanager).collect()
    } else if v.get("heartbeat").is_some_and(Value::is_object) {
        uptime_kuma(v).into_iter().collect()
    } else if v.get("data").and_then(|d| d.get("essentials")).is_some_and(Value::is_object) {
        azure(v).into_iter().collect()
    } else if v.get("activityType").is_some() && v.get("statusCode").is_some() {
        ninjaone(v).into_iter().collect()
    } else if v.get("event").is_some_and(Value::is_string) && v.get("item").is_some_and(Value::is_object) {
        glpi(v).into_iter().collect()
    } else if v.get("QualitativeNewState").is_some() {
        ncentral(v).into_iter().collect()
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
    // Loop protection: a ticket opened by Statusbar never becomes a second message.
    if crate::ticketing::own_ticket(conn, e)? {
        return Ok(OWN_TICKET);
    }
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

    fn fixture(path: &str) -> Value {
        let file = format!("{}/../integrations/{path}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(&file).expect(&file)).expect(&file)
    }

    #[test]
    fn azure_metric_alert() {
        let fired = parse(&fixture("azure-monitor/test-fired.json"));
        assert_eq!(fired.len(), 1);
        let e = &fired[0];
        assert!(e.problem);
        assert_eq!(
            (e.host.as_str(), e.service.as_str(), e.severity.as_str()),
            ("vm-intranet01", "Intranet VM CPU", "warning")
        );
        assert_eq!(e.title, "Intranet VM CPU (vm-intranet01)");
        assert_eq!(e.message, "The intranet server is overloaded.");
        let resolved = parse(&fixture("azure-monitor/test-resolved.json"));
        assert!(!resolved[0].problem);
        assert_eq!(resolved[0].key, e.key);
    }

    #[test]
    fn azure_service_health() {
        let active = parse(&fixture("azure-monitor/test-servicehealth-active.json"));
        let e = &active[0];
        assert!(e.problem);
        assert_eq!(e.key, "azure-sh-AB12-CD3");
        assert_eq!(e.title, "Azure SQL Database - West Europe - Connectivity issues");
        assert_eq!(
            (e.host.as_str(), e.service.as_str(), e.severity.as_str()),
            ("West Europe", "SQL Database", "warning")
        );
        assert!(e.message.starts_with("Summary of impact: Customers may"), "{}", e.message);
        let resolved = parse(&fixture("azure-monitor/test-servicehealth-resolved.json"));
        assert!(!resolved[0].problem);
        assert_eq!(resolved[0].key, e.key);
    }

    #[test]
    fn azure_other_alerts() {
        let resource = |health: &str| {
            parse(&json!({"schemaId":"azureMonitorCommonAlertSchema","data":{
                "essentials":{"alertRule":"VM health","monitoringService":"ResourceHealth","monitorCondition":"Fired",
                    "alertTargetIDs":["/subscriptions/x/resourceGroups/rg/providers/Microsoft.Compute/virtualMachines/vm1"]},
                "alertContext":{"properties":{"title":"Stopped","currentHealthStatus":health}}}}))
        };
        let down = resource("Unavailable");
        assert_eq!((down[0].problem, down[0].host.as_str(), down[0].severity.as_str()), (true, "vm1", "error"));
        let up = resource("Available");
        assert_eq!((up[0].problem, up[0].key == down[0].key), (false, true));
        assert!(resource("Unknown").is_empty());
        // Stateless activity log alerts never resolve, so they are ignored.
        assert!(parse(&json!({"data":{"essentials":{"monitoringService":"Activity Log - Administrative","monitorCondition":"Fired"},
            "alertContext":{}}})).is_empty());
        let prom = parse(&json!({"data":{"essentials":{"alertId":"p1","alertRule":"SqlDown","severity":"Sev0",
            "monitoringService":"Prometheus","monitorCondition":"Fired"},"alertContext":{"annotations":{"summary":"SQL is down"}}}}));
        assert_eq!((prom[0].title.as_str(), prom[0].severity.as_str()), ("SQL is down", "error"));
    }

    #[test]
    fn ninjaone_condition() {
        let t = parse(&fixture("ninjaone/test-triggered.json"));
        let e = &t[0];
        assert!(e.problem);
        assert_eq!(e.key, "ninja-42-3b7d9c2e-0000-4000-8000-000000000002");
        assert_eq!((e.title.as_str(), e.severity.as_str(), e.host.as_str()), ("Disk space below 10%", "error", "42"));
        let r = parse(&fixture("ninjaone/test-reset.json"));
        assert!(!r[0].problem);
        assert_eq!(r[0].key, e.key);
        let mut expanded = fixture("ninjaone/test-triggered.json");
        expanded["device"] = json!({"systemName":"FS01","displayName":"File server"});
        expanded["severity"] = json!("MODERATE");
        let x = parse(&expanded);
        assert_eq!((x[0].host.as_str(), x[0].severity.as_str()), ("File server", "warning"));
        // Other activities (patching, scripts, …) are not incidents.
        assert!(parse(&json!({"activityType":"PATCH_MANAGEMENT","statusCode":"COMPLETED","deviceId":1})).is_empty());
    }

    #[test]
    fn glpi_ticket() {
        let new = parse(&fixture("glpi/test-new.json"));
        let e = &new[0];
        assert!(e.problem);
        assert_eq!(e.key, "glpi-3");
        assert_eq!(e.title, "E-Mail \"Outlook\" down \\ C:\\temp");
        assert_eq!((e.host.as_str(), e.service.as_str(), e.severity.as_str()), ("Root entity", "", "warning"));
        assert_eq!(e.message, "Users cannot send mail");
        let solved = parse(&fixture("glpi/test-solved.json"));
        assert!(!solved[0].problem);
        assert_eq!(solved[0].key, e.key);
        let mut deleted = fixture("glpi/test-new.json");
        deleted["event"] = json!("delete");
        assert!(!parse(&deleted)[0].problem);
    }

    #[test]
    fn ncentral_xml() {
        let body =
            std::fs::read(format!("{}/../integrations/n-able/test-failed.xml", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let v = xml_fields(&body).expect("xml");
        let e = &parse(&v)[0];
        assert!(e.problem);
        assert_eq!(e.key, "ncentral-Example GmbH/FS01/Disk Drive Space");
        assert_eq!((e.host.as_str(), e.service.as_str(), e.severity.as_str()), ("FS01", "Disk Drive Space", "error"));
        assert_eq!(e.title, "Disk Drive Space (FS01)");
        assert!(e.message.starts_with("C: 97 % used & rising"), "{}", e.message);
        let body =
            std::fs::read(format!("{}/../integrations/n-able/test-normal.xml", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let r = &parse(&xml_fields(&body).unwrap())[0];
        assert_eq!((r.problem, r.key == e.key), (false, true));
        assert!(xml_fields(b"{\"a\":1}").is_none());
        assert!(parse(&xml_fields(b"<n><QualitativeNewState>No Data</QualitativeNewState></n>").unwrap()).is_empty());
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
