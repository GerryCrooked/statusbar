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

## Silent alarm

The silent alarm (off by default) lets any user with the client raise a message with a key combination. Before
switching it on:

* **Not an emergency call system.** It relies on the network, the server and someone looking at a screen. There
  is no acknowledgement towards the person who pressed the keys – on purpose, nothing is shown there. Keep your
  regular emergency procedures; agree with the recipients what an alarm means and who goes to check.
* **Who can raise it.** Every desktop with the client, and anyone who has the shared `CLIENT_TOKEN`. The user and
  computer names in the alarm are what the client reports, not verified identities (see above). The server only
  accepts computers that have connected in the last 30 days, which keeps scripts from inventing names, but a
  holder of the token can still name a known computer.
* **Limits against floods and false alarms.** While an alarm from a desktop session is open, pressing again there
  changes nothing. After it is resolved, further presses from that session are ignored for the configured
  cooldown (default 5 minutes). At most 5 alarms per 10 minutes are raised on one server; further attempts are
  refused and logged. The client itself ignores a second press within 10 seconds.
* **Key combination.** It must contain Ctrl, Alt or Win, and two modifiers for a character key, so normal typing
  and shortcuts such as Ctrl+C keep working. Pick a combination nobody uses by accident and test it on a few
  desktops; the client log tells when it could not be registered.
* **Silent where raised.** The alarm is not delivered to the user session that raised it. Other sessions on the
  same computer (terminal servers) and all other recipients see it like any other message, so do not target
  areas where the alarm itself would be a risk.
* **Records.** Each alarm is a message with `created_by` `alarm:<user>@<computer>`, kept like other messages for
  `RETENTION_DAYS`. Every accepted, repeated, limited or refused attempt is written to the server log with user,
  computer and the `X-Forwarded-For` address from the reverse proxy.
* **Service provider.** With a hub connection, the provider sees an active alarm like any other active message
  (title, severity, source `alarm`).

## Backups

Everything is in one SQLite file in the `status-data` volume (`/data/statusbar.sqlite`, WAL mode).
Back it up with `sqlite3 /data/statusbar.sqlite ".backup '/backup/statusbar.sqlite'"` or stop the container and
copy the volume.
