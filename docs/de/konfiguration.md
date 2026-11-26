---
id: configuration
title: Konfiguration
description: Alle Einstellungen des Notiflag-Servers in .env und in der Weboberfläche.
lang: de
slug: konfiguration
order: 4
---

# Konfiguration

Notiflag hat zwei Orte für Einstellungen:

- **`.env`** neben `compose.yaml`: Geheimnisse, Netz, Verzeichnisdienst, Lizenz, Hub, Microsoft 365. Änderungen
  gelten nach `docker compose up -d`.
- **Weboberfläche → Einstellungen**: alles, was die Anwender sehen und hören. Änderungen gelten sofort auf allen
  Clients.

## Einstellungen in `.env`

### Grundlagen

| Variable | Standard | Bedeutung |
|---|---|---|
| `ADMIN_TOKEN` | – (Pflicht) | Anmeldung an der Weboberfläche und Admin-API. Zufällig, mindestens 32 Zeichen: `openssl rand -hex 32` |
| `CLIENT_TOKEN` | – (Pflicht) | Damit lesen die Clients ihre Meldungen. Zufällig, mindestens 32 Zeichen, anders als `ADMIN_TOKEN` |
| `HTTP_PORT` | `8080` | Port, unter dem der Container auf dem Host erreichbar ist |
| `BIND_ADDRESS` | `127.0.0.1` | Adresse auf dem Host. `127.0.0.1` = nur für einen Reverse-Proxy auf demselben Host |
| `RETENTION_DAYS` | `90` | Wie lange entwarnte Meldungen in der Historie bleiben |
| `RUST_LOG` | `info` | Ausführlichkeit des Logs (`warn`, `info`, `debug`) |
| `DATABASE_PATH` | `/data/statusbar.sqlite` | Datenbankdatei im Container |
| `LOCALES_DIR` | `/data/locales` | Ordner für zusätzliche Sprachen der Oberfläche (siehe [Darstellung und Töne](darstellung.md#sprache)) |
| `CLIENT_MSI` | `/data/Statusbar.msi`, sonst der im Image | Installer, den die Seite **Computer** anbietet |
| `ADMIN_SESSION_HOURS` | `8` | Wie lange eine Anmeldung an der Weboberfläche gilt |

Ein Wechsel von `CLIENT_TOKEN` trennt alle Clients, bis sie den neuen Token haben (MSI mit `CLIENTTOKEN=<neu>`
neu verteilen).

### Active Directory

| Variable | Standard | Bedeutung |
|---|---|---|
| `AD_ENABLED` | `no` | `yes` = Anmeldung mit Verzeichniskonto, Empfänger nach Gruppe, OU und Benutzer |
| `AD_URL` | – | `ldap://dc01.example.local:389` (mit StartTLS) oder `ldaps://dc01.example.local:636` |
| `AD_BIND_DN` | – | Lese-Konto, z. B. `CN=svc_notiflag,OU=Service Accounts,DC=example,DC=local` |
| `AD_BIND_PASSWORD` | – | Passwort des Lese-Kontos |
| `AD_BASE_DN` | – | Suchbasis, z. B. `DC=example,DC=local` |
| `AD_ADMIN_GROUP` | `Statusbar-Admins` | Mitglieder dieser Gruppe dürfen sich an der Weboberfläche anmelden. Setze sie ausdrücklich |
| `AD_USER_FILTER` | aktive Benutzerkonten | LDAP-Filter für die Benutzer, die Notiflag kennt |
| `AD_SYNC_SECONDS` | `300` | Abstand des Abgleichs mit dem Verzeichnis |

Details und Zertifikate: [Active Directory](active-directory.md).

### Lizenz, Hub, Microsoft 365

| Variable | Bedeutung |
|---|---|
| `LICENSE_KEY` | Lizenzschlüssel (`SBL-…`), nötig ab 11 PCs ohne Hub. Siehe [Lizenz](lizenz.md) |
| `LICENSE_URL` | Anderes Lizenzportal (nur für Tests) |
| `HUB_URL`, `HUB_TOKEN` | Verbindung zum Hub deines IT-Dienstleisters. Siehe [Für IT-Dienstleister](dienstleister-hub.md) |
| `M365_TENANT_ID`, `M365_CLIENT_ID`, `M365_CLIENT_SECRET` oder `M365_CLIENT_SECRET_FILE` | Microsoft 365 Service Health (in Entwicklung). Weitere `M365_*`: [Anleitung](https://github.com/GerryCrooked/statusbar/blob/main/integrations/microsoft-365/README.md) |
| `WEBHOOK_TOKEN` | Nur für Einrichtungen von vor 0.5 (`/api/webhooks/incidents`); neue Einrichtungen nutzen Integrationen |

## Einstellungen in der Weboberfläche

Unter **Einstellungen**:

| Bereich | Was du einstellst | Mehr dazu |
|---|---|---|
| **Branding** | Name, Untertitel, Farbe, Sprache der Oberfläche, Logo | [Darstellung und Töne](darstellung.md) |
| **Leiste auf den Arbeitsplätzen** | Position, Höhe, Anzeigedauer der Entwarnung, Monitore, Vollbild- und Remotedesktop-Verhalten, Bezeichnungen | [Darstellung und Töne](darstellung.md#die-leiste) |
| **Töne** | Ton pro Art der Meldung | [Darstellung und Töne](darstellung.md#töne) |
| **Stiller Alarm** | Ein/aus, Tastenkombination, Texte, Empfänger, Grenzen | [Stiller Alarm](stiller-alarm.md) |
| **Benachrichtigungsgruppen** | Empfänger unter einem Namen bündeln | [Empfänger](empfaenger.md#benachrichtigungsgruppen) |
| **Meldungen deines Dienstleisters** | Sofort anzeigen, zur Freigabe vorhalten, nicht annehmen (nur mit Hub) | [Für IT-Dienstleister](dienstleister-hub.md) |
| **Lizenz** | Zustand der Lizenz | [Lizenz](lizenz.md) |

Unter **Monitoring** legst du Integrationen und Regeln an – siehe [Monitoring anbinden](monitoring.md).
