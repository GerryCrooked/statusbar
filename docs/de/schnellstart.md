---
id: quickstart
title: Schnellstart
description: Notiflag-Server starten, einen Windows-Client verbinden und die erste Meldung schicken – in etwa 15 Minuten.
lang: de
slug: schnellstart
order: 2
---

# Schnellstart

In etwa 15 Minuten läuft ein Testserver, ein Windows-PC zeigt die Leiste, und eine Meldung aus der Kommandozeile
erscheint und verschwindet wieder. Für den echten Betrieb folgt danach [Server installieren und
betreiben](server-installation.md).

Du brauchst:

- einen Linux-Rechner mit Docker und Docker Compose (eine kleine VM reicht),
- einen Windows-10/11-PC zum Testen,
- für den Test im Netz: eine HTTPS-Adresse für den Server. Der Client spricht nur HTTPS, außer mit `localhost`.

## 1. Server starten

```bash
git clone https://github.com/GerryCrooked/statusbar.git notiflag
cd notiflag
scripts/setup.sh                 # legt .env mit zwei zufälligen Tokens an
docker compose up -d --build
```

`scripts/setup.sh` schreibt zwei Geheimnisse in `.env`:

| Token | Wofür |
|---|---|
| `ADMIN_TOKEN` | Anmeldung an der Weboberfläche und Admin-API – wie ein Administratorpasswort behandeln |
| `CLIENT_TOKEN` | Damit lesen die Windows-Clients ihre Meldungen |

Öffne `http://127.0.0.1:8080` auf dem Server (oder per SSH-Tunnel) und melde dich mit dem `ADMIN_TOKEN` an.

Der Server lauscht nur auf `127.0.0.1`. Damit Windows-PCs ihn erreichen, kommt ein HTTPS-Reverse-Proxy davor –
Beispiele für Caddy und nginx stehen unter [Server installieren und betreiben](server-installation.md#https-reverse-proxy).

## 2. Einen Windows-Client verbinden

In der Weboberfläche unter **Computer**:

1. **Paket für diesen Server herunterladen** – ein ZIP mit `Statusbar.msi`, `install.cmd` und `uninstall.cmd`,
   bereits ausgefüllt mit Serveradresse und Client-Token.
2. Auf dem Test-PC entpacken und `install.cmd` als Administrator ausführen.
3. Abmelden und wieder anmelden (oder `C:\Program Files\Statusbar\statusbar-client.exe` einmal starten).

Nach wenigen Sekunden steht der PC unter **Computer** mit Benutzer und Client-Version.

## 3. Die erste Meldung

Unter **Meldungen → Neue Meldung**: Titel „Test“, Dringlichkeit *Hinweis*, **Veröffentlichen**. Die Leiste
erscheint auf dem Test-PC. **Entwarnen** färbt sie grün, nach 90 Sekunden ist sie weg.

## 4. Eine Meldung aus dem Monitoring

Unter **Monitoring → Integration hinzufügen**: Name `Test`, *Alarme ohne passende Regel* = **Unverändert
veröffentlichen**, Wartezeit `0`, **Anlegen**. Notiflag zeigt die Webhook-URL einmalig an – kopieren.

```bash
URL='https://status.example.com/api/hooks/sb_…'      # die kopierte URL
curl -X POST "$URL" -H 'Content-Type: application/json' \
  -d '{"status":"problem","host":"mail01","service":"SMTP","message":"Port 25 antwortet nicht"}'
# {"events":["created"],"ok":true}  → die Leiste erscheint
curl -X POST "$URL" -H 'Content-Type: application/json' \
  -d '{"status":"ok","host":"mail01","service":"SMTP"}'
# {"events":["resolved"],"ok":true} → grüne Entwarnung
```

## Wie es weitergeht

- [Server installieren und betreiben](server-installation.md): HTTPS, Updates, Sicherung
- [Windows-Client](windows-client.md): Verteilung per GPO, Intune oder Softwareverteilung
- [Monitoring anbinden](monitoring.md): Zabbix, Checkmk, PRTG, Microsoft 365 und mehr
- [Darstellung und Töne](darstellung.md): Name, Farbe, Logo, Position der Leiste
