# Upgrade to v0.4.1 and verify deployment

Use the v0.4.1 package directly from v0.2; do NOT install v0.3 first.

1. Back up your Docker SQLite volume and `.env` separately.
2. Extract the release to a **different directory** than the running deployment.
3. From the extracted release directory: `bash scripts/upgrade-v04.sh /opt/docker/vg-statusbar`.
4. On the Docker host: `cd /opt/docker/vg-statusbar && grep -n id="tabSettings" server/static/index.html`.
5. Rebuild with `docker compose build --no-cache status-server && docker compose up -d --force-recreate status-server`.
6. Check deployed API: `curl -fsS http://127.0.0.1:8999/api/version` (adjust bind host and port; or use HTTPS public hostname).
7. Browser: sign out/in if needed, Ctrl+F5. Header should show `Web v0.4.1 · Server v0.4.1` and Settings tab.

CAUTION: do not commit `.env`, SQLite db, CA certs, or secrets to a public repository.

No Rust or Windows integration test was run for this build in this environment.
