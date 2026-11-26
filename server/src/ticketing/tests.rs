use super::*;
use crate::hooks;
use crate::store::Integration;

fn incident(id: &str, source: &str, severity: &str) -> Incident {
    let t = now();
    Incident {
        id: id.into(),
        title: "E-mail is down".into(),
        message: "We are on it.".into(),
        severity: severity.into(),
        targets: vec![],
        status: "active".into(),
        source: source.into(),
        ext_key: None,
        created_at: t,
        starts_at: t,
        ends_at: None,
        resolved_at: None,
        created_by: "admin".into(),
        approved_by: None,
        resolved_by: None,
    }
}

fn enabled(system: &str) -> TicketConfig {
    TicketConfig {
        enabled: true,
        system: system.into(),
        url: "http://127.0.0.1:9".into(),
        since: 1,
        ..Default::default()
    }
}

fn vars(pairs: &[(&str, &str)]) -> Vars {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn pending(conn: &Connection) -> Vec<Entry> {
    entries(conn, 100).unwrap().into_iter().filter(|e| e.state == "pending").rev().collect()
}

// ---- templates and escaping ----------------------------------------------------------------

#[test]
fn placeholders_are_filled_once() {
    let v = vars(&[("title", "Mail {message} down"), ("message", "secret")]);
    assert_eq!(render("{title} / {message}", &v, plain), "Mail {message} down / secret", "values are not re-read");
    assert_eq!(render("{unknown} {title", &v, plain), "{unknown} {title", "unknown or open braces stay");
    assert_eq!(render("{user}", &v, plain), "", "known but empty");
    assert_eq!(render("{{title}}", &v, plain), "{Mail {message} down}");
}

#[test]
fn json_values_are_escaped() {
    let nasty = "Quote \" backslash \\ C:\\temp\nnew line\ttab \u{1} </script> ü";
    assert_eq!(json_escape("a\"b\\c\n"), "a\\\"b\\\\c\\n");
    let cfg = enabled("webhook");
    let mut v = incident_vars(&incident("i1", "manual", "error"), &Settings::default(), &cfg, "created");
    v.insert("title".into(), nasty.into());
    v.insert("message".into(), nasty.into());
    let r = render_all(&cfg, "en", &v);
    let body = webhook_body(&cfg, "en", &r.vars).expect("valid JSON");
    assert_eq!(body["title"], nasty);
    assert_eq!(body["message"], nasty);
    assert_eq!(body["event"], "created");
    assert_eq!(body["origin"], "statusbar-origin:i1");
    assert_eq!(body["priority"], "high");
    assert!(body["body"].as_str().unwrap().contains("[statusbar-origin:i1]"));
    // A template that is not JSON is refused before anything is sent.
    let broken = TicketConfig { webhook_template: "{\"title\": {title}}".into(), ..cfg };
    assert!(webhook_body(&broken, "en", &r.vars).is_err());
}

#[test]
fn text_templates_drop_empty_lines_and_keep_the_marker() {
    let s = Settings::default();
    let cfg = enabled("zammad");
    let plain_message = incident("i1", "manual", "warning");
    let r = render_all(&cfg, "en", &incident_vars(&plain_message, &s, &cfg, "created"));
    assert_eq!(r.title, "Degraded: E-mail is down");
    assert!(r.body.contains("Recipients: everyone"));
    assert!(!r.body.contains("Approved by"), "nobody approved: line left out");
    assert!(!r.body.contains("Raised by"), "not an alarm: line left out");
    assert!(r.body.ends_with("[statusbar-origin:i1]"));
    assert_eq!(r.priority, "2 normal");

    let mut alarm = incident("a1", crate::alarm::SOURCE, "alarm");
    alarm.created_by = "alarm:anna@PC-07".into();
    let de = Settings { language: "de".into(), label_alarm: "Stiller Alarm".into(), ..Settings::default() };
    let r = render_all(&cfg, "de", &incident_vars(&alarm, &de, &cfg, "created"));
    assert!(r.title.starts_with("Stiller Alarm: "));
    assert!(r.body.contains("Ausgelöst von: anna an PC-07"), "{}", r.body);
    assert!(r.body.contains("Herkunft: stiller Alarm"));
    assert_eq!(r.priority, "3 high");

    // An own template that already carries the marker is left as it is.
    let own = TicketConfig { body_template: "{title} ({source}) statusbar-origin:{id}".into(), ..cfg };
    let r = render_all(&own, "en", &incident_vars(&plain_message, &s, &own, "created"));
    assert_eq!(r.body, "E-mail is down (manual) statusbar-origin:i1");
}

#[test]
fn time_is_written_in_utc() {
    assert_eq!(utc(0), "1970-01-01 00:00 UTC");
    assert_eq!(utc(1_795_701_900), "2026-11-26 14:05 UTC");
    assert_eq!(utc(1_709_251_140), "2024-02-29 23:59 UTC");
}

#[test]
fn priorities_map_per_system_and_can_be_overridden() {
    let glpi = enabled("glpi");
    assert_eq!(["error", "warning", "info", "alarm"].map(|k| glpi.priority(k)), ["5", "3", "2", "6"].map(String::from));
    assert_eq!(enabled("znuny").priority("alarm"), "5 very high");
    assert_eq!(enabled("servicenow").priority("info"), "3");
    assert_eq!(enabled("jira").priority("error"), "", "Jira: left to the request type");
    let own =
        TicketConfig { priorities: Priorities { warning: "P2".into(), ..Default::default() }, ..enabled("topdesk") };
    assert_eq!((own.priority("warning").as_str(), own.priority("error").as_str()), ("P2", ""));
    assert_eq!(enabled("zammad").close_status(), "closed");
    assert_eq!(TicketConfig { close_status: "merged".into(), ..enabled("zammad") }.close_status(), "merged");
}

// ---- configuration ----------------------------------------------------------------------------

#[test]
fn secrets_are_write_only() {
    let old = TicketConfig { secret: "s3cret".into(), app_token: "app".into(), ..enabled("glpi") };
    let form = TicketConfig { secret: String::new(), ..old.clone() };
    let kept = sanitize(form.clone(), &old, false, false).unwrap();
    assert_eq!((kept.secret.as_str(), kept.app_token.as_str()), ("s3cret", "app"));
    let shown = public(&kept);
    assert_eq!((shown["secret"].as_str(), shown["secret_set"].as_bool()), (Some(""), Some(true)));
    assert!(!shown.to_string().contains("s3cret"));
    let cleared = sanitize(TicketConfig { enabled: false, ..form }, &old, true, true).unwrap();
    assert!(cleared.secret.is_empty() && cleared.app_token.is_empty());
}

#[test]
fn configuration_is_checked() {
    let off = TicketConfig::default();
    let zammad = TicketConfig { url: "https://zammad.example.com/".into(), secret: "t".into(), ..enabled("zammad") };
    assert!(sanitize(zammad.clone(), &off, false, false).is_err(), "group and customer missing");
    let ok = sanitize(
        TicketConfig { target: "Users".into(), customer: "it@example.com".into(), ..zammad.clone() },
        &off,
        false,
        false,
    )
    .unwrap();
    assert_eq!(ok.url, "https://zammad.example.com");
    assert!(ok.since > 1, "switched on now");
    assert_eq!(sanitize(ok.clone(), &ok, false, false).unwrap().since, ok.since, "kept while on");
    assert!(sanitize(TicketConfig { url: "ftp://x".into(), ..ok.clone() }, &off, false, false).is_err());
    assert!(sanitize(TicketConfig { system: "otrs5".into(), ..ok.clone() }, &off, false, false).is_err());
    assert!(sanitize(TicketConfig { target: "12".into(), ..enabled("jira") }, &off, false, false).is_err());
    let hook = TicketConfig { webhook_template: "{\"a\": {title}}".into(), ..enabled("webhook") };
    assert!(sanitize(hook, &off, false, false).is_err(), "template must give JSON");
    let odd = TicketConfig {
        sources: vec!["hub".into(), "x".into(), "manual".into(), "hub".into()],
        ..TicketConfig::default()
    };
    assert_eq!(sanitize(odd, &off, false, false).unwrap().sources, ["manual", "hub"]);
}

// ---- queue -------------------------------------------------------------------------------------

#[test]
fn published_messages_are_queued_by_trigger_and_source() {
    let conn = store::open(":memory:").unwrap();
    let cfg = enabled("zammad");
    for i in [
        incident("manual", "manual", "error"),
        incident("monitor", "integration-id", "warning"),
        incident("hub", crate::uplink::SOURCE, "info"),
        incident("alarm", crate::alarm::SOURCE, "alarm"),
        Incident { status: "draft".into(), ..incident("draft", "manual", "error") },
        Incident { starts_at: now() + 600, ..incident("later", "manual", "error") },
    ] {
        store::insert_incident(&conn, &i).unwrap();
    }
    assert_eq!(reconcile(&conn, &cfg).unwrap(), 3, "manual, monitoring and alarm; not hub (default)");
    let queued: Vec<String> = pending(&conn).into_iter().map(|e| e.incident_id).collect();
    assert_eq!(queued.len(), 3);
    for id in ["manual", "monitor", "alarm"] {
        assert!(queued.contains(&id.to_string()), "{id}");
    }
    assert_eq!(reconcile(&conn, &cfg).unwrap(), 0, "nothing twice");
    let l = link(&conn, "manual").unwrap().unwrap();
    assert_eq!((l.opened, l.closed, l.ticket_id), (1, 0, None));
    assert_eq!(link(&conn, "hub").unwrap().unwrap().opened, 0, "seen, no ticket wanted");

    // The scheduled message gets its ticket when it starts.
    conn.execute("UPDATE incidents SET starts_at=?1 WHERE id='later'", [now()]).unwrap();
    assert_eq!(reconcile(&conn, &cfg).unwrap(), 1);

    // Switched off: nothing.
    let conn2 = store::open(":memory:").unwrap();
    store::insert_incident(&conn2, &incident("x", "manual", "error")).unwrap();
    assert_eq!(reconcile(&conn2, &TicketConfig::default()).unwrap(), 0);
    // Messages from before switching on are left alone.
    let late = TicketConfig { since: now() + 5, ..cfg.clone() };
    assert_eq!(reconcile(&conn2, &late).unwrap(), 0);
}

#[test]
fn alarms_follow_their_own_trigger() {
    let conn = store::open(":memory:").unwrap();
    store::insert_incident(&conn, &incident("alarm", crate::alarm::SOURCE, "alarm")).unwrap();
    store::insert_incident(&conn, &incident("manual", "manual", "error")).unwrap();
    let cfg = TicketConfig { on_alarm: false, ..enabled("glpi") };
    assert_eq!(reconcile(&conn, &cfg).unwrap(), 1);
    assert_eq!(pending(&conn)[0].incident_id, "manual");
    let only_alarm = TicketConfig { on_publish: false, sources: vec!["alarm".into()], ..enabled("glpi") };
    assert!(wants_open(&only_alarm, &incident("a", crate::alarm::SOURCE, "alarm")));
    assert!(!wants_open(&only_alarm, &incident("m", "manual", "error")));
}

#[test]
fn resolving_closes_or_records() {
    let conn = store::open(":memory:").unwrap();
    let cfg = enabled("zammad");
    store::insert_incident(&conn, &incident("with", "manual", "error")).unwrap();
    reconcile(&conn, &cfg).unwrap();
    let open = pending(&conn).remove(0);
    finish(
        &conn,
        &open,
        &Outcome::Opened(Created { id: "7".into(), reference: Some("zammad-31007".into()), url: None }),
        "zammad",
    )
    .unwrap();
    store::resolve_incident(&conn, "with", "admin").unwrap();
    assert_eq!(reconcile(&conn, &cfg).unwrap(), 1);
    let close = pending(&conn).remove(0);
    assert_eq!((close.action.as_str(), close.vars["event"].as_str()), ("close", "resolved"));
    assert_eq!(close.vars["resolved_by"], "admin");
    assert_eq!(reconcile(&conn, &cfg).unwrap(), 0, "resolution handled once");

    // A message resolved without a ticket gets one only with the "resolved" trigger.
    let mut hub = incident("hub", crate::uplink::SOURCE, "info");
    hub.status = "resolved".into();
    hub.resolved_at = Some(now());
    store::insert_incident(&conn, &hub).unwrap();
    assert_eq!(reconcile(&conn, &cfg).unwrap(), 0, "hub not selected");
    let mut quiet = incident("quiet", "manual", "info");
    quiet.status = "resolved".into();
    quiet.resolved_at = Some(now());
    store::insert_incident(&conn, &quiet).unwrap();
    let recording = TicketConfig { on_publish: false, on_resolve: true, ..cfg.clone() };
    assert_eq!(reconcile(&conn, &recording).unwrap(), 1);
    let record = pending(&conn).into_iter().find(|e| e.incident_id == "quiet").unwrap();
    assert_eq!(record.action, "record");
    // Opened: closing follows as its own step.
    finish(&conn, &record, &Outcome::Recorded(Created { id: "8".into(), ..Default::default() }), "zammad").unwrap();
    let again = pending(&conn).into_iter().find(|e| e.incident_id == "quiet").unwrap();
    assert_eq!((again.action.as_str(), again.attempts), ("close", 0));

    // Resolving switched off: no close entry.
    let conn = store::open(":memory:").unwrap();
    store::insert_incident(&conn, &incident("m", "manual", "error")).unwrap();
    let none = TicketConfig { resolve_mode: "none".into(), ..cfg };
    reconcile(&conn, &none).unwrap();
    store::resolve_incident(&conn, "m", "admin").unwrap();
    assert_eq!(reconcile(&conn, &none).unwrap(), 0);
}

#[test]
fn failures_back_off_and_give_up() {
    let conn = store::open(":memory:").unwrap();
    store::insert_incident(&conn, &incident("m", "manual", "error")).unwrap();
    reconcile(&conn, &enabled("glpi")).unwrap();
    assert_eq!([1, 2, 3, 8, 20].map(backoff), [30, 60, 120, 3600, 3600]);
    for attempt in 1..MAX_ATTEMPTS {
        let e = next_due(&conn, now() + 10_000).unwrap().expect("still pending");
        assert_eq!(e.attempts, attempt - 1);
        finish(&conn, &e, &Outcome::Failed(Fail::Retry("503".into())), "glpi").unwrap();
        let after = entries(&conn, 1).unwrap().remove(0);
        assert_eq!((after.state.as_str(), after.attempts), ("pending", attempt));
        assert!(after.next_at >= now() + backoff(attempt) - 1);
        assert!(next_due(&conn, now()).unwrap().is_none(), "not due before the backoff");
    }
    let last = next_due(&conn, now() + 10_000).unwrap().unwrap();
    finish(&conn, &last, &Outcome::Failed(Fail::Retry("503".into())), "glpi").unwrap();
    assert_eq!(entries(&conn, 1).unwrap()[0].state, "failed");
    assert_eq!(link(&conn, "m").unwrap().unwrap().opened, 3);

    // A refusal (4xx) is not repeated.
    store::insert_incident(&conn, &incident("n", "manual", "error")).unwrap();
    reconcile(&conn, &enabled("glpi")).unwrap();
    let e = next_due(&conn, now()).unwrap().unwrap();
    finish(&conn, &e, &Outcome::Failed(Fail::Permanent("400 bad category".into())), "glpi").unwrap();
    let after = entries(&conn, 1).unwrap().remove(0);
    assert_eq!((after.state.as_str(), after.last_error.as_deref()), ("failed", Some("400 bad category")));
    // Waiting for the ticket to be opened costs no attempt.
    store::insert_incident(&conn, &incident("w", "manual", "error")).unwrap();
    reconcile(&conn, &enabled("glpi")).unwrap();
    let e = next_due(&conn, now()).unwrap().unwrap();
    finish(&conn, &e, &Outcome::Wait, "glpi").unwrap();
    let after = entries(&conn, 1).unwrap().remove(0);
    assert_eq!((after.state.as_str(), after.attempts), ("pending", 0));
}

#[tokio::test]
async fn closing_waits_for_the_ticket() {
    let http = client();
    let cfg = enabled("zammad");
    let e = Entry {
        id: 1,
        incident_id: "m".into(),
        action: "close".into(),
        vars: Vars::new(),
        attempts: 0,
        next_at: 0,
        state: "pending".into(),
        last_error: None,
        created_at: 0,
    };
    let mut l = Link {
        incident_id: "m".into(),
        system: "zammad".into(),
        ticket_id: None,
        ticket_ref: None,
        ticket_url: None,
        opened: 1,
        closed: 1,
    };
    assert_eq!(deliver(&http, &cfg, Some(&l), &e).await, Outcome::Wait);
    l.opened = 3;
    assert!(matches!(deliver(&http, &cfg, Some(&l), &e).await, Outcome::Skipped(_)));
    assert!(matches!(deliver(&http, &cfg, None, &e).await, Outcome::Skipped(_)));
}

// ---- loop protection -----------------------------------------------------------------------------

fn integration() -> Integration {
    Integration {
        id: "zammad-in".into(),
        name: "Zammad".into(),
        unmatched: "publish".into(),
        delay_seconds: 0,
        targets: vec![],
        created_at: 0,
        last_seen: None,
        events: 0,
    }
}

fn event(key: &str, title: &str, problem: bool) -> Event {
    Event {
        key: key.into(),
        problem,
        title: title.into(),
        message: String::new(),
        severity: "error".into(),
        host: "Users".into(),
        service: title.into(),
    }
}

#[test]
fn own_tickets_do_not_come_back() {
    let conn = store::open(":memory:").unwrap();
    let cfg = enabled("zammad");
    store::insert_incident(&conn, &incident("m", "manual", "error")).unwrap();
    reconcile(&conn, &cfg).unwrap();
    // While the ticket is being opened, its title identifies it (webhook faster than the answer).
    let racing = event("zammad-31007", "Outage: E-mail is down", true);
    assert_eq!(hooks::apply(&conn, &integration(), &[], &racing).unwrap(), hooks::OWN_TICKET);
    let e = pending(&conn).remove(0);
    finish(
        &conn,
        &e,
        &Outcome::Opened(Created { id: "7".into(), reference: Some("zammad-31007".into()), url: None }),
        "zammad",
    )
    .unwrap();
    // Afterwards by its key, whatever the title.
    for problem in [true, false] {
        let mine = event("zammad-31007", "renamed by an agent", problem);
        assert_eq!(hooks::apply(&conn, &integration(), &[], &mine).unwrap(), hooks::OWN_TICKET);
    }
    assert_eq!(store::recent_incidents(&conn, 10).unwrap().len(), 1, "no second message");
    // Other tickets work as before, even with the same title.
    let other = event("zammad-31008", "Outage: E-mail is down", true);
    assert_eq!(hooks::apply(&conn, &integration(), &[], &other).unwrap(), "created");
    assert_eq!(store::recent_incidents(&conn, 10).unwrap().len(), 2);
}

#[test]
fn marked_payloads_are_recognised() {
    assert!(payload_marked(&json!({"item": {"content": "text<br>[statusbar-origin:abc]"}})));
    assert!(payload_marked(&json!({"tags": ["vip", "statusbar-origin"]})));
    assert!(!payload_marked(&json!({"title": "statusbar outage", "n": 1})));
}

// ---- delivery against a stand-in server ----------------------------------------------------------

#[tokio::test]
async fn webhook_delivery_end_to_end() {
    use axum::{extract::State as S, http::StatusCode, routing::post, Router};
    use std::sync::{Arc, Mutex};
    type Seen = Arc<Mutex<Vec<(String, Value)>>>;
    let seen: Seen = Arc::default();
    async fn hook(S(seen): S<Seen>, headers: HeaderMap, body: String) -> (StatusCode, String) {
        let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
        let auth = headers.get("authorization").and_then(|h| h.to_str().ok()).unwrap_or("").to_string();
        let status = match v["title"].as_str() {
            Some("fail-503") => StatusCode::SERVICE_UNAVAILABLE,
            Some("fail-400") => StatusCode::BAD_REQUEST,
            _ => StatusCode::OK,
        };
        seen.lock().unwrap().push((auth, v));
        (status, r#"{"id": 4711, "url": "https://tickets.example.com/4711"}"#.into())
    }
    let app = Router::new().route("/hook", post(hook)).with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let http = client();
    let cfg = TicketConfig { url: format!("http://{addr}/hook"), secret: "tok".into(), ..enabled("webhook") };
    let i = incident("m1", "manual", "error");
    let mut v = incident_vars(&i, &Settings::default(), &cfg, "created");
    v.insert("_language".into(), "en".into());
    let open = Entry {
        id: 1,
        incident_id: "m1".into(),
        action: "open".into(),
        vars: v.clone(),
        attempts: 0,
        next_at: 0,
        state: "pending".into(),
        last_error: None,
        created_at: 0,
    };
    let Outcome::Opened(c) = deliver(&http, &cfg, None, &open).await else { panic!("not opened") };
    assert_eq!((c.id.as_str(), c.url.as_deref()), ("4711", Some("https://tickets.example.com/4711")));
    let l = Link {
        incident_id: "m1".into(),
        system: "webhook".into(),
        ticket_id: Some(c.id),
        ticket_ref: None,
        ticket_url: c.url,
        opened: 2,
        closed: 1,
    };
    let mut resolved = v.clone();
    resolved.insert("event".into(), "resolved".into());
    resolved.insert("resolved_by".into(), "anna".into());
    let close = Entry { action: "close".into(), vars: resolved, ..open.clone() };
    assert_eq!(deliver(&http, &cfg, Some(&l), &close).await, Outcome::Closed);
    {
        let got = seen.lock().unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "Bearer tok");
        assert_eq!((got[0].1["event"].as_str(), got[0].1["title"].as_str()), (Some("created"), Some("E-mail is down")));
        assert_eq!((got[1].1["event"].as_str(), got[1].1["ticket_id"].as_str()), (Some("resolved"), Some("4711")));
        assert_eq!(got[1].1["body"], "Statusbar: message resolved by anna at .");
    }
    // 5xx is retried, 4xx is not, an unreachable server is retried.
    let mut failing = v.clone();
    failing.insert("title".into(), "fail-503".into());
    let e = Entry { vars: failing.clone(), ..open.clone() };
    assert!(matches!(deliver(&http, &cfg, None, &e).await, Outcome::Failed(Fail::Retry(_))));
    failing.insert("title".into(), "fail-400".into());
    let e = Entry { vars: failing, ..open.clone() };
    assert!(matches!(deliver(&http, &cfg, None, &e).await, Outcome::Failed(Fail::Permanent(_))));
    let gone = TicketConfig { url: "http://127.0.0.1:9/hook".into(), ..cfg.clone() };
    assert!(matches!(deliver(&http, &gone, None, &open).await, Outcome::Failed(Fail::Retry(_))));
}
