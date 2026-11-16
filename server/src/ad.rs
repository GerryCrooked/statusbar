//! Optional Active Directory / LDAP directory: admin sign-in and user targeting.
//! TLS certificate verification is always on (StartTLS on ldap://, TLS on ldaps://).

use crate::store::now;
use ldap3::{LdapConn, LdapConnSettings, Scope, SearchEntry};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

#[derive(Clone, Default, Debug)]
pub struct Person {
    pub dn: String,
    pub groups: HashSet<String>,
}

#[derive(Clone, Default, Debug)]
pub struct Directory {
    pub users: HashMap<String, Person>,
    pub groups: Vec<String>,
    pub ous: Vec<String>,
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub struct AdConfig {
    pub url: String,
    pub bind_dn: String,
    pub bind_password: String,
    pub base_dn: String,
    pub user_filter: String,
    pub admin_group: String,
    pub sync_seconds: u64,
}

fn connect(cfg: &AdConfig) -> Result<LdapConn, String> {
    let settings = LdapConnSettings::new().set_conn_timeout(Duration::from_secs(10));
    let settings = if cfg.url.starts_with("ldap://") {
        settings.set_starttls(true)
    } else if cfg.url.starts_with("ldaps://") {
        settings
    } else {
        return Err("AD_URL must use ldap:// (StartTLS) or ldaps://".into());
    };
    LdapConn::with_settings(settings, &cfg.url).map_err(|e| e.to_string())
}

/// Verifies a user's password with a simple bind over TLS.
pub fn check_password(cfg: &AdConfig, dn: &str, password: &str) -> Result<(), String> {
    let mut ldap = connect(cfg)?;
    let result = ldap
        .simple_bind(dn, password)
        .map_err(|e| e.to_string())
        .and_then(|r| r.success().map(|_| ()).map_err(|e| e.to_string()));
    ldap.unbind().ok();
    result
}

/// Admin group membership is matched on the complete CN, never as a substring.
pub fn is_admin(person: &Person, group: &str) -> bool {
    let expected = format!("cn={},", group.to_lowercase());
    person.groups.iter().any(|dn| dn.starts_with(&expected))
}

fn collect_ous(dn: &str, ous: &mut HashSet<String>) {
    let parts: Vec<&str> = dn.split(',').collect();
    for (i, part) in parts.iter().enumerate() {
        if part.trim().to_lowercase().starts_with("ou=") {
            ous.insert(parts[i..].join(","));
        }
    }
}

pub fn sync(cfg: &AdConfig) -> Result<Directory, String> {
    let mut ldap = connect(cfg)?;
    ldap.simple_bind(&cfg.bind_dn, &cfg.bind_password)
        .map_err(|e| e.to_string())?
        .success()
        .map_err(|e| e.to_string())?;
    let (entries, _) = ldap
        .search(&cfg.base_dn, Scope::Subtree, &cfg.user_filter, vec!["sAMAccountName", "memberOf"])
        .map_err(|e| e.to_string())?
        .success()
        .map_err(|e| e.to_string())?;
    let mut users = HashMap::new();
    let mut groups = HashSet::new();
    let mut ous = HashSet::new();
    for row in entries {
        let entry = SearchEntry::construct(row);
        let Some(name) = entry.attrs.get("sAMAccountName").and_then(|v| v.first()).cloned() else { continue };
        let member_of = entry.attrs.get("memberOf").cloned().unwrap_or_default();
        collect_ous(&entry.dn, &mut ous);
        // Direct memberships only; nested groups are not expanded.
        let lowered: HashSet<String> = member_of.iter().map(|g| g.to_lowercase()).collect();
        groups.extend(member_of);
        users.insert(name.to_lowercase(), Person { dn: entry.dn, groups: lowered });
    }
    ldap.unbind().ok();
    let mut groups: Vec<_> = groups.into_iter().collect();
    groups.sort();
    let mut ous: Vec<_> = ous.into_iter().collect();
    ous.sort();
    Ok(Directory { users, groups, ous, last_sync: Some(now()), last_error: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admin_group_is_exact_cn() {
        let mut p = Person::default();
        p.groups.insert("cn=admins2,ou=groups,dc=example,dc=org".into());
        assert!(!is_admin(&p, "Admins"));
        p.groups.insert("cn=admins,ou=groups,dc=example,dc=org".into());
        assert!(is_admin(&p, "Admins"));
    }
}
