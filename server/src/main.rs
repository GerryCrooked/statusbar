use axum::{extract::{DefaultBodyLimit, Multipart, Path, Query, State}, http::{header, HeaderMap, StatusCode, HeaderValue}, response::{Html, IntoResponse, Response}, routing::{get, post, put, delete}, Json, Router};
use chrono::{DateTime, Duration, Utc};
use ldap3::{LdapConn, LdapConnSettings, Scope, SearchEntry};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, env, net::SocketAddr, sync::{Arc, RwLock, Mutex}, time::Duration as StdDuration};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

#[derive(Clone)]
struct Config {
    db: String, admin: String, webhook: String, client: String,
    ad_enabled: bool, ad_url: String, ad_bind: String, ad_password: String,
    ad_base: String, ad_filter: String, ad_sync: u64, resolved_seconds: i64,
    admin_group: String, session_hours: i64,
}
#[derive(Clone, Default, Debug)]
struct Directory { users: HashMap<String, Person>, groups: Vec<String>, ous: Vec<String>, last_sync: Option<DateTime<Utc>>, last_error: Option<String> }
#[derive(Clone, Default, Debug)]
struct Person { dn: String, groups: HashSet<String> }
#[derive(Clone)]
struct App { cfg: Config, directory: Arc<RwLock<Directory>>, sessions: Arc<Mutex<HashMap<String, Session>>> }
#[derive(Clone)]
struct Session { username: String, expires: DateTime<Utc> }
#[derive(Deserialize)]
struct LoginForm { username: String, password: String }
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Incident {
    id: String, title: String, message: String, severity: String, target_type: String, target_dn: String,
    status: String, created_at: String, resolved_at: Option<String>,
}
#[derive(Deserialize)]
struct NewIncident { title: String, message: String, severity: Option<String>, target_type: String, target_dn: Option<String> }
#[derive(Deserialize)]
struct ClientQuery { user: Option<String>, computer: Option<String> }
#[derive(Serialize)]
struct ClientResponse { incidents: Vec<Incident>, resolved_display_seconds: i64, directory_ready: bool }
#[derive(Serialize)]
struct StateResponse { directory_ready: bool, ad_enabled: bool, users: usize, groups: usize, last_sync: Option<String>, last_error: Option<String> }
#[derive(Serialize)]
struct TargetsResponse { groups: Vec<String>, ous: Vec<String>, custom_groups: Vec<CustomGroup> }
#[derive(Serialize, Deserialize, Clone)]
struct CustomGroup { id: String, name: String, ad_groups: Vec<String> }
#[derive(Deserialize)]
struct CustomGroupForm { name: String, ad_groups: Vec<String> }
#[derive(Serialize, Deserialize)]
struct Branding { name: String, subtitle: String, color: String, has_logo: bool, language: String }
#[derive(Deserialize)]
struct BrandingForm { name: String, subtitle: String, color: String, language: String }
#[derive(Serialize,Deserialize)]
struct TranslationPackage { locale: String, name: String, author: String, source: String, version: String, license: String, strings: HashMap<String,String> }
#[derive(Serialize)]
struct LanguageItem { locale: String, name: String, author: String, source: String, version: String, license: String, built_in: bool }
type HttpError = (StatusCode, String);

fn env_get(key: &str, default: &str) -> String { env::var(key).unwrap_or_else(|_| default.to_owned()) }
fn config() -> Config {
    Config {
        db: env_get("DATABASE_PATH", "/data/status.sqlite"),
        admin: env_get("ADMIN_TOKEN", ""), webhook: env_get("WEBHOOK_TOKEN", ""), client: env_get("CLIENT_TOKEN", ""),
        ad_enabled: env_get("AD_ENABLED", "false").eq_ignore_ascii_case("true"),
        ad_url: env_get("AD_URL", ""), ad_bind: env_get("AD_BIND_DN", ""), ad_password: env_get("AD_BIND_PASSWORD", ""),
        ad_base: env_get("AD_BASE_DN", ""),
        ad_filter: env_get("AD_USER_FILTER", "(&(objectCategory=person)(objectClass=user)(!(userAccountControl:1.2.840.113556.1.4.803:=2)))"),
        ad_sync: env_get("AD_SYNC_SECONDS", "300").parse::<u64>().unwrap_or(300).max(30),
        resolved_seconds: env_get("RESOLVED_DISPLAY_SECONDS", "90").parse::<i64>().unwrap_or(90).clamp(0, 3600),
        admin_group: env_get("AD_ADMIN_GROUP", "Serveradmin"),
        session_hours: env_get("ADMIN_SESSION_HOURS", "8").parse::<i64>().unwrap_or(8).clamp(1, 24),
    }
}
fn connect(db: &str) -> Result<Connection, String> { Connection::open(db).map_err(|e| e.to_string()) }
fn prepare_db(db: &str) -> Result<(), String> {
    let conn = connect(db)?;
    conn.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS incidents (
      id TEXT PRIMARY KEY, title TEXT NOT NULL, message TEXT NOT NULL,
      severity TEXT NOT NULL, target_type TEXT NOT NULL, target_dn TEXT NOT NULL,
      status TEXT NOT NULL, created_at TEXT NOT NULL, resolved_at TEXT
    ); CREATE INDEX IF NOT EXISTS idx_incidents_status ON incidents(status);
    CREATE TABLE IF NOT EXISTS custom_groups (
      id TEXT PRIMARY KEY, name TEXT NOT NULL COLLATE NOCASE UNIQUE,
      created_at TEXT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS custom_group_members (
      group_id TEXT NOT NULL REFERENCES custom_groups(id) ON DELETE CASCADE,
      ad_group_dn TEXT NOT NULL COLLATE NOCASE,
      PRIMARY KEY(group_id, ad_group_dn)
    );
    CREATE TABLE IF NOT EXISTS branding (
      id INTEGER PRIMARY KEY CHECK(id = 1),
      name TEXT NOT NULL, subtitle TEXT NOT NULL, color TEXT NOT NULL,
      logo BLOB, logo_mime TEXT
    );
    INSERT OR IGNORE INTO branding(id,name,subtitle,color) VALUES (1,'VG Statusbar','IT incident notifications','#173451');
    CREATE TABLE IF NOT EXISTS translations (
      locale TEXT PRIMARY KEY, package_json TEXT NOT NULL
    );")
        .map_err(|e| e.to_string())?;
    // Existing v0.3 databases lack the language column; migrate without losing settings.
    let has_language={
        let mut st=conn.prepare("PRAGMA table_info(branding)").map_err(|e|e.to_string())?;
        st.query_map([],|r|r.get::<_,String>(1)).map_err(|e|e.to_string())?
          .collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?
          .contains(&"language".to_string())
    };
    if !has_language {conn.execute("ALTER TABLE branding ADD COLUMN language TEXT NOT NULL DEFAULT 'en'",[]).map_err(|e|e.to_string())?;}
    Ok(())
}
fn list_incidents(db: &str) -> Result<Vec<Incident>, String> {
    let conn = connect(db)?;
    let mut stmt = conn.prepare("SELECT id,title,message,severity,target_type,target_dn,status,created_at,resolved_at FROM incidents ORDER BY created_at DESC").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(Incident { id:r.get(0)?, title:r.get(1)?, message:r.get(2)?, severity:r.get(3)?, target_type:r.get(4)?, target_dn:r.get(5)?, status:r.get(6)?, created_at:r.get(7)?, resolved_at:r.get(8)? })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
fn load_custom_groups(db: &str) -> Result<Vec<CustomGroup>, String> {
    let conn=connect(db)?;
    let mut stmt=conn.prepare("SELECT id,name FROM custom_groups ORDER BY name COLLATE NOCASE").map_err(|e|e.to_string())?;
    let rows=stmt.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?;
    let mut result=Vec::new();
    for row in rows {
        let (id,name)=row.map_err(|e|e.to_string())?;
        let mut members=conn.prepare("SELECT ad_group_dn FROM custom_group_members WHERE group_id=?1 ORDER BY ad_group_dn").map_err(|e|e.to_string())?;
        let values=members.query_map([&id],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?
            .collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        result.push(CustomGroup {id,name,ad_groups:values});
    }
    Ok(result)
}
fn load_branding(db: &str) -> Result<Branding,String> {
    let conn=connect(db)?;
    conn.query_row("SELECT name,subtitle,color,logo IS NOT NULL,language FROM branding WHERE id=1",[],|r|Ok(Branding{name:r.get(0)?,subtitle:r.get(1)?,color:r.get(2)?,has_logo:r.get(3)?,language:r.get(4)?})).map_err(|e|e.to_string())
}
fn valid_color(s:&str)->bool {s.len()==7 && s.starts_with('#') && s[1..].bytes().all(|b|b.is_ascii_hexdigit())}
fn constant_equal(a: &str,b: &str) -> bool { a.len() == b.len() && bool::from(a.as_bytes().ct_eq(b.as_bytes())) }
fn authorize(headers: &HeaderMap, token: &str) -> Result<(), HttpError> {
    let candidate = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).unwrap_or("");
    if !candidate.is_empty() && constant_equal(candidate, token) { Ok(()) } else { Err((StatusCode::UNAUTHORIZED,"Bearer token required".into())) }
}
fn ad_admin(person: &Person, group: &str) -> bool {
    // Matching by complete CN component, not substring (Serveradmin2 must not match).
    let expected = format!("cn={},", group.to_lowercase());
    person.groups.iter().any(|dn| dn.starts_with(&expected))
}
fn cookie_session(headers: &HeaderMap) -> Option<String> {
    headers.get(header::COOKIE)?.to_str().ok()?
        .split(';').map(str::trim)
        .find_map(|c| c.strip_prefix("vg_session=").map(str::to_owned))
}
fn session_user(state: &App, headers: &HeaderMap) -> Result<String, HttpError> {
    let sid = cookie_session(headers).ok_or((StatusCode::UNAUTHORIZED, "Login required".into()))?;
    let sessions = state.sessions.lock().map_err(internal)?;
    let session = sessions.get(&sid).ok_or((StatusCode::UNAUTHORIZED, "Login required".into()))?;
    if session.expires < Utc::now() {return Err((StatusCode::UNAUTHORIZED, "Session expired".into()))}
    let d = state.directory.read().map_err(internal)?;
    let person = d.users.get(&session.username).ok_or((StatusCode::FORBIDDEN, "AD account unavailable".into()))?;
    if !ad_admin(person, &state.cfg.admin_group) {return Err((StatusCode::FORBIDDEN, "Serveradmin membership required".into()))}
    Ok(session.username.clone())
}
fn admin_authorize(state: &App, headers: &HeaderMap) -> Result<(), HttpError> {
    if authorize(headers, &state.cfg.admin).is_ok() {return Ok(())}
    session_user(state, headers).map(|_| ())
}
fn ldap_connect(cfg: &Config) -> Result<LdapConn, String> {
    let settings = LdapConnSettings::new().set_conn_timeout(StdDuration::from_secs(10));
    let settings = if cfg.ad_url.starts_with("ldap://") {
        settings.set_starttls(true)
    } else if cfg.ad_url.starts_with("ldaps://") {
        settings
    } else {
        return Err("AD_URL must use ldap:// with StartTLS or ldaps://".into());
    };
    // STARTTLS is negotiated by ldap3 before any simple bind; TLS certificate verification stays enabled.
    LdapConn::with_settings(settings, &cfg.ad_url).map_err(|e| e.to_string())
}
async fn login(State(state): State<App>, Json(payload): Json<LoginForm>) -> Result<impl IntoResponse, HttpError> {
    if !state.cfg.ad_enabled {return Err((StatusCode::SERVICE_UNAVAILABLE, "AD disabled".into()))}
    let username = payload.username.trim().split('\\').next_back().unwrap_or("").to_lowercase();
    if username.is_empty() || username.len() > 128 || payload.password.is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials or permissions".into()));
    }
    let dn = {
        let d = state.directory.read().map_err(internal)?;
        let person = d.users.get(&username).ok_or((StatusCode::UNAUTHORIZED, "Invalid credentials or permissions".into()))?;
        if !ad_admin(person, &state.cfg.admin_group) {return Err((StatusCode::UNAUTHORIZED, "Invalid credentials or permissions".into()))}
        person.dn.clone()
    };
    let cfg = state.cfg.clone();
    let password = payload.password;
    let valid = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut ldap = ldap_connect(&cfg)?;
        let result = ldap.simple_bind(&dn, &password).map_err(|e| e.to_string())?
            .success().map_err(|e| e.to_string());
        ldap.unbind().ok();
        result.map(|_| ())
    }).await.map_err(internal)?;
    if valid.is_err() {return Err((StatusCode::UNAUTHORIZED, "Invalid credentials or permissions".into()))}
    let sid = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    {
        let mut sessions = state.sessions.lock().map_err(internal)?;
        sessions.retain(|_, v| v.expires > Utc::now());
        sessions.insert(sid.clone(), Session {username: username.clone(), expires: Utc::now() + Duration::hours(state.cfg.session_hours)});
    }
    let mut response = Json(serde_json::json!({"ok":true,"username":username})).into_response();
    response.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_str(
        &format!("vg_session={sid}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age={}", state.cfg.session_hours * 3600)
    ).map_err(internal)?);
    Ok(response)
}
async fn me(State(state): State<App>, headers: HeaderMap) -> Result<Json<serde_json::Value>, HttpError> {
    let username = session_user(&state, &headers)?;
    Ok(Json(serde_json::json!({"username":username,"admin":true})))
}
async fn logout(State(state): State<App>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(sid) = cookie_session(&headers) {
        if let Ok(mut sessions) = state.sessions.lock() {sessions.remove(&sid);}
    }
    let mut response=Json(serde_json::json!({"ok":true})).into_response();
    response.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_static("vg_session=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0"));
    response
}
fn validate_new(i: &NewIncident, dir: &Directory, ad_enabled: bool) -> Result<(String,String,String),HttpError> {
    if i.title.trim().is_empty() || i.title.len()>120 || i.message.trim().is_empty() || i.message.len()>1500 {
        return Err((StatusCode::BAD_REQUEST,"Title 1-120 chars, message 1-1500 chars".into()));
    }
    let severity=i.severity.as_deref().unwrap_or("error").to_lowercase();
    if !["error","warning","info"].contains(&severity.as_str()) { return Err((StatusCode::BAD_REQUEST,"Invalid severity".into())) }
    let t=i.target_type.to_lowercase();
    if !["all","group","ou","user","custom"].contains(&t.as_str()) { return Err((StatusCode::BAD_REQUEST,"Invalid target_type".into())) }
    let dn = i.target_dn.as_deref().unwrap_or("").trim().to_string();
    if t!="all" && (!ad_enabled || dir.last_sync.is_none()) { return Err((StatusCode::SERVICE_UNAVAILABLE,"AD directory not synchronized".into())) }
    if t!="all" && dn.is_empty() { return Err((StatusCode::BAD_REQUEST,"target_dn required".into())) }
    if t=="group" && !dir.groups.iter().any(|x|x.eq_ignore_ascii_case(&dn)) { return Err((StatusCode::BAD_REQUEST,"Unknown AD group DN".into())) }
    if t=="ou" && !dir.ous.iter().any(|x|x.eq_ignore_ascii_case(&dn)) { return Err((StatusCode::BAD_REQUEST,"Unknown OU DN".into())) }
    if t=="user" && !dir.users.contains_key(&dn.to_lowercase()) { return Err((StatusCode::BAD_REQUEST,"Unknown sAMAccountName".into())) }
    Ok((severity,t,dn))
}
async fn create_incident(State(state):State<App>, headers: HeaderMap, Json(payload): Json<NewIncident>) -> Result<Json<Incident>,HttpError> {
    // Admin and webhook tokens may both create, but only admin can resolve/delete.
    if admin_authorize(&state,&headers).is_err() { authorize(&headers,&state.cfg.webhook)?; }
    let (severity,t,dn) = {
        let d=state.directory.read().map_err(|_|(StatusCode::INTERNAL_SERVER_ERROR,"lock poisoned".into()))?;
        validate_new(&payload,&d,state.cfg.ad_enabled)?
    };
    if t=="custom" {
        let db=state.cfg.db.clone();
        let target=dn.clone();
        let exists=tokio::task::spawn_blocking(move||load_custom_groups(&db).map(|groups|groups.iter().any(|g|g.id==target))).await.map_err(internal)?.map_err(internal)?;
        if !exists {return Err((StatusCode::BAD_REQUEST,"Unknown custom group".into()))}
    }
    let incident=Incident { id:uuid::Uuid::new_v4().to_string(), title:payload.title.trim().into(), message:payload.message.trim().into(), severity, target_type:t, target_dn:dn, status:"active".into(), created_at:Utc::now().to_rfc3339(), resolved_at:None };
    let i=incident.clone(); let db=state.cfg.db.clone();
    tokio::task::spawn_blocking(move || -> Result<(),String> {
        let c=connect(&db)?;
        c.execute("INSERT INTO incidents VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![i.id,i.title,i.message,i.severity,i.target_type,i.target_dn,i.status,i.created_at,i.resolved_at]).map_err(|e|e.to_string())?;
        Ok(())
    }).await.map_err(internal)?.map_err(internal)?;
    Ok(Json(incident))
}
async fn resolve_incident(State(state):State<App>, headers: HeaderMap, Path(id): Path<String>) -> Result<Json<serde_json::Value>,HttpError> {
    admin_authorize(&state,&headers)?;
    let db=state.cfg.db.clone();
    let n=tokio::task::spawn_blocking(move || -> Result<usize,String> {
        let c=connect(&db)?;
        c.execute("UPDATE incidents SET status='resolved',resolved_at=?1 WHERE id=?2 AND status='active'",params![Utc::now().to_rfc3339(),id]).map_err(|e|e.to_string())
    }).await.map_err(internal)?.map_err(internal)?;
    if n==0 {return Err((StatusCode::NOT_FOUND,"Active incident not found".into()))}
    Ok(Json(serde_json::json!({"ok":true})))
}
async fn get_incidents(State(state):State<App>,headers:HeaderMap)->Result<Json<Vec<Incident>>,HttpError> {
    admin_authorize(&state,&headers)?;
    let db=state.cfg.db.clone();
    let entries=tokio::task::spawn_blocking(move||list_incidents(&db)).await.map_err(internal)?.map_err(internal)?;
    Ok(Json(entries))
}
fn visible(i: &Incident, person: Option<&Person>, user: &str, ad_enabled: bool, custom: &[CustomGroup]) -> bool {
    if i.target_type=="all" {return true}
    if !ad_enabled {return false}
    let Some(p)=person else{return false};
    match i.target_type.as_str() {
        "user"=> user.eq_ignore_ascii_case(&i.target_dn),
        "group"=>p.groups.contains(&i.target_dn.to_lowercase()),
        "custom"=>custom.iter().find(|g|g.id==i.target_dn).is_some_and(|g|g.ad_groups.iter().any(|dn|p.groups.contains(&dn.to_lowercase()))),
        "ou"=>p.dn.to_lowercase().ends_with(&format!(",{}",i.target_dn.to_lowercase())),
        _=>false,
    }
}
async fn client_state(State(state):State<App>,headers:HeaderMap,Query(query):Query<ClientQuery>)->Result<Json<ClientResponse>,HttpError> {
    authorize(&headers,&state.cfg.client)?;
    let username=query.user.as_deref().unwrap_or("").split('\\').next_back().unwrap_or("").to_lowercase();
    let _computer=query.computer.as_deref().unwrap_or("");
    let (person,ready)={let d=state.directory.read().map_err(|_|(StatusCode::INTERNAL_SERVER_ERROR,"lock".into()))?; (d.users.get(&username).cloned(),d.last_sync.is_some())};
    let db=state.cfg.db.clone();
    let (entries,custom)=tokio::task::spawn_blocking(move||Ok::<_,String>((list_incidents(&db)?,load_custom_groups(&db)?))).await.map_err(internal)?.map_err(internal)?;
    let now=Utc::now();
    let entries=entries.into_iter().filter(|i|{
        (i.status=="active" || i.resolved_at.as_ref().and_then(|dt|DateTime::parse_from_rfc3339(dt).ok()).is_some_and(|dt| now.signed_duration_since(dt.with_timezone(&Utc))<Duration::seconds(state.cfg.resolved_seconds)))
        && visible(i,person.as_ref(),&username,state.cfg.ad_enabled,&custom)
    }).collect();
    Ok(Json(ClientResponse {incidents:entries,resolved_display_seconds:state.cfg.resolved_seconds,directory_ready:ready}))
}
async fn targets(State(state):State<App>,headers:HeaderMap)->Result<Json<TargetsResponse>,HttpError> {
    admin_authorize(&state,&headers)?;
    let d=state.directory.read().map_err(|_|(StatusCode::INTERNAL_SERVER_ERROR,"lock".into()))?;
    let groups=d.groups.clone(); let ous=d.ous.clone(); drop(d);
    let db=state.cfg.db.clone();
    let custom_groups=tokio::task::spawn_blocking(move||load_custom_groups(&db)).await.map_err(internal)?.map_err(internal)?;
    Ok(Json(TargetsResponse{groups,ous,custom_groups}))
}
async fn directory_status(State(state):State<App>,headers:HeaderMap)->Result<Json<StateResponse>,HttpError>{
    admin_authorize(&state,&headers)?;
    let d=state.directory.read().map_err(|_|(StatusCode::INTERNAL_SERVER_ERROR,"lock".into()))?;
    Ok(Json(StateResponse {directory_ready:d.last_sync.is_some(),ad_enabled:state.cfg.ad_enabled,users:d.users.len(),groups:d.groups.len(),last_sync:d.last_sync.map(|v|v.to_rfc3339()),last_error:d.last_error.clone()}))
}
async fn custom_groups_list(State(state):State<App>,headers:HeaderMap)->Result<Json<Vec<CustomGroup>>,HttpError>{
    admin_authorize(&state,&headers)?;
    let db=state.cfg.db.clone();
    let rows=tokio::task::spawn_blocking(move||load_custom_groups(&db)).await.map_err(internal)?.map_err(internal)?;
    Ok(Json(rows))
}
async fn save_custom_group(State(state):State<App>,headers:HeaderMap,Json(form):Json<CustomGroupForm>)->Result<Json<CustomGroup>,HttpError>{
    admin_authorize(&state,&headers)?;
    let name=form.name.trim().to_string();
    if name.is_empty() || name.chars().count()>80 || name.chars().any(char::is_control) || form.ad_groups.is_empty() || form.ad_groups.len()>100 {
        return Err((StatusCode::BAD_REQUEST,"Name (1-80 chars) and 1-100 AD groups required".into()));
    }
    let members:Vec<String>={
        let d=state.directory.read().map_err(internal)?;
        let mut set=HashSet::new();
        for requested in &form.ad_groups {
            let canonical=d.groups.iter().find(|g|g.eq_ignore_ascii_case(requested)).ok_or((StatusCode::BAD_REQUEST,"Unknown AD group".into()))?;
            set.insert(canonical.clone());
        }
        set.into_iter().collect()
    };
    let db=state.cfg.db.clone();
    let group=tokio::task::spawn_blocking(move||->Result<CustomGroup,String>{
        let mut conn=connect(&db)?;
        conn.execute_batch("PRAGMA foreign_keys=ON").map_err(|e|e.to_string())?;
        let tx=conn.transaction().map_err(|e|e.to_string())?;
        let id=uuid::Uuid::new_v4().to_string();
        tx.execute("INSERT INTO custom_groups(id,name,created_at) VALUES (?1,?2,?3)",params![id,name,Utc::now().to_rfc3339()]).map_err(|e|e.to_string())?;
        for m in &members {tx.execute("INSERT INTO custom_group_members(group_id,ad_group_dn) VALUES (?1,?2)",params![id,m]).map_err(|e|e.to_string())?;}
        tx.commit().map_err(|e|e.to_string())?;
        Ok(CustomGroup{id,name,ad_groups:members})
    }).await.map_err(internal)?.map_err(|e|(StatusCode::CONFLICT,format!("Could not create group (name already exists?): {e}")))?;
    Ok(Json(group))
}
async fn update_custom_group(State(state):State<App>,headers:HeaderMap,Path(id):Path<String>,Json(form):Json<CustomGroupForm>)->Result<Json<CustomGroup>,HttpError>{
    admin_authorize(&state,&headers)?;
    let name=form.name.trim().to_string();
    if name.is_empty() || name.chars().count()>80 || name.chars().any(char::is_control) || form.ad_groups.is_empty() || form.ad_groups.len()>100 {
        return Err((StatusCode::BAD_REQUEST,"Name (1-80 chars) and 1-100 AD groups required".into()));
    }
    let members={
        let d=state.directory.read().map_err(internal)?;
        let mut set=HashSet::new();
        for requested in &form.ad_groups {let canonical=d.groups.iter().find(|g|g.eq_ignore_ascii_case(requested)).ok_or((StatusCode::BAD_REQUEST,"Unknown AD group".into()))?;set.insert(canonical.clone());}
        set.into_iter().collect::<Vec<_>>()
    };
    let db=state.cfg.db.clone();
    let group=tokio::task::spawn_blocking(move||->Result<Option<CustomGroup>,String>{
        let mut conn=connect(&db)?;
        conn.execute_batch("PRAGMA foreign_keys=ON").map_err(|e|e.to_string())?;
        let tx=conn.transaction().map_err(|e|e.to_string())?;
        let updated=tx.execute("UPDATE custom_groups SET name=?1 WHERE id=?2",params![name,id]).map_err(|e|e.to_string())?;
        if updated==0 {return Ok(None)}
        tx.execute("DELETE FROM custom_group_members WHERE group_id=?1",[&id]).map_err(|e|e.to_string())?;
        for m in &members {tx.execute("INSERT INTO custom_group_members(group_id,ad_group_dn) VALUES (?1,?2)",params![id,m]).map_err(|e|e.to_string())?;}
        tx.commit().map_err(|e|e.to_string())?;
        Ok(Some(CustomGroup{id,name,ad_groups:members}))
    }).await.map_err(internal)?.map_err(|e|(StatusCode::CONFLICT,format!("Update failed: {e}")))?;
    group.map(Json).ok_or((StatusCode::NOT_FOUND,"Custom group not found".into()))
}
async fn delete_custom_group(State(state):State<App>,headers:HeaderMap,Path(id):Path<String>)->Result<Json<serde_json::Value>,HttpError>{
    admin_authorize(&state,&headers)?;
    let db=state.cfg.db.clone();
    let result=tokio::task::spawn_blocking(move||->Result<usize,String>{
        let mut conn=connect(&db)?;
        conn.execute_batch("PRAGMA foreign_keys=ON").map_err(|e|e.to_string())?;
        let tx=conn.transaction().map_err(|e|e.to_string())?;
        let active:i64=tx.query_row("SELECT COUNT(*) FROM incidents WHERE target_type='custom' AND target_dn=?1 AND status='active'",[&id],|r|r.get(0)).map_err(|e|e.to_string())?;
        if active>0 {return Err("ACTIVE_INCIDENTS".into())}
        let n=tx.execute("DELETE FROM custom_groups WHERE id=?1",[&id]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e|e.to_string())?;
        Ok(n)
    }).await.map_err(internal)?;
    match result {
        Err(e) if e=="ACTIVE_INCIDENTS"=>Err((StatusCode::CONFLICT,"Resolve active incidents targeting this group first".into())),
        Err(e)=>Err(internal(e)),
        Ok(0)=>Err((StatusCode::NOT_FOUND,"Custom group not found".into())),
        Ok(_)=>Ok(Json(serde_json::json!({"ok":true})))
    }
}
async fn public_branding(State(state):State<App>)->Result<Json<Branding>,HttpError>{
    let db=state.cfg.db.clone();
    Ok(Json(tokio::task::spawn_blocking(move||load_branding(&db)).await.map_err(internal)?.map_err(internal)?))
}
async fn set_branding(State(state):State<App>,headers:HeaderMap,Json(form):Json<BrandingForm>)->Result<Json<Branding>,HttpError>{
    admin_authorize(&state,&headers)?;
    let name=form.name.trim().to_string();let subtitle=form.subtitle.trim().to_string();let color=form.color.trim().to_string();let language=form.language.trim().to_ascii_lowercase();
    if name.is_empty() || name.chars().count()>70 || subtitle.chars().count()>140 || !valid_color(&color) || !valid_locale(&language) || name.chars().chain(subtitle.chars()).any(char::is_control) {
        return Err((StatusCode::BAD_REQUEST,"Invalid branding fields".into()))
    }
    let db=state.cfg.db.clone();
    let langs_db=db.clone();
    let language_available=tokio::task::spawn_blocking(move||available_locale(&langs_db,&language)).await.map_err(internal)?.map_err(internal)?;
    if !language_available {return Err((StatusCode::BAD_REQUEST,"Translation language is not installed".into()))}
    let language=form.language.trim().to_ascii_lowercase();
    let b=tokio::task::spawn_blocking(move||->Result<Branding,String>{
        let conn=connect(&db)?;
        conn.execute("UPDATE branding SET name=?1,subtitle=?2,color=?3,language=?4 WHERE id=1",params![name,subtitle,color,language]).map_err(|e|e.to_string())?;
        load_branding(&db)
    }).await.map_err(internal)?.map_err(internal)?;
    Ok(Json(b))
}
async fn upload_logo(State(state):State<App>,headers:HeaderMap,mut multipart:Multipart)->Result<Json<Branding>,HttpError>{
    admin_authorize(&state,&headers)?;
    let field=multipart.next_field().await.map_err(|_|(StatusCode::BAD_REQUEST,"Invalid upload".into()))?.ok_or((StatusCode::BAD_REQUEST,"Image required".into()))?;
    if field.name()!=Some("logo") {return Err((StatusCode::BAD_REQUEST,"Expected logo field".into()))}
    let data=field.bytes().await.map_err(|_|(StatusCode::BAD_REQUEST,"Invalid image".into()))?;
    if data.len()>1_000_000 || data.is_empty(){return Err((StatusCode::BAD_REQUEST,"Maximum logo size: 1 MB".into()))}
    let mime=if data.starts_with(&[0x89,0x50,0x4e,0x47,0x0d,0x0a,0x1a,0x0a]) {"image/png"}
        else if data.starts_with(&[0xff,0xd8,0xff]) {"image/jpeg"}
        else if data.len()>12 && &data[..4]==b"RIFF" && &data[8..12]==b"WEBP" {"image/webp"}
        else {return Err((StatusCode::BAD_REQUEST,"Only PNG, JPEG or WebP images are supported".into()))};
    let db=state.cfg.db.clone();let data=data.to_vec();
    Ok(Json(tokio::task::spawn_blocking(move||->Result<Branding,String>{
        let conn=connect(&db)?;
        conn.execute("UPDATE branding SET logo=?1,logo_mime=?2 WHERE id=1",params![data,mime]).map_err(|e|e.to_string())?;
        load_branding(&db)
    }).await.map_err(internal)?.map_err(internal)?))
}
async fn clear_logo(State(state):State<App>,headers:HeaderMap)->Result<Json<Branding>,HttpError>{
    admin_authorize(&state,&headers)?;
    let db=state.cfg.db.clone();
    Ok(Json(tokio::task::spawn_blocking(move||->Result<Branding,String>{
        let conn=connect(&db)?;conn.execute("UPDATE branding SET logo=NULL,logo_mime=NULL WHERE id=1",[]).map_err(|e|e.to_string())?;
        load_branding(&db)
    }).await.map_err(internal)?.map_err(internal)?))
}
async fn branding_logo(State(state):State<App>)->Result<Response,HttpError>{
    let db=state.cfg.db.clone();
    let image=tokio::task::spawn_blocking(move||->Result<Option<(Vec<u8>,String)>,String>{
        let conn=connect(&db)?;
        conn.query_row("SELECT logo,logo_mime FROM branding WHERE id=1",[],|r|Ok((r.get::<_,Option<Vec<u8>>>(0)?,r.get::<_,Option<String>>(1)?)))
            .map(|(bytes,mime)|bytes.zip(mime)).map_err(|e|e.to_string())
    }).await.map_err(internal)?.map_err(internal)?;
    let (data,mime)=image.ok_or((StatusCode::NOT_FOUND,"No logo".into()))?;
    Ok(([(header::CONTENT_TYPE,mime),(header::CACHE_CONTROL,"no-store".into())],data).into_response())
}
fn valid_locale(s:&str)->bool {
    let n=s.len();
    (2..=16).contains(&n) && s.bytes().all(|b|b.is_ascii_lowercase() || b.is_ascii_digit() || b==b'-')
}
fn available_locale(db:&str, locale:&str)->Result<bool,String>{
    if locale=="en" || locale=="de" {return Ok(true)}
    let conn=connect(db)?;
    conn.query_row("SELECT EXISTS(SELECT 1 FROM translations WHERE locale=?1)",[locale],|r|r.get::<_,i64>(0))
        .map(|n|n!=0).map_err(|e|e.to_string())
}
fn builtin_translation(locale:&str)->Option<TranslationPackage>{
    let raw=match locale {
        "en"=>include_str!("../locales/en.json"),
        "de"=>include_str!("../locales/de.json"),
        _=>return None
    };
    serde_json::from_str(raw).ok()
}
fn translation_from_db(db:&str,locale:&str)->Result<Option<TranslationPackage>,String>{
    let conn=connect(db)?;
    let raw=conn.query_row("SELECT package_json FROM translations WHERE locale=?1",[locale],|r|r.get::<_,String>(0));
    match raw {
        Ok(raw)=>serde_json::from_str(&raw).map(Some).map_err(|e|e.to_string()),
        Err(rusqlite::Error::QueryReturnedNoRows)=>Ok(None),
        Err(e)=>Err(e.to_string())
    }
}
async fn languages(State(state):State<App>)->Result<Json<Vec<LanguageItem>>,HttpError>{
    let db=state.cfg.db.clone();
    let items=tokio::task::spawn_blocking(move||->Result<Vec<LanguageItem>,String>{
        let mut items=Vec::new();
        for locale in ["en","de"] {
            let p=builtin_translation(locale).ok_or("Missing built-in translations")?;
            items.push(LanguageItem{locale:p.locale,name:p.name,author:p.author,source:p.source,version:p.version,license:p.license,built_in:true});
        }
        let conn=connect(&db)?;
        let mut stmt=conn.prepare("SELECT package_json FROM translations ORDER BY locale").map_err(|e|e.to_string())?;
        let rows=stmt.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?;
        for row in rows {
            let p:TranslationPackage=serde_json::from_str(&row.map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            items.push(LanguageItem{locale:p.locale,name:p.name,author:p.author,source:p.source,version:p.version,license:p.license,built_in:false});
        }
        Ok(items)
    }).await.map_err(internal)?.map_err(internal)?;
    Ok(Json(items))
}
async fn translation(State(state):State<App>,Path(locale):Path<String>)->Result<Json<TranslationPackage>,HttpError>{
    if let Some(p)=builtin_translation(&locale){return Ok(Json(p))}
    let db=state.cfg.db.clone();
    let p=tokio::task::spawn_blocking(move||translation_from_db(&db,&locale)).await.map_err(internal)?.map_err(internal)?
       .ok_or((StatusCode::NOT_FOUND,"Language not found".into()))?;
    Ok(Json(p))
}
async fn upload_translation(State(state):State<App>,headers:HeaderMap,Json(p):Json<TranslationPackage>)->Result<Json<serde_json::Value>,HttpError>{
    admin_authorize(&state,&headers)?;
    if !valid_locale(&p.locale) || p.locale=="en" || p.locale=="de" {
        return Err((StatusCode::BAD_REQUEST,"Invalid locale or built-in locale".into()))
    }
    if p.name.trim().is_empty() || p.name.len()>100 || p.author.trim().is_empty() || p.author.len()>200 ||
        p.source.trim().is_empty() || p.source.len()>500 || p.version.trim().is_empty() || p.version.len()>60 ||
        p.license.trim().is_empty() || p.license.len()>120 || p.strings.is_empty() || p.strings.len()>500 ||
        p.strings.iter().any(|(k,v)| k.len()>100 || k.is_empty() || !k.bytes().all(|b|b.is_ascii_alphanumeric() || b==b'.' || b==b'_') || v.len()>2000 || v.chars().any(|c|c=='\0')) {
        return Err((StatusCode::BAD_REQUEST,"Invalid translation package or metadata".into()))
    }
    let serialized=serde_json::to_string(&p).map_err(internal)?;
    if serialized.len()>120_000 {return Err((StatusCode::PAYLOAD_TOO_LARGE,"Translation package exceeds 120 kB".into()))}
    let db=state.cfg.db.clone();
    tokio::task::spawn_blocking(move||->Result<(),String>{
        let conn=connect(&db)?;
        conn.execute("INSERT INTO translations(locale,package_json) VALUES (?1,?2) ON CONFLICT(locale) DO UPDATE SET package_json=excluded.package_json",params![p.locale,serialized]).map_err(|e|e.to_string())?;
        Ok(())
    }).await.map_err(internal)?.map_err(internal)?;
    Ok(Json(serde_json::json!({"ok":true})))
}
async fn delete_translation(State(state):State<App>,headers:HeaderMap,Path(locale):Path<String>)->Result<Json<serde_json::Value>,HttpError>{
    admin_authorize(&state,&headers)?;
    if locale=="en" || locale=="de" {return Err((StatusCode::BAD_REQUEST,"Built-in languages cannot be removed".into()))}
    let db=state.cfg.db.clone();
    let removed=tokio::task::spawn_blocking(move||->Result<usize,String>{
        let conn=connect(&db)?;
        let active:String=conn.query_row("SELECT language FROM branding WHERE id=1",[],|r|r.get(0)).map_err(|e|e.to_string())?;
        if active==locale {return Err("ACTIVE_LANGUAGE".into())}
        conn.execute("DELETE FROM translations WHERE locale=?1",[locale]).map_err(|e|e.to_string())
    }).await.map_err(internal)?;
    match removed{
        Err(e) if e=="ACTIVE_LANGUAGE"=>Err((StatusCode::CONFLICT,"Select a different language before deleting".into())),
        Err(e)=>Err(internal(e)),
        Ok(0)=>Err((StatusCode::NOT_FOUND,"Language not found".into())),
        Ok(_)=>Ok(Json(serde_json::json!({"ok":true})))
    }
}
fn internal(e:impl std::fmt::Display)->HttpError{error!("internal error: {e}");(StatusCode::INTERNAL_SERVER_ERROR,"Internal error".into())}
async fn index()->impl IntoResponse { Html(include_str!("../static/index.html")) }
async fn health()->impl IntoResponse{(StatusCode::OK,"ok")}
fn extract_ous(dn:&str,ous:&mut HashSet<String>){
    let parts:Vec<&str>=dn.split(',').collect();
    for (i,p) in parts.iter().enumerate(){ if p.trim().to_lowercase().starts_with("ou=") {ous.insert(parts[i..].join(","));} }
}
fn sync_ldap(cfg: Config)->Result<Directory,String>{
    let mut ldap=ldap_connect(&cfg)?;
    ldap.simple_bind(&cfg.ad_bind,&cfg.ad_password).map_err(|e|e.to_string())?.success().map_err(|e|e.to_string())?;
    let (entries,_) = ldap.search(&cfg.ad_base,Scope::Subtree,&cfg.ad_filter,vec!["sAMAccountName","memberOf","distinguishedName"]).map_err(|e|e.to_string())?.success().map_err(|e|e.to_string())?;
    let mut users=HashMap::new(); let mut groups=HashSet::new(); let mut ous=HashSet::new();
    for row in entries {
        let r=SearchEntry::construct(row);
        let Some(username)=r.attrs.get("sAMAccountName").and_then(|v|v.first()).cloned() else {continue};
        let dn=r.dn;
        extract_ous(&dn,&mut ous);
        let memberships:HashSet<String>=r.attrs.get("memberOf").cloned().unwrap_or_default().into_iter().map(|s|s.to_lowercase()).collect();
        // Direct membership; nested groups can be added as separate enhancement.
        for g in r.attrs.get("memberOf").cloned().unwrap_or_default(){groups.insert(g);}
        users.insert(username.to_lowercase(),Person{dn,groups:memberships});
    }
    ldap.unbind().ok();
    let mut groups:Vec<_>=groups.into_iter().collect();groups.sort();
    let mut ous:Vec<_>=ous.into_iter().collect();ous.sort();
    Ok(Directory{users,groups,ous,last_sync:Some(Utc::now()),last_error:None})
}
async fn sync_loop(state:App){
    loop {
        let cfg=state.cfg.clone();
        let result=tokio::task::spawn_blocking(move||sync_ldap(cfg)).await;
        match result {
            Ok(Ok(d))=>{info!("AD synced users={} groups={}",d.users.len(),d.groups.len()); if let Ok(mut lock)=state.directory.write(){*lock=d;}},
            other=>{let problem=format!("{other:?}"); warn!("AD sync failed: {problem}"); if let Ok(mut lock)=state.directory.write(){lock.last_error=Some(problem);}}
        }
        tokio::time::sleep(StdDuration::from_secs(state.cfg.ad_sync)).await;
    }
}
#[tokio::main]
async fn main(){
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_|"info".into())).init();
    if env::args().any(|a|a=="--healthcheck") {
        // Container healthcheck: simple HTTP health probe to localhost.
        match tokio::net::TcpStream::connect("127.0.0.1:8080").await {Ok(_)=>std::process::exit(0),Err(_)=>std::process::exit(1)}
    }
    let cfg=config();
    for (name,secret) in [("ADMIN_TOKEN",&cfg.admin),("WEBHOOK_TOKEN",&cfg.webhook),("CLIENT_TOKEN",&cfg.client)] {
        if secret.len()<32 || secret.contains("CHANGE_ME") {panic!("Set strong {name} of at least 32 characters in .env")}
    }
    if cfg.admin==cfg.webhook || cfg.admin==cfg.client || cfg.webhook==cfg.client {panic!("Tokens must be different")}
    if cfg.ad_enabled && (cfg.ad_password.is_empty() || cfg.ad_password.contains("CHANGE_ME") || cfg.ad_base.is_empty()) {panic!("Configure AD settings")}
    prepare_db(&cfg.db).expect("Initialize database");
    let app=App{cfg,directory:Arc::new(RwLock::new(Directory::default())),sessions:Arc::new(Mutex::new(HashMap::new()))};
    if app.cfg.ad_enabled {tokio::spawn(sync_loop(app.clone()));}
    let router=Router::new()
        .route("/",get(index))
        .route("/health",get(health))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(me))
        .route("/api/auth/logout", post(logout))
        .route("/api/admin/incidents",get(get_incidents).post(create_incident))
        .route("/api/webhooks/incidents",post(create_incident))
        .route("/api/admin/incidents/{id}/resolve",post(resolve_incident))
        .route("/api/admin/targets",get(targets))
        .route("/api/admin/custom-groups",get(custom_groups_list).post(save_custom_group))
        .route("/api/admin/custom-groups/{id}",put(update_custom_group).delete(delete_custom_group))
        .route("/api/branding",get(public_branding))
        .route("/api/languages",get(languages))
        .route("/api/languages/{locale}",get(translation))
        .route("/api/admin/languages",post(upload_translation).layer(DefaultBodyLimit::max(130_000)))
        .route("/api/admin/languages/{locale}",delete(delete_translation))
        .route("/api/branding/logo",get(branding_logo))
        .route("/api/admin/branding",put(set_branding))
        .route("/api/admin/branding/logo",post(upload_logo).delete(clear_logo).layer(DefaultBodyLimit::max(1_100_000)))
        .route("/api/admin/directory",get(directory_status))
        .route("/api/client/state",get(client_state))
        .with_state(app);
    let addr:SocketAddr=env_get("LISTEN_ADDR","0.0.0.0:8080").parse().expect("LISTEN_ADDR");
    info!("listening on {addr}");
    let listener=TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener,router).await.expect("serve");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn admin_group_is_exact_cn() {
        let mut p=Person::default();
        p.groups.insert("cn=serveradmin2,ou=gruppen,dc=example,dc=org".into());
        assert!(!ad_admin(&p,"Serveradmin"));
        p.groups.insert("cn=serveradmin,ou=gruppen,dc=example,dc=org".into());
        assert!(ad_admin(&p,"Serveradmin"));
    }
    #[test] fn all_target_is_visible() {
        let incident=Incident{id:"1".into(),title:"Test".into(),message:"Sample".into(),severity:"error".into(),target_type:"all".into(),target_dn:"".into(),status:"active".into(),created_at:"now".into(),resolved_at:None};
        assert!(visible(&incident,None,"nobody",false,&[]));
    }
    #[test] fn group_target_requires_membership() {
        let incident=Incident{id:"1".into(),title:"Test".into(),message:"Sample".into(),severity:"error".into(),target_type:"group".into(),target_dn:"CN=Finance,DC=example,DC=local".into(),status:"active".into(),created_at:"now".into(),resolved_at:None};
        let p=Person{dn:"CN=User,OU=Finance,DC=example,DC=local".into(),groups:["cn=finance,dc=example,dc=local".into()].into_iter().collect()};
        assert!(visible(&incident,Some(&p),"user",true,&[]));
        assert!(!visible(&incident,None,"user",true,&[]));
    }
    #[test] fn custom_group_matches_any_ad_group() {
        let i=Incident{id:"1".into(),title:"Test".into(),message:"Sample".into(),severity:"error".into(),target_type:"custom".into(),target_dn:"group-1".into(),status:"active".into(),created_at:"now".into(),resolved_at:None};
        let p=Person{dn:"cn=joe,dc=example".into(),groups:["cn=helpdesk,dc=example".to_string()].into_iter().collect()};
        let g=CustomGroup{id:"group-1".into(),name:"IT".into(),ad_groups:vec!["CN=Helpdesk,DC=example".into(),"CN=IT,DC=example".into()]};
        assert!(visible(&i,Some(&p),"joe",true,&[g]));
    }
    #[test] fn ou_target_uses_dn_suffix() {
        let incident=Incident{id:"1".into(),title:"Test".into(),message:"Sample".into(),severity:"error".into(),target_type:"ou".into(),target_dn:"OU=Bauamt,DC=example,DC=local".into(),status:"active".into(),created_at:"now".into(),resolved_at:None};
        let p=Person{dn:"CN=User,OU=Bauamt,DC=example,DC=local".into(),groups:HashSet::new()};
        assert!(visible(&incident,Some(&p),"user",true,&[]));
    }
}
