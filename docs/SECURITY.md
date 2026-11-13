# Security and limitations (read before deployment)

This is a **pilot implementation**, not a finished audited enterprise product.

- Host server behind an HTTPS reverse proxy; default Docker binding is `127.0.0.1`. Don't expose port 8080 over the Internet. Restrict access by VLAN/firewall. Use a trusted internal CA for LDAPS and HTTPS.
- Admin, webhook, and client bearer tokens must be distinct randomly generated secrets, at least 32 characters. Rotate after compromise. Do not commit `.env` or `client.json`.
- **Critical:** Windows client uses a **shared client bearer token** and sends a claimed Windows account name. This is not a cryptographically verified AD identity. Anyone holding the client token can impersonate another user in the API and see their incident text. Only use groups for non-confidential operational messages in the pilot. For production, deploy machine/user-specific authenticated enrollment, Kerberos / mTLS and server-side session identity verification.
- Endpoints accept authenticated messages only; server stores them in SQLite. Admin HTML uses session memory and sends bearer token via header, not browser persistence. Protect against XSS and proxy exposure; use HTTPS.
- LDAP synchronization uses simple bind over LDAPS with certificate validation. Service account needs read-only directory access. No authentication against AD passwords for clients. Nested AD groups are NOT expanded. SAML/SSO isn't included.
- There is no rate limiting, audit trail, client provisioning portal, role separation, HA, full test coverage, or managed database backups.
- Native Windows AppBar implementation currently uses **primary monitor** only, and runs once per user session. Windows 10/11, RDS, taskbar placement changes, DPI and Explorer restarts must be validated in your environment before broad rollout.
- On network errors, the Windows client keeps showing the last successfully fetched incident state. Fail-open would falsely imply recovery; this is intentionally fail-visible, but old incidents may remain until connectivity recovers.
- Client tokens stored in `C:\ProgramData\VGStatusbar\client.json` are readable by local standard users to allow access from their session. Restrict audience and rotate regularly. Do not use to protect sensitive information.
- `client.json` default path and installer registry auto-start suit testing; a robust MSI and signed executables are not included. For baramundi, provide an approved signed executable and explicit upgrade/uninstall actions.
