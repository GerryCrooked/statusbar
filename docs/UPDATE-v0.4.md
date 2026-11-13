# Upgrade to v0.4 – localization and translation packages

**Pilot code: build and runtime not verified against your environment.**

## What changed

- English (`en`) is the initial default and fallback. German (`de`) is bundled.
- Admin **Settings** has a default language selection, and language-pack import.
- Language selection is stored in the existing SQLite database (`branding.language`).
- A new `translations` table stores imported JSON packages and attribution.
- A missing translated key falls back to its English text.
- Branding, optional logo, and custom AD notification groups remain supported.
- Import is admin-only, size-limited and treats translations as text, not markup.
- Changing the default language changes the language of the **web administration UI**, not the content of incident messages. Existing incident titles/messages are authored text and are never automatically translated.
- Native Windows client strings are not part of this web UI localization release.

## Install

First back up the SQLite Docker volume and `.env` outside the Git repository.
Do **not** publish CA certificates, tokens, passwords, or database files.

Extract the ZIP in a separate directory and execute from its project root:

```sh
bash scripts/upgrade-v04.sh /opt/docker/vg-statusbar
cd /opt/docker/vg-statusbar
docker compose up -d --build --force-recreate
docker compose logs --tail=100 status-server
```

The script copies `server/src/main.rs`, `server/static/index.html`,
`server/Cargo.toml`, `server/Dockerfile` and `server/locales/{en,de}.json`.
It does **not** replace `.env`, certificates or the existing SQLite volume.
The new `language` column and `translations` table are added at startup.

## Translation format

Language packs are JSON objects with these required fields:

```json
{
  "locale": "fr",
  "name": "Français",
  "author": "Translator name / team",
  "source": "https://example.org/translation-project",
  "version": "1.0.0",
  "license": "CC BY 4.0 (example only – choose your actual license)",
  "strings": {
    "administrator_sign_in": "Connexion administrateur",
    "settings": "Paramètres"
  }
}
```

Use `server/locales/en.json` as a **complete reference** with all keys.
Only a privileged administrator can upload/replace/remove language packs.
English and German are immutable built-ins; the currently selected language
cannot be removed. Imported packages are stored in SQLite.
Metadata (translator, source URL, version, license) is displayed in Settings.
Import only languages whose licensing/attribution terms allow your use.
Uploaded text does not execute as JavaScript or render as HTML.

## Publish to existing public GitHub repository

Run from the extracted release project root, and verify the local source before push:

```sh
git clone https://github.com/GerryCrooked/statusbar.git /tmp/statusbar-v04-publish
cp server/src/main.rs /tmp/statusbar-v04-publish/server/src/main.rs
cp server/static/index.html /tmp/statusbar-v04-publish/server/static/index.html
cp server/Dockerfile /tmp/statusbar-v04-publish/server/Dockerfile
mkdir -p /tmp/statusbar-v04-publish/server/locales
cp server/locales/*.json /tmp/statusbar-v04-publish/server/locales/
cp docs/UPDATE-v0.4.md /tmp/statusbar-v04-publish/docs/UPDATE-v0.4.md
cd /tmp/statusbar-v04-publish
git diff --check
git status --short
git add server/src/main.rs server/static/index.html server/Dockerfile server/locales docs/UPDATE-v0.4.md
git commit -m "feat: internationalize web UI and add translation package management"
git push origin main
```

Always verify no secrets or internal names appear in the staged diff, especially
because the target GitHub repository is public.
