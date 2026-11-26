---
id: server
title: Server installieren und betreiben
description: Notiflag-Server mit Docker installieren, per HTTPS erreichbar machen, aktualisieren und sichern.
lang: de
slug: server-installation
order: 3
---

# Server installieren und betreiben

Der Notiflag-Server ist ein einzelner Container mit einer SQLite-Datenbank. Er braucht einen HTTPS-Reverse-Proxy
davor und sonst nichts.

## Voraussetzungen

| | |
|---|---|
| Betriebssystem | Linux mit Docker Engine und Docker Compose (Plugin `docker compose`) |
| Größe | Eine kleine VM reicht; die Datenbank wächst mit der Zahl der Meldungen, nicht der PCs |
| Netz eingehend | HTTPS (443) vom Reverse-Proxy: von allen Windows-PCs und von den Monitoring-Systemen, die Meldungen schicken |
| Netz ausgehend | nur wenn genutzt: Lizenzportal `licensestatusbar.it-klein.com` (mit `LICENSE_KEY`), der Hub deines Dienstleisters (`HUB_URL`), Microsoft Graph (Microsoft 365), dein Domain Controller (Active Directory) |
| Zertifikat | Ein Zertifikat, dem die Windows-PCs vertrauen (öffentlich oder aus deiner internen CA) |

## Installation

Es gibt zwei Wege. Beide nutzen die Datei `compose.yaml` aus dem Repository und ein Volume `status-data` für die
Daten.

### Variante A: Fertiges Image (empfohlen)

Das veröffentlichte Image enthält den Server und den Windows-Installer `Statusbar.msi`.

```bash
git clone https://github.com/GerryCrooked/statusbar.git notiflag && cd notiflag
scripts/setup.sh                       # legt .env mit zufälligen Tokens an
```

`compose.override.yaml` daneben anlegen, damit Compose das Image lädt statt zu bauen:

```yaml
services:
  statusbar:
    image: ghcr.io/gerrycrooked/statusbar:latest   # oder eine feste Version: :<version>
    build: !reset null
```

```bash
docker compose pull && docker compose up -d
```

Tags: `latest` = letzte Version, `<version>` = eine bestimmte Version, `edge` = aktueller Entwicklungsstand
(nicht für den Betrieb).

### Variante B: Aus dem Quellcode bauen

```bash
git clone https://github.com/GerryCrooked/statusbar.git notiflag && cd notiflag
scripts/setup.sh
docker compose up -d --build
```

Ein selbst gebauter Server enthält keinen Windows-Installer. Lege `Statusbar.msi` unter `/data/Statusbar.msi` im
Volume ab (oder setze `CLIENT_MSI`), damit die Seite **Computer** das Paket anbietet – siehe
[Windows-Client](windows-client.md#client-bauen).

### Erster Start

```bash
docker compose ps          # Status "healthy"
curl http://127.0.0.1:8080/health
# ok
```

Anmelden unter `http://127.0.0.1:8080` mit dem `ADMIN_TOKEN` aus `.env` (oder mit einem Konto aus Active
Directory, siehe [Active Directory](active-directory.md)). Alle Einstellungen in `.env` erklärt
[Konfiguration](konfiguration.md).

Der Container läuft mit schreibgeschütztem Dateisystem, ohne Capabilities und mit `no-new-privileges`; nur
`/data` ist beschreibbar.

## HTTPS-Reverse-Proxy

Der Server spricht HTTP und lauscht standardmäßig nur auf `127.0.0.1:8080`. Davor gehört ein Reverse-Proxy mit
TLS. Wichtig:

- **Lange Anfragen erlauben.** Die Clients halten eine Verbindung bis zu 55 Sekunden offen (Long-Poll). Ein
  Lese-Timeout von mindestens 60 Sekunden ist nötig; die meisten Proxys haben das als Standard.
- **`X-Forwarded-For` setzen.** Der Server protokolliert damit die Absender-Adresse eines stillen Alarms.
- **Nicht puffern.** Antworten sind klein; Puffern verzögert nur.

**Caddy** (holt das Zertifikat selbst):

```caddy
status.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

**nginx**:

```nginx
server {
    listen 443 ssl;
    server_name status.example.com;
    ssl_certificate     /etc/ssl/status.example.com.crt;
    ssl_certificate_key /etc/ssl/status.example.com.key;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_read_timeout 75s;
        proxy_buffering off;
    }
}
```

Läuft der Proxy in einem anderen Container, setze `BIND_ADDRESS` in `.env` auf eine Adresse, die er erreicht,
oder hänge beide Container in dasselbe Docker-Netz.

**Wer muss den Server erreichen?**

| Von | Pfad |
|---|---|
| Windows-Clients | `/api/client/…`, `/api/sounds/…` |
| Monitoring, Ticketsysteme, Skripte | `/api/hooks/…` |
| Admins im Browser | alles andere |

Cloud-Dienste (Azure Monitor, Jira Cloud, NinjaOne, Datto RMM …) senden aus dem Internet. Wenn du die
Weboberfläche intern halten willst, gib nach außen nur `/api/hooks/` frei.

## Aktualisieren

**Fertiges Image:**

```bash
docker compose pull && docker compose up -d
```

**Aus dem Quellcode:**

```bash
git pull --ff-only && docker compose build --no-cache && docker compose up -d
```

Die Datenbank wird beim Start automatisch auf den neuen Stand gebracht. Server und Clients sind unabhängig
voneinander aktualisierbar; aktualisiere zuerst den Server, dann die Clients (neue MSI über die Softwareverteilung).

## Sichern und Wiederherstellen

Alle Daten – Meldungen, Einstellungen, Logo, eigene Töne – liegen in der Datenbank `/data/statusbar.sqlite` im
Volume `status-data`, dazu eventuell eigene Sprachdateien unter `/data/locales`. Sichere das ganze Volume:

```bash
docker compose stop
docker run --rm -v notiflag_status-data:/data -v "$PWD":/backup alpine \
  tar czf /backup/notiflag-data-$(date +%F).tgz -C /data .
docker compose start
```

Der Volume-Name ist `<Ordnername>_status-data` (hier `notiflag`); `docker volume ls` zeigt ihn. Wiederherstellen:
Container stoppen, Archiv ins (leere) Volume entpacken, starten. Sichere zusätzlich `.env` – ohne die Tokens
müssen alle Clients neu verteilt werden.

Im laufenden Betrieb geht es auch ohne Stopp, wenn `sqlite3` auf dem Host installiert ist:
`sqlite3 <Volume-Pfad>/statusbar.sqlite ".backup '/backup/statusbar.sqlite'"`.

## Server umziehen

1. Auf dem alten Server sichern (siehe oben), `.env` mitnehmen.
2. Auf dem neuen Server installieren, Volume wiederherstellen, mit derselben `.env` starten.
3. DNS-Name des Servers auf die neue Adresse zeigen lassen. Clients und Monitoring merken nichts.

Ändert sich die Adresse, verteile die MSI neu mit `SERVERURL=<neu>`: Ein laufender Client übernimmt den neuen
Wert innerhalb einer halben Minute.

## Logs und Zustand

```bash
docker compose logs -f statusbar                         # alles
docker compose logs statusbar | grep "monitoring event"  # eingehende Monitoring-Ereignisse
```

`GET /health` antwortet `ok`, `GET /api/meta` liefert Version und Namen. Den Zustand von Lizenz, Hub-Verbindung
und Microsoft 365 zeigt die Weboberfläche unter **Einstellungen** bzw. **Monitoring**.

## Umstieg von Version 0.4

Der Server ab 0.5 übernimmt eine bestehende Datenbank beim ersten Start (Meldungen, Branding und Logo; die alten
Tabellen bleiben mit der Endung `_v04` erhalten) und bedient 0.4-Clients weiter. Zu beachten:

- Der Standardpfad der Datenbank ist jetzt `/data/statusbar.sqlite`; setze `DATABASE_PATH`, um eine bestehende
  Datei zu behalten.
- `AD_ADMIN_GROUP` hat keinen organisationsspezifischen Standard mehr – setze ihn ausdrücklich.
- Die Einstellungen der Leiste sind von `BAR_*`-Variablen in die Weboberfläche gewandert (vorhandene Werte werden
  einmal als Startwerte gelesen).
- „Eigene Gruppen“ aus 0.4 werden beim ersten Start zu Benachrichtigungsgruppen.
- `WEBHOOK_TOKEN` und `/api/webhooks/incidents` funktionieren weiter, wenn die Variable gesetzt ist; neue
  Einrichtungen nutzen Integrationen.
- Der Client heißt jetzt `statusbar-client.exe`, seine Konfiguration liegt in `%ProgramData%\Statusbar\client.json`.
