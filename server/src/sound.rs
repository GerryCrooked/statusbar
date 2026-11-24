//! Sounds played by the desktops when a message appears, one per kind of message.
//!
//! Each kind can stay silent, use a standard Windows sound (played by alias, so it follows
//! the sound scheme of the computer) or an uploaded WAV file. Uploaded files are kept in the
//! database like the logo and fetched once by each desktop, which caches them by hash.

use crate::store;
use crate::{admin, bad, bearer, internal, ok, same, Api, App, HttpError};
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

/// Kinds of message that can have a sound, most urgent first.
pub const KINDS: [&str; 5] = ["alarm", "error", "warning", "info", "resolved"];

/// Standard Windows sounds offered in the settings (registry aliases of the sound scheme).
pub const ALIASES: [&str; 10] = [
    "SystemAsterisk",
    "SystemExclamation",
    "SystemHand",
    "SystemNotification",
    "SystemDefault",
    "Notification.Default",
    "Notification.IM",
    "Notification.Mail",
    "Notification.Reminder",
    "Notification.SMS",
];

pub const CUSTOM: &str = "custom";
const MAX_BYTES: usize = 1_000_000;

/// Keeps only values the desktops understand; anything else becomes "no sound".
pub fn sanitize(value: &mut String) {
    let v = value.trim();
    *value = if v == CUSTOM || ALIASES.contains(&v) { v.to_string() } else { String::new() };
}

/// The setting value of one kind.
fn setting<'a>(s: &'a store::Settings, kind: &str) -> &'a str {
    match kind {
        "alarm" => &s.sound_alarm,
        "error" => &s.sound_error,
        "warning" => &s.sound_warning,
        "info" => &s.sound_info,
        "resolved" => &s.sound_resolved,
        _ => "",
    }
}

/// What the desktops are told: `alias:<name>`, `file:<hash>` or nothing, per kind.
/// `uploaded` maps kinds to the hash of their uploaded file.
pub fn for_clients(s: &store::Settings, uploaded: &std::collections::HashMap<String, String>) -> Value {
    let mut out = serde_json::Map::new();
    for kind in KINDS {
        let value = match setting(s, kind) {
            "" => None,
            CUSTOM => uploaded.get(kind).map(|hash| format!("file:{hash}")),
            alias => Some(format!("alias:{alias}")),
        };
        if let Some(v) = value {
            out.insert(kind.into(), json!(v));
        }
    }
    Value::Object(out)
}

/// Loads the hashes of all uploaded sounds (at start and after a change).
pub fn reload(app: &App) -> Result<(), HttpError> {
    let mut map = std::collections::HashMap::new();
    {
        let db = app.db.lock().unwrap();
        for kind in KINDS {
            if let Some((_, hash)) = store::sound(&db, kind).map_err(internal)? {
                map.insert(kind.to_string(), hash);
            }
        }
    }
    *app.sounds.write().unwrap() = map;
    // Desktops pick up the new file at once.
    app.rev.send_modify(|v| *v += 1);
    Ok(())
}

fn check_kind(kind: &str) -> Result<(), HttpError> {
    if KINDS.contains(&kind) {
        Ok(())
    } else {
        Err((StatusCode::NOT_FOUND, "Unknown kind of message".into()))
    }
}

/// A RIFF/WAVE file, as Windows plays it without extra codecs.
pub fn is_wav(data: &[u8]) -> bool {
    data.len() > 44 && &data[..4] == b"RIFF" && &data[8..12] == b"WAVE"
}

pub async fn list(State(app): State<App>, headers: HeaderMap) -> Api {
    admin(&app, &headers)?;
    let uploaded = app.sounds.read().unwrap().clone();
    Ok(Json(json!({"kinds": KINDS, "aliases": ALIASES, "uploaded": uploaded})))
}

pub async fn upload(State(app): State<App>, headers: HeaderMap, Path(kind): Path<String>, body: Bytes) -> Api {
    admin(&app, &headers)?;
    check_kind(&kind)?;
    if body.len() > MAX_BYTES || !is_wav(&body) {
        return Err(bad("Only WAV files up to 1 MB"));
    }
    let hash: String = {
        use sha2::{Digest, Sha256};
        Sha256::digest(&body[..]).iter().take(8).map(|b| format!("{b:02x}")).collect()
    };
    {
        let db = app.db.lock().unwrap();
        store::set_sound(&db, &kind, Some((&body[..], &hash))).map_err(internal)?;
    }
    reload(&app)?;
    Ok(Json(json!({"ok": true, "hash": hash})))
}

pub async fn remove(State(app): State<App>, headers: HeaderMap, Path(kind): Path<String>) -> Api {
    admin(&app, &headers)?;
    check_kind(&kind)?;
    {
        let db = app.db.lock().unwrap();
        store::set_sound(&db, &kind, None).map_err(internal)?;
    }
    reload(&app)?;
    ok()
}

/// The uploaded file, for desktops (client token) and for listening in the admin UI.
pub async fn download(
    State(app): State<App>,
    headers: HeaderMap,
    Path(kind): Path<String>,
) -> Result<Response, HttpError> {
    if !same(bearer(&headers), &app.cfg.client_token) {
        admin(&app, &headers)?;
    }
    check_kind(&kind)?;
    let found = { store::sound(&app.db.lock().unwrap(), &kind) }.map_err(internal)?;
    let (data, _) = found.ok_or((StatusCode::NOT_FOUND, "No sound uploaded".to_string()))?;
    Ok(([(header::CONTENT_TYPE, "audio/wav"), (header::CACHE_CONTROL, "no-cache")], data).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_checked_and_resolved_for_desktops() {
        let mut v = " SystemHand ".to_string();
        sanitize(&mut v);
        assert_eq!(v, "SystemHand");
        let mut v = "C:\\Windows\\evil.wav".to_string();
        sanitize(&mut v);
        assert_eq!(v, "");
        let s = store::Settings {
            sound_alarm: CUSTOM.into(),
            sound_error: "SystemHand".into(),
            sound_info: CUSTOM.into(),
            ..store::Settings::default()
        };
        let uploaded = [("alarm".to_string(), "abc123".to_string())].into_iter().collect();
        let out = for_clients(&s, &uploaded);
        assert_eq!(out, json!({"alarm": "file:abc123", "error": "alias:SystemHand"}), "custom without file is silent");
    }

    #[test]
    fn only_wav_is_accepted() {
        let mut wav = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        wav.resize(64, 0);
        assert!(is_wav(&wav));
        assert!(!is_wav(b"ID3\x03 mp3 data that is long enough to pass the length check ....."));
    }

    #[test]
    fn sounds_are_stored_per_kind() {
        let conn = store::open(":memory:").unwrap();
        store::set_sound(&conn, "alarm", Some((b"data", "h1"))).unwrap();
        assert_eq!(store::sound(&conn, "alarm").unwrap(), Some((b"data".to_vec(), "h1".to_string())));
        assert_eq!(store::sound(&conn, "error").unwrap(), None);
        store::set_sound(&conn, "alarm", None).unwrap();
        assert_eq!(store::sound(&conn, "alarm").unwrap(), None);
    }
}
