# Updating running v0.2 to v0.3

This archive contains the server and UI source changes. Keep `.env`, secrets,
CA files and the SQLite named Docker volume in place.

From the running source directory:

```sh
# Back up your configuration locally, not into Git
cp .env ../vg-statusbar.env.backup
# Docker volume backup is recommended separately before upgrading
```

Copy `server/src/main.rs`, `server/Cargo.toml`, and
`server/static/index.html` from this release into the running deployment.
Then build and restart:

```sh
docker compose up -d --build --force-recreate
docker compose logs --tail=80 status-server
```

Visit the admin page through HTTPS and open **Einstellungen**. Create one
custom group with two AD groups and publish a harmless test incident to it.

To publish only audited source files to GitHub, use a separate clean clone:

```sh
git clone https://github.com/GerryCrooked/statusbar.git /tmp/statusbar-source
cp server/src/main.rs /tmp/statusbar-source/server/src/main.rs
cp server/Cargo.toml /tmp/statusbar-source/server/Cargo.toml
cp server/static/index.html /tmp/statusbar-source/server/static/index.html
cp README.md /tmp/statusbar-source/README.md
cp docs/UPDATE-v0.3.md /tmp/statusbar-source/docs/UPDATE-v0.3.md
cd /tmp/statusbar-source
git diff --check
git status --short
git add server/src/main.rs server/Cargo.toml server/static/index.html README.md docs/UPDATE-v0.3.md
git commit -m 'feat: customizable branding and composite AD notification groups'
git push origin main
```

Never push `.env`, `.db`, tokens, credentials, CA certificates or private keys.
