//! Statusbar server: incidents in SQLite, long-poll delivery to desktop clients,
//! monitoring webhooks with auto-resolve, and a small admin web UI.

mod ad;
mod glob;
mod hooks;
mod store;
mod zip;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};
use store::{now, Incident, Integration, Rule, Settings};
use subtle::ConstantTimeEq;
use tokio::sync::watch;
use tracing::{error, info, warn};

type HttpError = (StatusCode, String);
type Api = Result<Json<Value>, HttpError>;

struct Config {
    admin_token: String,
    client_token: String,
    legacy_webhook_token: String,
    session_hours: i64,
    retention_days: i64,
    locales_dir: String,
    client_msi: String,
    ad: Option<ad::AdConfig>,
}

struct Session {
    username: String,
    expires: i64,
}

struct Inner {
    cfg: Config,
    db: Mutex<Connection>,
    settings: RwLock<Settings>,
    /// Incidents clients may currently see; rebuilt by `refresh`.
    live: RwLock<Arc<Vec<Incident>>>,
    /// Bumped whenever `live` or the display settings change; wakes long-polls.
    rev: watch::Sender<u64>,
    directory: RwLock<ad::Directory>,
    sessions: Mutex<HashMap<String, Session>>,
}
type App = Arc<Inner>;

// ---- helpers -----------------------------------------------------------------

fn env_get(key: &str, default: &str) -> String {
    env::var(key).ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| default.to_owned())
}
fn env_bool(key: &str, default: bool) -> bool {
    match env::var(key).ok().map(|s| s.trim().to_ascii_lowercase()) {
        None => default,
        Some(v) if v.is_empty() => default,
        Some(v) if ["true", "yes", "1", "on"].contains(&v.as_str()) => true,
        Some(v) if ["false", "no", "0", "off"].contains(&v.as_str()) => false,
        Some(v) => panic!("Invalid boolean for {key}: {v} (use yes/no)"),
    }
}
fn env_num<T: std::str::FromStr>(key: &str, default: T) -> T {
    env::var(key).ok().and_then(|v| v.trim().parse().ok()).unwrap_or(default)
}
fn internal(e: impl std::fmt::Display) -> HttpError {
    error!("internal error: {e}");
    (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".into())
}
fn bad(msg: &str) -> HttpError {
    (StatusCode::BAD_REQUEST, msg.into())
}
fn ok() -> Api {
    Ok(Json(json!({"ok": true})))
}
fn same(a: &str, b: &str) -> bool {
    !a.is_empty() && a.len() == b.len() && bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}
fn sha256_hex(s: &str) -> String {
    Sha256::digest(s.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}
fn random_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}
fn bearer(headers: &HeaderMap) -> &str {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("")
        .trim()
}
fn valid_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}
fn valid_locale(s: &str) -> bool {
    (2..=16).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn clean(s: &str, max: usize) -> String {
    s.trim().chars().filter(|c| !c.is_control() || *c == '\n').take(max).collect()
}

/// Rebuilds the client-visible incident list and wakes waiting clients if it changed.
fn refresh(app: &App) {
    let resolved_seconds = app.settings.read().unwrap().resolved_seconds;
    let fresh = {
        let db = app.db.lock().unwrap();
        store::live_incidents(&db, resolved_seconds)
    };
    match fresh {
        Ok(list) => {
            let changed = **app.live.read().unwrap() != list;
            if changed {
                *app.live.write().unwrap() = Arc::new(list);
                app.rev.send_modify(|v| *v += 1);
            }
        }
        Err(e) => error!("refresh failed: {e}"),
    }
}

// ---- authentication ------------------------------------------------------------

const COOKIE: &str = "sb_session";

fn session_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|c| c.strip_prefix(COOKIE).and_then(|rest| rest.strip_prefix('=')).map(str::to_owned))
}

fn session_user(app: &App, headers: &HeaderMap) -> Result<String, HttpError> {
    let denied = || (StatusCode::UNAUTHORIZED, "Sign-in required".to_string());
    let sid = session_id(headers).ok_or_else(denied)?;
    let username = {
        let sessions = app.sessions.lock().unwrap();
        let s = sessions.get(&sid).ok_or_else(denied)?;
        if s.expires < now() {
            return Err(denied());
        }
        s.username.clone()
    };
    // In directory mode the admin right is re-checked on every request, so removing
    // someone from the admin group takes effect at the next directory sync.
    if let Some(adc) = &app.cfg.ad {
        let dir = app.directory.read().unwrap();
        let allowed = dir.users.get(&username).is_some_and(|p| ad::is_admin(p, &adc.admin_group));
        if !allowed {
            return Err((StatusCode::FORBIDDEN, "Administrator group membership required".into()));
        }
    }
    Ok(username)
}

fn admin(app: &App, headers: &HeaderMap) -> Result<(), HttpError> {
    if same(bearer(headers), &app.cfg.admin_token) {
        return Ok(());
    }
    session_user(app, headers).map(|_| ())
}

#[derive(Deserialize)]
struct LoginForm {
    #[serde(default)]
    username: String,
    password: String,
}

async fn login(State(app): State<App>, Json(form): Json<LoginForm>) -> Result<Response, HttpError> {
    let username = match check_login(&app, form).await {
        Some(u) => u,
        None => {
            // Flat delay on failure: cheap brute-force damping without state.
            tokio::time::sleep(Duration::from_millis(1200)).await;
            return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".into()));
        }
    };
    let sid = random_token();
    {
        let mut sessions = app.sessions.lock().unwrap();
        let t = now();
        sessions.retain(|_, s| s.expires > t);
        sessions.insert(sid.clone(), Session { username: username.clone(), expires: t + app.cfg.session_hours * 3600 });
    }
    let cookie =
        format!("{COOKIE}={sid}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age={}", app.cfg.session_hours * 3600);
    let mut response = Json(json!({"ok": true, "username": username})).into_response();
    response.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).map_err(internal)?);
    Ok(response)
}

async fn check_login(app: &App, form: LoginForm) -> Option<String> {
    let Some(adc) = app.cfg.ad.clone() else {
        return same(&form.password, &app.cfg.admin_token).then(|| "admin".to_string());
    };
    let username = form.username.trim().rsplit('\\').next().unwrap_or("").to_lowercase();
    if username.is_empty() || username.len() > 128 || form.password.is_empty() {
        return None;
    }
    let dn = {
        let dir = app.directory.read().unwrap();
        let person = dir.users.get(&username)?;
        if !ad::is_admin(person, &adc.admin_group) {
            return None;
        }
        person.dn.clone()
    };
    let password = form.password;
    let result = tokio::task::spawn_blocking(move || ad::check_password(&adc, &dn, &password)).await;
    matches!(result, Ok(Ok(()))).then_some(username)
}

async fn logout(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Some(sid) = session_id(&headers) {
        app.sessions.lock().unwrap().remove(&sid);
    }
    let mut response = Json(json!({"ok": true})).into_response();
    let cookie = format!("{COOKIE}=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0");
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

async fn me(State(app): State<App>, headers: HeaderMap) -> Api {
    let username = session_user(&app, &headers)?;
    Ok(Json(json!({"username": username})))
}

// ---- public metadata -------------------------------------------------------------

fn extra_locales(dir: &str) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".json")).map(str::to_owned))
                .filter(|n| valid_locale(n) && n != "en" && n != "de")
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

async fn meta(State(app): State<App>) -> Api {
    let s = app.settings.read().unwrap().clone();
    let has_logo = {
        let db = app.db.lock().unwrap();
        store::logo(&db).map_err(internal)?.is_some()
    };
    let mut languages = vec!["en".to_string(), "de".to_string()];
    languages.extend(extra_locales(&app.cfg.locales_dir));
    Ok(Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "mode": if app.cfg.ad.is_some() { "directory" } else { "standalone" },
        "name": s.name, "subtitle": s.subtitle, "color": s.color, "language": s.language,
        "has_logo": has_logo, "languages": languages,
    })))
}

async fn logo(State(app): State<App>) -> Result<Response, HttpError> {
    let found = {
        let db = app.db.lock().unwrap();
        store::logo(&db).map_err(internal)?
    };
    let (data, mime) = found.ok_or((StatusCode::NOT_FOUND, "No logo".to_string()))?;
    Ok(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "no-cache".to_string())], data).into_response())
}

async fn locale(State(app): State<App>, Path(name): Path<String>) -> Result<Response, HttpError> {
    let body = match name.as_str() {
        "en" => include_str!("../locales/en.json").to_string(),
        "de" => include_str!("../locales/de.json").to_string(),
        other if valid_locale(other) => {
            std::fs::read_to_string(std::path::Path::new(&app.cfg.locales_dir).join(format!("{other}.json")))
                .map_err(|_| (StatusCode::NOT_FOUND, "Unknown language".to_string()))?
        }
        _ => return Err((StatusCode::NOT_FOUND, "Unknown language".into())),
    };
    Ok(([(header::CONTENT_TYPE, "application/json; charset=utf-8")], body).into_response())
}

async fn index() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; frame-ancestors 'none'",
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        include_str!("../static/index.html"),
    )
        .into_response()
}

async fn app_js() -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8"), (header::CACHE_CONTROL, "no-cache")],
        include_str!("../static/app.js"),
    )
        .into_response()
}

// ---- desktop clients ---------------------------------------------------------------

#[derive(Deserialize)]
struct ClientQuery {
    user: Option<String>,
    computer: Option<String>,
    rev: Option<u64>,
    wait: Option<u64>,
}

fn visible(i: &Incident, user: &str, computer: &str, person: Option<&ad::Person>) -> bool {
    if i.targets.is_empty() {
        return true;
    }
    i.targets.iter().any(|t| match t.split_once(':') {
        Some(("host", pattern)) => !computer.is_empty() && glob::matches(pattern, computer),
        Some(("user", name)) => person.is_some() && name.eq_ignore_ascii_case(user),
        Some(("group", dn)) => person.is_some_and(|p| p.groups.contains(&dn.to_lowercase())),
        Some(("ou", dn)) => person.is_some_and(|p| p.dn.to_lowercase().ends_with(&format!(",{}", dn.to_lowercase()))),
        _ => false,
    })
}

async fn client_state(State(app): State<App>, headers: HeaderMap, Query(q): Query<ClientQuery>) -> Api {
    if !same(bearer(&headers), &app.cfg.client_token) {
        return Err((StatusCode::UNAUTHORIZED, "Client token required".into()));
    }
    // Long-poll: a client that already has the current revision waits for the next change.
    let mut rx = app.rev.subscribe();
    let current = *rx.borrow_and_update();
    if q.rev == Some(current) {
        let wait = q.wait.unwrap_or(25).clamp(1, 55);
        let _ = tokio::time::timeout(Duration::from_secs(wait), rx.changed()).await;
    }
    let rev = *app.rev.borrow();
    let user = q.user.as_deref().unwrap_or("").rsplit('\\').next().unwrap_or("").to_lowercase();
    let computer = q.computer.as_deref().unwrap_or("").trim().to_string();
    let (person, directory_ready) = {
        let dir = app.directory.read().unwrap();
        (dir.users.get(&user).cloned(), dir.last_sync.is_some())
    };
    let live = app.live.read().unwrap().clone();
    let incidents: Vec<Value> = live
        .iter()
        .filter(|i| visible(i, &user, &computer, person.as_ref()))
        .map(
            |i| json!({"id": i.id, "title": i.title, "message": i.message, "severity": i.severity, "status": i.status}),
        )
        .collect();
    let s = app.settings.read().unwrap().clone();
    Ok(Json(json!({
        "rev": rev,
        "incidents": incidents,
        "resolved_display_seconds": s.resolved_seconds,
        "directory_ready": directory_ready,
        "display": {
            "position": s.position, "all_monitors": s.all_monitors, "height_px": s.height_px,
            "initial_height_px": s.initial_height_px, "initial_seconds": s.initial_seconds,
            "over_fullscreen": s.over_fullscreen, "rdp_windowed": s.rdp_windowed,
            "labels": {"error": s.label_error, "warning": s.label_warning, "info": s.label_info, "resolved": s.label_resolved},
        },
    })))
}

// ---- incidents (admin) ---------------------------------------------------------------

fn check_targets(app: &App, targets: &[String]) -> Result<Vec<String>, HttpError> {
    if targets.len() > 50 {
        return Err(bad("At most 50 targets"));
    }
    let mut out = Vec::new();
    for t in targets {
        let (kind, value) = t.split_once(':').ok_or_else(|| bad("Target must look like kind:value"))?;
        let value = value.trim();
        if value.is_empty() || value.len() > 400 {
            return Err(bad("Target value missing or too long"));
        }
        match kind {
            "host" => {}
            "group" | "ou" | "user" if app.cfg.ad.is_some() => {}
            "group" | "ou" | "user" => return Err(bad("Directory targets need AD_ENABLED=yes")),
            _ => return Err(bad("Unknown target kind (host, group, ou, user)")),
        }
        out.push(format!("{kind}:{value}"));
    }
    Ok(out)
}

fn check_severity(raw: Option<&str>) -> Result<String, HttpError> {
    let s = raw.unwrap_or("error").to_ascii_lowercase();
    if ["error", "warning", "info"].contains(&s.as_str()) {
        Ok(s)
    } else {
        Err(bad("Severity must be error, warning or info"))
    }
}

#[derive(Deserialize)]
struct NewIncident {
    title: String,
    #[serde(default)]
    message: String,
    severity: Option<String>,
    #[serde(default)]
    targets: Vec<String>,
    starts_at: Option<i64>,
    ends_at: Option<i64>,
}

fn create(app: &App, form: NewIncident, source: &str) -> Api {
    let title = clean(&form.title, 120);
    if title.is_empty() {
        return Err(bad("Title required (max. 120 characters)"));
    }
    let t = now();
    let starts_at = form.starts_at.filter(|v| *v > t).unwrap_or(t);
    if form.ends_at.is_some_and(|end| end <= starts_at) {
        return Err(bad("End must be after start"));
    }
    let incident = Incident {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        message: clean(&form.message, 1500),
        severity: check_severity(form.severity.as_deref())?,
        targets: check_targets(app, &form.targets)?,
        status: "active".into(),
        source: source.into(),
        ext_key: None,
        created_at: t,
        starts_at,
        ends_at: form.ends_at,
        resolved_at: None,
    };
    {
        let db = app.db.lock().unwrap();
        store::insert_incident(&db, &incident).map_err(internal)?;
    }
    refresh(app);
    Ok(Json(json!(incident)))
}

async fn create_incident(State(app): State<App>, headers: HeaderMap, Json(form): Json<NewIncident>) -> Api {
    admin(&app, &headers)?;
    create(&app, form, "manual")
}

async fn list_incidents(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    let list = {
        let db = app.db.lock().unwrap();
        store::recent_incidents(&db, 200).map_err(internal)?
    };
    Ok(Json(json!({"incidents": list, "now": now()})))
}

fn change(app: &App, result: store::DbResult<usize>) -> Api {
    if result.map_err(internal)? == 0 {
        return Err((StatusCode::NOT_FOUND, "Not found or already in that state".into()));
    }
    refresh(app);
    ok()
}

async fn resolve_incident(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Api {
    admin(&app, &headers)?;
    let result = { store::resolve_incident(&app.db.lock().unwrap(), &id) };
    change(&app, result)
}
async fn publish_incident(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Api {
    admin(&app, &headers)?;
    let result = { store::publish_incident(&app.db.lock().unwrap(), &id) };
    change(&app, result)
}
async fn delete_incident(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Api {
    admin(&app, &headers)?;
    let result = { store::delete_incident(&app.db.lock().unwrap(), &id) };
    change(&app, result)
}

// ---- settings (admin) ------------------------------------------------------------------

fn sanitize(mut s: Settings) -> Result<Settings, HttpError> {
    s.name = clean(&s.name, 70);
    s.subtitle = clean(&s.subtitle, 140);
    s.language = s.language.trim().to_ascii_lowercase();
    if s.name.is_empty() || !valid_color(&s.color) || !valid_locale(&s.language) {
        return Err(bad("Name, #rrggbb colour and language are required"));
    }
    s.position = if s.position == "bottom" { "bottom".into() } else { "top".into() };
    s.height_px = s.height_px.clamp(24, 160);
    s.initial_height_px = s.initial_height_px.clamp(s.height_px, 300);
    s.initial_seconds = s.initial_seconds.min(60);
    s.resolved_seconds = s.resolved_seconds.clamp(0, 3600);
    for label in [&mut s.label_error, &mut s.label_warning, &mut s.label_info, &mut s.label_resolved] {
        *label = clean(label, 30).replace('\n', " ");
    }
    Ok(s)
}

async fn get_settings(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    Ok(Json(json!(*app.settings.read().unwrap())))
}

async fn put_settings(State(app): State<App>, headers: HeaderMap, Json(form): Json<Settings>) -> Api {
    admin(&app, &headers)?;
    let s = sanitize(form)?;
    {
        let db = app.db.lock().unwrap();
        store::save_settings(&db, &s).map_err(internal)?;
    }
    *app.settings.write().unwrap() = s.clone();
    app.rev.send_modify(|v| *v += 1);
    refresh(&app);
    Ok(Json(json!(s)))
}

async fn put_logo(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Api {
    admin(&app, &headers)?;
    if body.is_empty() || body.len() > 1_000_000 {
        return Err(bad("Logo must be between 1 byte and 1 MB"));
    }
    let mime = if body.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) {
        "image/png"
    } else if body.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if body.len() > 12 && &body[..4] == b"RIFF" && &body[8..12] == b"WEBP" {
        "image/webp"
    } else {
        return Err(bad("Only PNG, JPEG or WebP"));
    };
    let db = app.db.lock().unwrap();
    store::set_logo(&db, Some((&body[..], mime))).map_err(internal)?;
    ok()
}

async fn delete_logo(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    let db = app.db.lock().unwrap();
    store::set_logo(&db, None).map_err(internal)?;
    ok()
}

/// The installer: an explicit CLIENT_MSI, else one placed in the data volume, else the one shipped in the image.
fn msi_path(cfg: &Config) -> Option<std::path::PathBuf> {
    let candidates: Vec<&str> = if cfg.client_msi.is_empty() {
        vec!["/data/Statusbar.msi", "/opt/statusbar/Statusbar.msi"]
    } else {
        vec![cfg.client_msi.as_str()]
    };
    candidates.into_iter().map(std::path::PathBuf::from).find(|p| p.is_file())
}

fn read_msi(cfg: &Config) -> Result<Vec<u8>, HttpError> {
    let missing = || (StatusCode::NOT_FOUND, "No installer on this server".to_string());
    std::fs::read(msi_path(cfg).ok_or_else(missing)?).map_err(|_| missing())
}

fn download(content_type: &'static str, filename: &'static str, data: Vec<u8>) -> Response {
    let disposition = format!("attachment; filename=\"{filename}\"");
    ([(header::CONTENT_TYPE, content_type.to_string()), (header::CONTENT_DISPOSITION, disposition)], data)
        .into_response()
}

/// What an admin needs to roll out clients: the shared client token and whether an installer is on hand.
async fn deploy(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    Ok(Json(json!({"client_token": app.cfg.client_token, "has_msi": msi_path(&app.cfg).is_some()})))
}

async fn client_msi(State(app): State<App>, headers: HeaderMap) -> Result<Response, HttpError> {
    admin(&app, &headers)?;
    Ok(download("application/x-msi", "Statusbar.msi", read_msi(&app.cfg)?))
}

#[derive(Deserialize)]
struct PackageQuery {
    server: String,
}

/// Only characters that are harmless inside a .cmd file.
fn script_safe(s: &str) -> bool {
    !s.is_empty() && s.len() <= 300 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b".:/_-".contains(&b))
}

fn install_scripts(server: &str, token: &str) -> (String, String, String) {
    let install = format!(
        "@echo off\r\nrem Installs or updates the client for all users of this computer. Run elevated.\r\n\
         msiexec /i \"%~dp0Statusbar.msi\" SERVERURL={server} CLIENTTOKEN={token} /qn /norestart\r\nexit /b %ERRORLEVEL%\r\n"
    );
    let uninstall =
        "@echo off\r\nmsiexec /x \"%~dp0Statusbar.msi\" /qn /norestart\r\nexit /b %ERRORLEVEL%\r\n".to_string();
    let readme = format!(
        "Client package for {server}\r\n\r\n\
         install.cmd    installs or updates silently (run as administrator or from your deployment tool)\r\n\
         uninstall.cmd  removes the client\r\n\r\n\
         Deployment tools can call the installer directly:\r\n\
         msiexec /i Statusbar.msi SERVERURL={server} CLIENTTOKEN=<token from install.cmd> /qn\r\n\r\n\
         The client starts at each user's next sign-in. The package contains the client token:\r\n\
         it lets a computer read messages and nothing else. Keep it inside your organisation.\r\n"
    );
    (install, uninstall, readme)
}

/// The unchanged (signed) installer plus scripts filled in for this server, as one ZIP.
async fn client_package(
    State(app): State<App>,
    headers: HeaderMap,
    Query(q): Query<PackageQuery>,
) -> Result<Response, HttpError> {
    admin(&app, &headers)?;
    let server = q.server.trim().trim_end_matches('/');
    let local = server.starts_with("http://127.0.0.1") || server.starts_with("http://localhost");
    if !(server.starts_with("https://") || local) || !script_safe(server) {
        return Err(bad("server must be an https:// address without special characters"));
    }
    if !script_safe(&app.cfg.client_token) {
        return Err(bad("CLIENT_TOKEN contains characters that cannot be written into an install script"));
    }
    let msi = read_msi(&app.cfg)?;
    let (install, uninstall, readme) = install_scripts(server, &app.cfg.client_token);
    let archive = zip::archive(&[
        ("Statusbar.msi", &msi[..]),
        ("install.cmd", install.as_bytes()),
        ("uninstall.cmd", uninstall.as_bytes()),
        ("README.txt", readme.as_bytes()),
    ]);
    Ok(download("application/zip", "Statusbar-setup.zip", archive))
}

async fn targets(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    let dir = app.directory.read().unwrap();
    let mut users: Vec<&String> = dir.users.keys().collect();
    users.sort();
    Ok(Json(json!({
        "directory": app.cfg.ad.is_some(),
        "groups": dir.groups, "ous": dir.ous, "users": users,
        "last_sync": dir.last_sync, "last_error": dir.last_error,
    })))
}

// ---- integrations and rules (admin) ------------------------------------------------------

#[derive(Deserialize)]
struct NewIntegration {
    name: String,
    unmatched: Option<String>,
    delay_seconds: Option<i64>,
    #[serde(default)]
    targets: Vec<String>,
}

async fn list_integrations(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    let db = app.db.lock().unwrap();
    let integrations = store::integrations(&db).map_err(internal)?;
    let rules = store::rules(&db).map_err(internal)?;
    Ok(Json(json!({"integrations": integrations, "rules": rules})))
}

async fn create_integration(State(app): State<App>, headers: HeaderMap, Json(form): Json<NewIntegration>) -> Api {
    admin(&app, &headers)?;
    let name = clean(&form.name, 60);
    let unmatched = form.unmatched.unwrap_or_else(|| "draft".into());
    if name.is_empty() || !["draft", "publish", "ignore"].contains(&unmatched.as_str()) {
        return Err(bad("Name required; unmatched must be draft, publish or ignore"));
    }
    let integration = Integration {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        unmatched,
        delay_seconds: form.delay_seconds.unwrap_or(0).clamp(0, 3600),
        targets: check_targets(&app, &form.targets)?,
        created_at: now(),
        last_seen: None,
        events: 0,
    };
    // The token is shown exactly once; only its SHA-256 is stored.
    let token = format!("sb_{}", random_token());
    let db = app.db.lock().unwrap();
    store::insert_integration(&db, &integration, &sha256_hex(&token)).map_err(internal)?;
    Ok(Json(json!({"integration": integration, "token": token})))
}

async fn delete_integration(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Api {
    admin(&app, &headers)?;
    let n = { store::delete_integration(&app.db.lock().unwrap(), &id) }.map_err(internal)?;
    if n == 0 {
        return Err((StatusCode::NOT_FOUND, "Not found".into()));
    }
    ok()
}

#[derive(Deserialize)]
struct NewRule {
    pattern: String,
    title: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    targets: Vec<String>,
}

async fn create_rule(State(app): State<App>, headers: HeaderMap, Json(form): Json<NewRule>) -> Api {
    admin(&app, &headers)?;
    let rule = Rule {
        id: uuid::Uuid::new_v4().to_string(),
        pattern: clean(&form.pattern, 200),
        title: clean(&form.title, 120),
        message: clean(&form.message, 1500),
        severity: if form.severity.trim().is_empty() {
            String::new()
        } else {
            check_severity(Some(form.severity.trim()))?
        },
        targets: check_targets(&app, &form.targets)?,
    };
    if rule.pattern.is_empty() || rule.title.is_empty() {
        return Err(bad("Pattern and title required"));
    }
    let db = app.db.lock().unwrap();
    store::insert_rule(&db, &rule).map_err(internal)?;
    Ok(Json(json!(rule)))
}

async fn delete_rule(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Api {
    admin(&app, &headers)?;
    let n = { store::delete_rule(&app.db.lock().unwrap(), &id) }.map_err(internal)?;
    if n == 0 {
        return Err((StatusCode::NOT_FOUND, "Not found".into()));
    }
    ok()
}

// ---- monitoring webhooks ----------------------------------------------------------------

#[derive(Deserialize)]
struct HookQuery {
    token: Option<String>,
}

fn run_hook(app: &App, token: &str, body: &[u8]) -> Api {
    let denied = || (StatusCode::UNAUTHORIZED, "Unknown integration token".to_string());
    if token.len() < 20 {
        return Err(denied());
    }
    let payload: Value = serde_json::from_slice(body).map_err(|_| bad("JSON body required"))?;
    let events = hooks::parse(&payload);
    let mut results: Vec<&str> = Vec::new();
    {
        let db = app.db.lock().unwrap();
        let integration = store::integration_by_hash(&db, &sha256_hex(token)).map_err(internal)?.ok_or_else(denied)?;
        let rules = store::rules(&db).map_err(internal)?;
        for e in &events {
            let outcome = hooks::apply(&db, &integration, &rules, e).map_err(internal)?;
            info!(integration = %integration.name, key = %e.key, problem = e.problem, outcome, "monitoring event");
            results.push(outcome);
        }
        store::touch_integration(&db, &integration.id, events.len() as i64).map_err(internal)?;
    }
    refresh(app);
    Ok(Json(json!({"ok": true, "events": results})))
}

async fn hook_header(State(app): State<App>, headers: HeaderMap, Query(q): Query<HookQuery>, body: Bytes) -> Api {
    let token = q.token.unwrap_or_else(|| bearer(&headers).to_string());
    run_hook(&app, &token, &body)
}
async fn hook_path(State(app): State<App>, Path(token): Path<String>, body: Bytes) -> Api {
    run_hook(&app, &token, &body)
}

/// Pre-0.5 webhook (`WEBHOOK_TOKEN`): creates a plain incident, cannot resolve.
#[derive(Deserialize)]
struct LegacyIncident {
    title: String,
    #[serde(default)]
    message: String,
    severity: Option<String>,
    target_type: Option<String>,
    target_dn: Option<String>,
}
async fn legacy_webhook(State(app): State<App>, headers: HeaderMap, Json(old): Json<LegacyIncident>) -> Api {
    if !same(bearer(&headers), &app.cfg.legacy_webhook_token) {
        return Err((StatusCode::UNAUTHORIZED, "Bearer token required".into()));
    }
    let targets = match (old.target_type.as_deref().unwrap_or("all"), old.target_dn) {
        ("all", _) => vec![],
        (kind @ ("group" | "ou" | "user"), Some(dn)) => vec![format!("{kind}:{dn}")],
        _ => return Err(bad("Unsupported target")),
    };
    let form = NewIncident {
        title: old.title,
        message: old.message,
        severity: old.severity,
        targets,
        starts_at: None,
        ends_at: None,
    };
    create(&app, form, "webhook")
}

// ---- startup ---------------------------------------------------------------------------

fn load_config() -> Config {
    let ad = env_bool("AD_ENABLED", false).then(|| ad::AdConfig {
        url: env_get("AD_URL", ""),
        bind_dn: env_get("AD_BIND_DN", ""),
        bind_password: env_get("AD_BIND_PASSWORD", ""),
        base_dn: env_get("AD_BASE_DN", ""),
        user_filter: env_get(
            "AD_USER_FILTER",
            "(&(objectCategory=person)(objectClass=user)(!(userAccountControl:1.2.840.113556.1.4.803:=2)))",
        ),
        admin_group: env_get("AD_ADMIN_GROUP", "Statusbar-Admins"),
        sync_seconds: env_num("AD_SYNC_SECONDS", 300u64).max(30),
    });
    let cfg = Config {
        admin_token: env_get("ADMIN_TOKEN", ""),
        client_token: env_get("CLIENT_TOKEN", ""),
        legacy_webhook_token: env_get("WEBHOOK_TOKEN", ""),
        session_hours: env_num("ADMIN_SESSION_HOURS", 8i64).clamp(1, 24),
        retention_days: env_num("RETENTION_DAYS", 90i64).clamp(1, 3650),
        locales_dir: env_get("LOCALES_DIR", "/data/locales"),
        client_msi: env_get("CLIENT_MSI", ""),
        ad,
    };
    for (name, secret) in [("ADMIN_TOKEN", &cfg.admin_token), ("CLIENT_TOKEN", &cfg.client_token)] {
        if secret.len() < 32 || secret.contains("CHANGE_ME") {
            panic!("Set a random {name} of at least 32 characters (openssl rand -hex 32)");
        }
    }
    if cfg.admin_token == cfg.client_token {
        panic!("ADMIN_TOKEN and CLIENT_TOKEN must differ");
    }
    if let Some(adc) = &cfg.ad {
        if adc.url.is_empty()
            || adc.base_dn.is_empty()
            || adc.bind_password.is_empty()
            || adc.bind_password.contains("CHANGE_ME")
        {
            panic!("AD_ENABLED=yes needs AD_URL, AD_BIND_DN, AD_BIND_PASSWORD and AD_BASE_DN");
        }
    }
    cfg
}

/// Stored settings win; anything not stored yet (first start, or display values after an
/// upgrade from a pre-0.5 database) falls back to the old BAR_* variables, then to defaults.
fn initial_settings(conn: &Connection) -> Settings {
    let d = Settings::default();
    let fallback = Settings {
        position: env_get("BAR_POSITION", &d.position).to_ascii_lowercase(),
        all_monitors: env_bool("BAR_ALL_MONITORS", d.all_monitors),
        height_px: env_num("BAR_HEIGHT_PX", d.height_px),
        initial_height_px: env_num("BAR_INITIAL_HEIGHT_PX", d.initial_height_px),
        initial_seconds: env_num("BAR_INITIAL_SECONDS", d.initial_seconds),
        resolved_seconds: env_num("RESOLVED_DISPLAY_SECONDS", d.resolved_seconds),
        ..d
    };
    let mut merged = json!(fallback);
    if let (Some(target), Ok(Some(Value::Object(stored)))) = (merged.as_object_mut(), store::load_settings_raw(conn)) {
        for (key, value) in stored {
            target.insert(key, value);
        }
    }
    serde_json::from_value(merged).unwrap_or(fallback)
}

async fn directory_loop(app: App, adc: ad::AdConfig) {
    loop {
        let cfg = adc.clone();
        match tokio::task::spawn_blocking(move || ad::sync(&cfg)).await {
            Ok(Ok(dir)) => {
                info!("directory synced: {} users, {} groups", dir.users.len(), dir.groups.len());
                *app.directory.write().unwrap() = dir;
            }
            other => {
                let problem = match other {
                    Ok(Err(e)) => e,
                    _ => "sync task failed".to_string(),
                };
                warn!("directory sync failed: {problem}");
                app.directory.write().unwrap().last_error = Some(problem);
            }
        }
        tokio::time::sleep(Duration::from_secs(adc.sync_seconds)).await;
    }
}

async fn housekeeping_loop(app: App) {
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let result = { store::housekeeping(&app.db.lock().unwrap(), app.cfg.retention_days) };
        if let Err(e) = result {
            error!("housekeeping failed: {e}");
        }
        // Also picks up time-based changes: delayed starts, scheduled ends, expiring all-clears.
        refresh(&app);
    }
}

#[tokio::main]
async fn main() {
    let addr: SocketAddr = env_get("LISTEN_ADDR", "0.0.0.0:8080").parse().expect("LISTEN_ADDR");
    if env::args().any(|a| a == "--healthcheck") {
        let probe = SocketAddr::from(([127, 0, 0, 1], addr.port()));
        std::process::exit(if std::net::TcpStream::connect_timeout(&probe, Duration::from_secs(3)).is_ok() {
            0
        } else {
            1
        });
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cfg = load_config();
    let conn = store::open(&env_get("DATABASE_PATH", "/data/statusbar.sqlite")).expect("open database");
    let settings = sanitize(initial_settings(&conn)).unwrap_or_default();
    store::save_settings(&conn, &settings).expect("store settings");
    let live = store::live_incidents(&conn, settings.resolved_seconds).expect("load incidents");
    let ad_cfg = cfg.ad.clone();
    let app: App = Arc::new(Inner {
        cfg,
        db: Mutex::new(conn),
        settings: RwLock::new(settings),
        live: RwLock::new(Arc::new(live)),
        // Start from the clock so revisions stay unique across restarts.
        rev: watch::channel(now() as u64).0,
        directory: RwLock::new(ad::Directory::default()),
        sessions: Mutex::new(HashMap::new()),
    });
    if let Some(adc) = ad_cfg {
        tokio::spawn(directory_loop(app.clone(), adc));
    }
    tokio::spawn(housekeeping_loop(app.clone()));

    let mut router = Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/health", get(|| async { "ok" }))
        .route("/api/meta", get(meta))
        .route("/api/logo", get(logo))
        .route("/api/locales/{name}", get(locale))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/me", get(me))
        .route("/api/client/state", get(client_state))
        .route("/api/hooks", post(hook_header))
        .route("/api/hooks/{token}", post(hook_path))
        .route("/api/admin/incidents", get(list_incidents).post(create_incident))
        .route("/api/admin/incidents/{id}", delete(delete_incident))
        .route("/api/admin/incidents/{id}/resolve", post(resolve_incident))
        .route("/api/admin/incidents/{id}/publish", post(publish_incident))
        .route("/api/admin/settings", get(get_settings).put(put_settings))
        .route("/api/admin/logo", put(put_logo).delete(delete_logo))
        .route("/api/admin/targets", get(targets))
        .route("/api/admin/deploy", get(deploy))
        .route("/api/admin/client.msi", get(client_msi))
        .route("/api/admin/client-package.zip", get(client_package))
        .route("/api/admin/integrations", get(list_integrations).post(create_integration))
        .route("/api/admin/integrations/{id}", delete(delete_integration))
        .route("/api/admin/rules", post(create_rule))
        .route("/api/admin/rules/{id}", delete(delete_rule));
    if !app.cfg.legacy_webhook_token.is_empty() {
        router = router.route("/api/webhooks/incidents", post(legacy_webhook));
    }
    let router = router.with_state(app);

    tokio::spawn(async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut term = signal(SignalKind::terminate()).expect("signal handler");
            tokio::select! { _ = term.recv() => {}, _ = tokio::signal::ctrl_c() => {} }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        info!("shutting down");
        std::process::exit(0);
    });

    info!("statusbar-server {} listening on {addr}", env!("CARGO_PKG_VERSION"));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, router).await.expect("serve");
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
    fn no_targets_means_everyone() {
        assert!(visible(&incident(&[]), "", "", None));
    }

    #[test]
    fn host_patterns_work_without_a_directory() {
        let i = incident(&["host:PC-ACC-*"]);
        assert!(visible(&i, "anna", "pc-acc-07", None));
        assert!(!visible(&i, "anna", "PC-HR-01", None));
        assert!(!visible(&i, "anna", "", None));
    }

    #[test]
    fn directory_targets_need_a_known_person() {
        let p = ad::Person {
            dn: "CN=Anna,OU=Finance,DC=example,DC=local".into(),
            groups: ["cn=finance,dc=example,dc=local".to_string()].into_iter().collect(),
        };
        let group = incident(&["group:CN=Finance,DC=example,DC=local"]);
        assert!(visible(&group, "anna", "pc", Some(&p)));
        assert!(!visible(&group, "anna", "pc", None));
        let ou = incident(&["ou:OU=Finance,DC=example,DC=local"]);
        assert!(visible(&ou, "anna", "pc", Some(&p)));
        let user = incident(&["user:Anna", "host:srv-*"]);
        assert!(visible(&user, "anna", "pc", Some(&p)));
        assert!(!visible(&user, "bob", "pc", Some(&p)));
    }

    #[test]
    fn install_script_is_filled_in_and_guarded() {
        let (install, _, _) = install_scripts("https://status.example.com", "abc123");
        assert!(install.contains("SERVERURL=https://status.example.com CLIENTTOKEN=abc123 /qn"));
        assert!(install.contains("\r\n"));
        assert!(script_safe("https://status.example.com:8443"));
        assert!(!script_safe("https://x.example & calc"));
        assert!(!script_safe("https://x.example/%PATH%"));
    }

    #[test]
    fn settings_are_clamped() {
        let s =
            sanitize(Settings { height_px: 5, initial_height_px: 1, position: "left".into(), ..Settings::default() })
                .unwrap();
        assert_eq!((s.height_px, s.initial_height_px, s.position.as_str()), (24, 24, "top"));
        assert!(sanitize(Settings { color: "red".into(), ..Settings::default() }).is_err());
    }
}
