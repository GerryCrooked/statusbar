# Notiflag documentation

| | Deutsch | English |
|---|---|---|
| Overview | [Was ist Notiflag?](de/index.md) | [What is Notiflag?](en/index.md) |
| Quick start | [Schnellstart](de/schnellstart.md) | [Quick start](en/quickstart.md) |
| Server | [Server installieren und betreiben](de/server-installation.md) | [Install and run the server](en/server-installation.md) |
| Configuration | [Konfiguration](de/konfiguration.md) | [Configuration](en/configuration.md) |
| Client | [Windows-Client](de/windows-client.md) | [Windows client](en/windows-client.md) |
| Messages | [Meldungen](de/meldungen.md) | [Messages](en/messages.md) |
| Recipients | [Empfänger](de/empfaenger.md) | [Recipients](en/recipients.md) |
| Directory | [Active Directory](de/active-directory.md) | [Active Directory](en/active-directory.md) |
| Silent alarm | [Stiller Alarm](de/stiller-alarm.md) | [Silent alarm](en/silent-alarm.md) |
| Appearance | [Darstellung und Töne](de/darstellung.md) | [Appearance and sounds](en/appearance.md) |
| Monitoring | [Monitoring anbinden](de/monitoring.md) | [Connect your monitoring](en/monitoring.md) |
| Service providers | [Für IT-Dienstleister](de/dienstleister-hub.md) | [For IT service providers](en/service-provider-hub.md) |
| License | [Lizenz](de/lizenz.md) | [License](en/license.md) |
| Security | [Sicherheit und Datenschutz](de/sicherheit-und-datenschutz.md) | [Security and privacy](en/security-and-privacy.md) |
| API | [API](de/api.md) | [API](en/api.md) |
| Troubleshooting | [Fehlerbehebung](de/fehlerbehebung.md) | [Troubleshooting](en/troubleshooting.md) |
| FAQ | [Häufige Fragen](de/faq.md) | [FAQ](en/faq.md) |

Reference material in English: [INTEGRATIONS.md](INTEGRATIONS.md) (monitoring reference),
[HUB.md](HUB.md) (hub protocol), and one guide per monitoring system under [`integrations/`](../integrations/README.md).

## For publishing on the website

The pages are plain Markdown (GitHub flavour) so they read on GitHub as they are and can be pulled into the website.

- **Structure**: `docs/de/*.md` and `docs/en/*.md`, one file per page, the same pages in both languages.
- **Navigation**: [`nav.json`](nav.json) lists the pages per language in reading order with `id`, `order`, `slug`,
  `title`, `description` and `path`. Raw URL: `https://raw.githubusercontent.com/GerryCrooked/statusbar/main/docs/nav.json`;
  each page: `https://raw.githubusercontent.com/GerryCrooked/statusbar/main/docs/<path>`.
- **Front matter**: every page starts with YAML front matter carrying the same fields as `nav.json`. Strip it before
  rendering; use `title` as page title and `description` as meta description.
- **Translations**: pages with the same `id` are translations of each other (for `hreflang` and the language switch).
- **Links between pages** are relative (`schnellstart.md`, `monitoring.md#regeln`). Map `<file>.md` to the page with that
  `path` and its `slug` in the same language; anchors are GitHub-style heading slugs.
- **Links to the repository** (integration guides, reference files) are absolute GitHub URLs and can stay as they are.
- **No images**; diagrams are text in `text` code blocks.
- **Changing pages**: edit both languages, then run `python3 scripts/docs-nav.py` – it rewrites `nav.json` and fails
  on broken links, missing headings or a page without translation.
