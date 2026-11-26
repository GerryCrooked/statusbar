//! License check against the Notiflag license portal (`LICENSE_KEY`).
//!
//! A license never blocks anything: without a valid one the admin UI only shows a notice.
//! - A server connected to a service provider's hub (`HUB_URL`) needs no license of its own.
//! - Otherwise up to [`FREE_COMPUTERS`] computers are free; beyond that a license is expected.
//! - A hub (an add-on that calls [`license_as_hub`]) always needs a hub license and reports the
//!   computers of all its customers.
//!
//! The portal signs its answers (Ed25519); the public key is built in ([`PUBLIC_KEY`]). The
//! last valid answer is kept and stays good for the grace period the portal sets (14 days),
//! so an unreachable portal does not matter for that long. Sent per check: the key, a random
//! id of this installation, kind, version, the display name and the number of computers.

use crate::store::{self, now};
use crate::{admin, Api, App};
use axum::{extract::State, http::HeaderMap, Json};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{sync::OnceLock, time::Duration};
use tracing::{info, warn};

/// Where installations ask, unless `LICENSE_URL` says otherwise.
pub const DEFAULT_URL: &str = "https://licensestatusbar.it-klein.com/api/v1/check";
/// Public half of the portal's signing key (base64, 32 bytes).
pub const PUBLIC_KEY: &str = "eSkKdM6KuUwdq4UdED97CPQMNbCFZZdt/Lo55Sp7DTo=";
/// Computers a server may have without a license.
pub const FREE_COMPUTERS: i64 = 10;

const ANSWER_KEY: &str = "license_answer";
const INSTANCE_KEY: &str = "instance_id";

/// Set by a hub add-on: counts the computers of all customers.
static HUB_COUNT: OnceLock<fn(&App) -> i64> = OnceLock::new();

/// Makes this installation a hub for licensing: it checks a hub license and reports what
/// `count` returns (the computers of all customers). Call before [`crate::start`].
pub fn license_as_hub(count: fn(&App) -> i64) {
    let _ = HUB_COUNT.set(count);
}

fn kind() -> &'static str {
    if HUB_COUNT.get().is_some() {
        "hub"
    } else {
        "server"
    }
}

pub struct LicenseConfig {
    pub key: String,
    pub url: String,
}

/// The signed payload of a check answer.
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
#[serde(default)]
pub struct Answer {
    pub v: i64,
    pub instance: String,
    pub issued_at: i64,
    pub check_hours: i64,
    pub grace_days: i64,
    /// valid, expired, revoked, wrong_kind or unknown
    pub status: String,
    pub kind: Option<String>,
    pub licensee: Option<String>,
    pub max_computers: Option<i64>,
    pub valid_until: Option<i64>,
    pub free: bool,
}

impl Answer {
    /// Until when this answer counts when the portal cannot be reached.
    pub fn good_until(&self) -> i64 {
        self.issued_at + self.grace_days.clamp(1, 90) * 86_400
    }
}

#[derive(Default)]
pub struct LicenseState {
    pub instance: String,
    pub answer: Option<Answer>,
    pub last_ok: Option<i64>,
    pub last_error: Option<String>,
}

/// Checks the signature of `{payload, signature}` and returns the payload.
fn open_envelope(envelope: &Value, public_key: &str) -> Result<Answer, String> {
    let field = |name: &str| {
        envelope[name].as_str().and_then(|s| B64.decode(s).ok()).ok_or_else(|| format!("answer without {name}"))
    };
    let (payload, signature) = (field("payload")?, field("signature")?);
    let key: [u8; 32] = B64.decode(public_key).ok().and_then(|k| k.try_into().ok()).ok_or("bad public key")?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| "bad public key")?;
    let signature: [u8; 64] = signature.try_into().map_err(|_| "bad signature")?;
    key.verify_strict(&payload, &Signature::from_bytes(&signature)).map_err(|_| "signature does not match")?;
    let answer: Answer = serde_json::from_slice(&payload).map_err(|_| "unreadable answer")?;
    if answer.v != 1 {
        return Err(format!("unsupported answer version {}", answer.v));
    }
    Ok(answer)
}

/// An answer that is genuine and meant for this installation, or why not.
fn accept(envelope: &Value, instance: &str) -> Result<Answer, String> {
    let answer = open_envelope(envelope, PUBLIC_KEY)?;
    if answer.instance != instance {
        return Err("answer is for another installation".into());
    }
    Ok(answer)
}

/// What the admin UI shows. `notice` is set when there is something to tell the admin.
#[derive(Serialize, Debug, PartialEq)]
pub struct Verdict {
    /// hub_connected, unlicensed, valid, expired, revoked, wrong_kind, unknown, stale, unverified
    pub state: String,
    /// unlicensed, over_limit, invalid, stale, unverified, offline – or none
    pub notice: Option<&'static str>,
    pub free_limit: i64,
}

/// The pure decision, from what is known now.
pub fn evaluate(
    kind: &str,
    has_key: bool,
    uplink: bool,
    answer: Option<&Answer>,
    last_error: bool,
    computers: i64,
    t: i64,
) -> Verdict {
    let free_limit = if kind == "hub" { 0 } else { FREE_COMPUTERS };
    let over_free = computers > free_limit;
    let verdict = |state: &str, notice| Verdict { state: state.into(), notice, free_limit };
    if !has_key {
        if kind == "server" && uplink {
            return verdict("hub_connected", None);
        }
        return verdict("unlicensed", over_free.then_some("unlicensed"));
    }
    let Some(a) = answer else {
        // Never got an answer yet: only worth a word when the portal could not be asked.
        return verdict("unverified", (last_error && over_free && !uplink).then_some("unverified"));
    };
    if a.good_until() < t {
        return verdict("stale", (over_free && !uplink).then_some("stale"));
    }
    let status = if a.status == "valid" && a.valid_until.is_some_and(|u| u < t) { "expired" } else { &a.status };
    if status != "valid" {
        // A key was set on purpose, so a broken one is always mentioned.
        return verdict(status, Some("invalid"));
    }
    if a.max_computers.is_some_and(|m| computers > m) {
        return verdict("valid", Some("over_limit"));
    }
    // Still fine, but tell the admin before the grace period runs out.
    let offline = last_error && t - a.issued_at > 2 * 86_400;
    verdict("valid", offline.then_some("offline"))
}

fn computers(app: &App) -> i64 {
    match HUB_COUNT.get() {
        Some(count) => count(app),
        None => store::count_computers(&app.db.lock().unwrap(), now() - crate::uplink::ACTIVE_WINDOW).unwrap_or(0),
    }
}

pub async fn license_status(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    let count = computers(&app);
    let state = app.license.lock().unwrap();
    let has_key = app.cfg.license.is_some();
    let t = now();
    let verdict = evaluate(
        kind(),
        has_key,
        app.cfg.uplink.is_some(),
        state.answer.as_ref(),
        state.last_error.is_some(),
        count,
        t,
    );
    let a = state.answer.as_ref();
    Ok(Json(json!({
        "kind": kind(), "configured": has_key, "state": verdict.state, "notice": verdict.notice,
        "free_limit": verdict.free_limit, "computers": count,
        "licensee": a.and_then(|a| a.licensee.clone()), "max_computers": a.and_then(|a| a.max_computers),
        "valid_until": a.and_then(|a| a.valid_until), "free": a.is_some_and(|a| a.free),
        "good_until": a.map(Answer::good_until), "last_ok": state.last_ok, "last_error": state.last_error, "now": t,
    })))
}

/// Loads the installation id and the last answer; creates the id on first start.
pub fn load(app: &App) {
    let db = app.db.lock().unwrap();
    let instance = match store::kv_get(&db, INSTANCE_KEY) {
        Ok(Some(id)) => id,
        _ => {
            let id = uuid::Uuid::new_v4().simple().to_string();
            if let Err(e) = store::kv_set(&db, INSTANCE_KEY, &id) {
                warn!("could not store the installation id: {e}");
            }
            id
        }
    };
    let answer = store::kv_get(&db, ANSWER_KEY)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|env| accept(&env, &instance).ok());
    let mut state = app.license.lock().unwrap();
    state.last_ok = answer.as_ref().map(|a| a.issued_at);
    state.answer = answer;
    state.instance = instance;
}

async fn check(app: &App, client: &reqwest::Client, cfg: &LicenseConfig) -> Result<(Answer, Value), String> {
    let instance = app.license.lock().unwrap().instance.clone();
    let body = json!({
        "key": cfg.key, "instance": instance, "kind": kind(), "version": env!("CARGO_PKG_VERSION"),
        "name": app.settings.read().unwrap().name.clone(), "computers": computers(app),
    });
    let response = client.post(&cfg.url).json(&body).send().await.map_err(|e| format!("portal not reachable: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("portal answered {}", response.status()));
    }
    let envelope: Value = response.json().await.map_err(|_| "unreadable answer".to_string())?;
    let answer = accept(&envelope, &instance)?;
    Ok((answer, envelope))
}

pub async fn license_loop(app: App) {
    let Some(cfg) = app.cfg.license.as_ref() else {
        return;
    };
    let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).build().expect("http client");
    // Let the first desktops report in before counting.
    tokio::time::sleep(Duration::from_secs(10)).await;
    let mut backoff = 300;
    loop {
        let wait = match check(&app, &client, cfg).await {
            Ok((answer, envelope)) => {
                if let Err(e) = store::kv_set(&app.db.lock().unwrap(), ANSWER_KEY, &envelope.to_string()) {
                    warn!("could not store the license answer: {e}");
                }
                info!("license check: {}", answer.status);
                let hours = answer.check_hours.clamp(1, 168) as u64;
                let mut state = app.license.lock().unwrap();
                state.last_ok = Some(now());
                state.last_error = None;
                state.answer = Some(answer);
                backoff = 300;
                hours * 3600
            }
            Err(e) => {
                warn!("license check failed: {e}");
                app.license.lock().unwrap().last_error = Some(e);
                let wait = backoff;
                backoff = (backoff * 2).min(3600);
                wait
            }
        };
        tokio::time::sleep(Duration::from_secs(wait)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn answer(status: &str) -> Answer {
        Answer {
            v: 1,
            instance: "abc12345".into(),
            issued_at: 1_000_000,
            check_hours: 6,
            grace_days: 14,
            status: status.into(),
            kind: Some("server".into()),
            licensee: Some("Praxis".into()),
            max_computers: Some(50),
            valid_until: None,
            free: true,
        }
    }

    #[test]
    fn signed_answers_are_checked() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let public = B64.encode(key.verifying_key().as_bytes());
        let payload = serde_json::to_vec(&answer("valid")).unwrap();
        let envelope = |payload: &[u8]| json!({"payload": B64.encode(payload), "signature": B64.encode(key.sign(payload).to_bytes())});
        assert_eq!(open_envelope(&envelope(&payload), &public).unwrap(), answer("valid"));
        let mut forged = envelope(&payload);
        let tampered = serde_json::to_vec(&Answer { max_computers: None, ..answer("valid") }).unwrap();
        forged["payload"] = json!(B64.encode(tampered));
        assert!(open_envelope(&forged, &public).is_err());
        let other = B64.encode(SigningKey::from_bytes(&[8; 32]).verifying_key().as_bytes());
        assert!(open_envelope(&envelope(&payload), &other).is_err());
        assert!(open_envelope(&json!({}), &public).is_err());
    }

    #[test]
    fn the_built_in_key_is_a_key() {
        let bytes: [u8; 32] = B64.decode(PUBLIC_KEY).unwrap().try_into().unwrap();
        assert!(VerifyingKey::from_bytes(&bytes).is_ok());
    }

    #[test]
    fn notices() {
        let t = 1_000_000 + 3600;
        let notice = |kind, key, uplink, a: Option<&Answer>, err, n| evaluate(kind, key, uplink, a, err, n, t).notice;
        // No key: free up to ten, nothing with a hub, hubs always need one.
        assert_eq!(notice("server", false, false, None, false, 10), None);
        assert_eq!(notice("server", false, false, None, false, 11), Some("unlicensed"));
        assert_eq!(notice("server", false, true, None, false, 500), None);
        assert_eq!(notice("hub", false, false, None, false, 1), Some("unlicensed"));
        // Valid license, within and over its limit.
        let valid = answer("valid");
        assert_eq!(notice("server", true, false, Some(&valid), false, 50), None);
        assert_eq!(notice("server", true, false, Some(&valid), false, 51), Some("over_limit"));
        // A broken key is always mentioned.
        assert_eq!(notice("server", true, false, Some(&answer("revoked")), false, 1), Some("invalid"));
        let ended = Answer { valid_until: Some(t - 1), ..answer("valid") };
        assert_eq!(evaluate("server", true, false, Some(&ended), false, 1, t).state, "expired");
        // Portal away: fine within the grace period, a hint after two days, stale after it.
        assert_eq!(notice("server", true, false, Some(&valid), true, 20), None);
        let later = 1_000_000 + 3 * 86_400;
        assert_eq!(evaluate("server", true, false, Some(&valid), true, 20, later).notice, Some("offline"));
        let gone = 1_000_000 + 15 * 86_400;
        assert_eq!(evaluate("server", true, false, Some(&valid), true, 20, gone).notice, Some("stale"));
        assert_eq!(evaluate("server", true, false, Some(&valid), true, 5, gone).notice, None);
        // Never reached the portal.
        assert_eq!(notice("server", true, false, None, true, 20), Some("unverified"));
        assert_eq!(notice("server", true, false, None, false, 20), None);
    }
}
