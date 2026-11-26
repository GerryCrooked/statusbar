---
id: api
title: API
description: HTTP-API des Notiflag-Servers – Meldungen per Skript, Monitoring-Webhooks, Einstellungen, Gruppen und Zustand.
lang: de
slug: api
order: 15
---

# API

Alles, was die Weboberfläche kann, geht auch per HTTP. Alle Aufrufe sprechen JSON.

## Anmeldung

| Bereich | Anmeldung |
|---|---|
| `/api/admin/…` | `Authorization: Bearer <ADMIN_TOKEN>` – Aktionen erscheinen in der Historie als *API* –, oder das Sitzungs-Cookie der Weboberfläche |
| `/api/hooks/…` | Token einer Integration (siehe [Monitoring anbinden](monitoring.md#allgemeines-format)) |
| `/api/client/…`, `/api/sounds/…` | `Authorization: Bearer <CLIENT_TOKEN>` |
| `/health`, `/api/meta` | keine |

Fehler antworten mit einem HTTP-Status und einem kurzen Text, z. B. `400 Title required (max. 120 characters)`.

## Meldungen

| Aufruf | Wirkung |
|---|---|
| `GET /api/admin/incidents` | Die letzten 200 Meldungen |
| `POST /api/admin/incidents` | Meldung veröffentlichen |
| `POST /api/admin/incidents/{id}/resolve` | Entwarnen |
| `POST /api/admin/incidents/{id}/publish` | Wartende Meldung freigeben |
| `DELETE /api/admin/incidents/{id}` | Löschen bzw. wartende Meldung verwerfen |

Eine Meldung anlegen:

```bash
curl -X POST https://status.example.com/api/admin/incidents \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' \
  -d '{"title":"Wartung heute Abend","message":"ERP 20-21 Uhr offline","severity":"info",
       "targets":["host:PC-BUCH-*","team:Buchhaltung"],"starts_at":1795190400,"ends_at":1795194000}'
```

| Feld | Pflicht | Bedeutung |
|---|---|---|
| `title` | ja | Höchstens 120 Zeichen |
| `message` | | Höchstens 1500 Zeichen |
| `severity` | | `error` (Standard), `warning`, `info` |
| `targets` | | Leer = alle. Sonst bis zu 50 Einträge `art:wert`: `host:<Muster>`, `team:<Benachrichtigungsgruppe>`, mit Active Directory `group:<DN>`, `ou:<DN>`, `user:<Anmeldename>` |
| `starts_at` | | Unix-Sekunden; erst ab dann anzeigen |
| `ends_at` | | Unix-Sekunden; dann automatisch entwarnen |

Eine Meldung in der Antwort und in der Liste:

```json
{ "id": "6f1c…", "title": "Wartung heute Abend", "message": "…", "severity": "info",
  "targets": ["host:PC-BUCH-*"], "status": "active", "source": "manual", "ext_key": null,
  "created_at": 1795170000, "starts_at": 1795190400, "ends_at": 1795194000, "resolved_at": null,
  "created_by": "api", "approved_by": null, "resolved_by": null }
```

`status`: `draft` (wartet auf Freigabe), `active`, `resolved`, `discarded`. `severity` kann bei Alarmen auch `alarm`
sein. `created_by` / `resolved_by`: Benutzername, `admin`, `api`, `monitoring:<Integration>`, `hub:<Hub>`,
`alarm:<benutzer>@<computer>` oder `schedule`.

## Monitoring

| Aufruf | Wirkung |
|---|---|
| `POST /api/hooks/{token}` (oder `/api/hooks` mit Bearer-Token oder `?token=`) | Problem oder Erholung melden – Format unter [Monitoring anbinden](monitoring.md#allgemeines-format) |
| `GET /api/admin/integrations` | Integrationen und Regeln |
| `POST /api/admin/integrations` | `{"name","unmatched":"draft\|publish\|ignore","delay_seconds","targets"}` – Antwort enthält einmalig `token` |
| `DELETE /api/admin/integrations/{id}` | Integration löschen |
| `POST /api/admin/rules` | `{"pattern","title","message","severity","targets"}` (`severity` leer = wie gemeldet) |
| `DELETE /api/admin/rules/{id}` | Regel löschen |
| `GET /api/admin/m365` | Zustand von Microsoft 365 Service Health |
| `GET /api/admin/ticketing`, `PUT /api/admin/ticketing`, `POST /api/admin/ticketing/test`, `POST /api/admin/ticketing/retry` | Tickets aus Meldungen: Konfiguration und Status, speichern, Testticket, fehlgeschlagene erneut – siehe [Statusbar → Ticketsystem](https://github.com/GerryCrooked/statusbar/blob/main/docs/INTEGRATIONS.md#statusbar--ticket-system-a-ticket-for-every-message) |

## Einstellungen, Gruppen, Töne

| Aufruf | Wirkung |
|---|---|
| `GET /api/admin/settings`, `PUT /api/admin/settings` | Alle Einstellungen als ein Objekt (erst lesen, ändern, ganz zurückschreiben) |
| `PUT /api/admin/logo` (Rohdaten des Bildes), `DELETE /api/admin/logo` | Logo |
| `GET /api/admin/sounds`, `PUT /api/admin/sounds/{art}` (WAV), `DELETE /api/admin/sounds/{art}` | Eigene Töne; `{art}` = `alarm`, `error`, `warning`, `info`, `resolved` |
| `GET /api/admin/teams`, `POST /api/admin/teams` `{"name","targets"}`, `PUT/DELETE /api/admin/teams/{id}` | Benachrichtigungsgruppen |
| `GET /api/admin/targets` | Vorschläge für Empfänger: Computer, Gruppen, OUs, Benutzer, Benachrichtigungsgruppen |

## Computer, Verteilung, Zustand

| Aufruf | Wirkung |
|---|---|
| `GET /api/admin/computers` | Computer mit Client: Name, letzter Benutzer, Client-Version, letzter Kontakt |
| `GET /api/admin/client.msi` | Der Installer |
| `GET /api/admin/client-package.zip` | ZIP mit Installer, `install.cmd`, `uninstall.cmd` für diesen Server |
| `GET /api/admin/license` | Zustand der Lizenz |
| `GET /api/admin/uplink` | Zustand der Hub-Verbindung |
| `GET /health` | `ok`, wenn der Server läuft |
| `GET /api/meta` | Version, Name, Modus (`standalone` oder `directory`) |

## Für Clients

Diese Aufrufe nutzt der Windows-Client; sie sind hier zur Vollständigkeit aufgeführt.

| Aufruf | Wirkung |
|---|---|
| `GET /api/client/state?user=&computer=&version=&rev=&wait=` | Was ein Arbeitsplatz zeigen soll; Long-Poll bis `wait` Sekunden (höchstens 55) |
| `POST /api/client/alarm` `{"user","computer"}` | Stiller Alarm (nur wenn eingeschaltet) |
| `GET /api/sounds/{art}` | Ein hochgeladener Ton |
