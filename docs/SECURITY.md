# Security notes

Read this before rolling out.

* **HTTPS.** The server speaks plain HTTP and is meant to sit behind a TLS reverse proxy; by default it only
  binds to `127.0.0.1`. The client refuses non-HTTPS servers (except localhost). The admin session cookie is
  `Secure`, `HttpOnly`, `SameSite=Strict`.
* **Tokens.** `ADMIN_TOKEN` (full control) and `CLIENT_TOKEN` (read messages) live in the server environment and
  must be random, at least 32 characters, and different. Integration tokens are created in the UI, shown once and
  stored as SHA-256 hashes; an integration token can only open and resolve its own messages.
* **Shared client token, unverified identity.** All desktops use the same `CLIENT_TOKEN`, readable by local users
  in `client.json`, and the client states its own user and computer name. Targeting (computer patterns, groups,
  OUs, users) therefore decides *who is shown* a message, not who is *able to read* it.
  Do not put confidential information into messages.
* **Directory.** LDAP uses StartTLS or LDAPS with certificate verification, a read-only bind account, and direct
  group memberships only (nested groups are not expanded). Admin sign-in checks the password with a bind and
  requires membership in `AD_ADMIN_GROUP`; membership is re-checked on every request.
* **Brute force.** Failed sign-ins are delayed. There is no lockout and no audit trail yet – restrict access to the
  admin UI by network where possible.
* **Offline behaviour.** If the server is unreachable, clients keep showing the last known state. They never
  invent an all-clear.
* **Desktop client.** Runs per user session, without elevation. It only creates and positions its own windows
  and never moves or resizes other applications' windows. Sign the executable before distribution.
* **Not an emergency system.** Delivery is best effort; there is no acknowledgement. Do not use it as the only
  channel for safety-critical alerts.

## Backups

Everything is in one SQLite file in the `status-data` volume (`/data/statusbar.sqlite`, WAL mode).
Back it up with `sqlite3 /data/statusbar.sqlite ".backup '/backup/statusbar.sqlite'"` or stop the container and
copy the volume.
