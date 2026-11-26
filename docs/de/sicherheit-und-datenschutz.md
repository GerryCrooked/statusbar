---
id: security
title: Sicherheit und Datenschutz
description: Sicherheitsmodell, gespeicherte Daten, Datenflüsse und Grenzen von Notiflag – vor dem Rollout lesen.
lang: de
slug: sicherheit-und-datenschutz
order: 14
---

# Sicherheit und Datenschutz

Vor dem Rollout lesen. Notiflag ist bewusst klein gehalten: ein Server, den du selbst betreibst, ohne Cloud und ohne
Telemetrie.

## Sicherheitsmodell

| Thema | Umsetzung |
|---|---|
| **Transport** | Der Server spricht HTTP hinter einem TLS-Reverse-Proxy und lauscht standardmäßig nur auf `127.0.0.1`. Der Client verweigert Server ohne HTTPS (außer `localhost`). |
| **Anmeldung an der Weboberfläche** | Mit `ADMIN_TOKEN` oder einem Verzeichniskonto aus `AD_ADMIN_GROUP`. Sitzungs-Cookie `Secure`, `HttpOnly`, `SameSite=Strict`, gültig `ADMIN_SESSION_HOURS`. Fehlgeschlagene Anmeldungen werden verzögert; es gibt keine Kontosperre. |
| **Tokens** | `ADMIN_TOKEN` (alles) und `CLIENT_TOKEN` (Meldungen lesen, stillen Alarm auslösen) liegen in `.env`: zufällig, mindestens 32 Zeichen, verschieden. Integrations-Tokens werden einmal angezeigt und nur als SHA-256-Hash gespeichert; ein Integrations-Token kann nur seine eigenen Meldungen öffnen und entwarnen. |
| **Gemeinsamer Client-Token** | Alle Arbeitsplätze nutzen denselben `CLIENT_TOKEN`, lesbar für lokale Benutzer. Der Client meldet Benutzer- und Computernamen selbst. Empfänger steuern daher, **wem eine Meldung angezeigt wird**, nicht wer sie lesen **könnte**. Keine vertraulichen Inhalte in Meldungen. |
| **Verzeichnis** | LDAP mit StartTLS oder LDAPS und Zertifikatsprüfung, nur lesendes Konto, nur direkte Gruppenmitgliedschaften. |
| **Container** | Schreibgeschütztes Dateisystem, keine Capabilities, `no-new-privileges`; beschreibbar ist nur `/data`. |
| **Desktop-Client** | Läuft pro Benutzersitzung ohne Adminrechte. Er erzeugt und positioniert nur eigene Fenster (einzige Ausnahme: die ausdrücklich experimentelle Remotedesktop-Option). Signiere EXE und MSI vor der Verteilung. |
| **Töne** | Nur Admins laden WAV-Dateien hoch (bis 1 MB, Prüfung auf RIFF/WAVE). Andere als die angebotenen Windows-Tonnamen werden verworfen. |
| **Offline** | Ist der Server nicht erreichbar, zeigen Clients den letzten Stand weiter und erfinden nie eine Entwarnung. |

**Empfehlungen:** Weboberfläche per Netz oder Reverse-Proxy auf das Admin-Netz beschränken; nach außen höchstens
`/api/hooks/` freigeben; `.env` mit Rechten `600` (macht `scripts/setup.sh`); Tokens bei Verdacht wechseln.

## Kein Alarmierungssystem

Die Zustellung ist „best effort“, es gibt keine Empfangsbestätigung. Nutze Notiflag nicht als einzigen Kanal für
sicherheitskritische Alarme – das gilt besonders für den [stillen Alarm](stiller-alarm.md).

## Welche Daten gespeichert werden

Alles liegt in einer SQLite-Datei auf deinem Server:

| Daten | Inhalt | Aufbewahrung |
|---|---|---|
| Meldungen | Titel, Text, Dringlichkeit, Empfänger, Zeiten, wer angelegt/freigegeben/entwarnt hat (Benutzername, `api`, Integration, Dienstleister, `alarm:<benutzer>@<computer>`) | entwarnt: `RETENTION_DAYS` (Standard 90) Tage |
| Computer | Computername, zuletzt angemeldeter Benutzer, Client-Version, letzter Kontakt | angezeigt und gezählt 30 Tage; ein Eintrag je Computer bleibt gespeichert (derzeit ohne automatische Löschung) |
| Einstellungen | Branding, Leiste, Töne, stiller Alarm, Benachrichtigungsgruppen, Integrationen (Token als Hash), Regeln | bis zur Änderung |
| Verzeichnis | Benutzernamen, Gruppen, OUs aus dem AD | nur im Arbeitsspeicher, regelmäßig neu gelesen |

**Personenbezogen** sind vor allem Benutzernamen (letzter Benutzer pro Computer, Admins in der Historie, Auslöser
stiller Alarme) und Computernamen, die auf Personen schließen lassen. Grundlage ist typischerweise das berechtigte
Interesse am IT-Betrieb; für den stillen Alarm empfiehlt sich eine Information der Anwender und – wo vorhanden – die
Einbindung von Betriebsrat und Datenschutzbeauftragten.

## Welche Daten den Server verlassen

| Ziel | Nur wenn | Was |
|---|---|---|
| Lizenzportal | `LICENSE_KEY` gesetzt | Schlüssel, zufällige Installations-ID, Version, Name, Anzahl Computer – siehe [Lizenz](lizenz.md#was-an-das-lizenzportal-geht) |
| Hub des Dienstleisters | `HUB_URL` gesetzt | Version, Name, Computerliste, Titel aktiver Meldungen, Gruppennamen – siehe [Für IT-Dienstleister](dienstleister-hub.md#was-an-den-hub-geht) |
| Microsoft Graph | `M365_*` gesetzt | Abruf der Service-Health-Störungen deines Tenants; es werden keine Daten an Microsoft gesendet außer der Anmeldung |
| Domain Controller | `AD_ENABLED=yes` | Lesende LDAP-Abfragen |

Sonst nichts: keine Telemetrie, keine Nutzungsstatistik, keine externen Schriften oder Skripte in der Weboberfläche.

## Protokolle

- **Server-Log** (`docker compose logs`): Starts, Monitoring-Ereignisse (Integration, Schlüssel, Ergebnis),
  Verzeichnisabgleich, stille Alarme mit Benutzer, Computer und Absenderadresse. Rotation über die Log-Einstellungen
  von Docker.
- **Client-Log** `%LOCALAPPDATA%\Statusbar\client.log`: Start, Verbindung, Entscheidungen zur Anzeige. Nie der Token.

Es gibt noch kein eigenes Audit-Log für Änderungen an Einstellungen.

## Sicherung

Die SQLite-Datei im Volume enthält alle Daten – siehe [Sichern und
Wiederherstellen](server-installation.md#sichern-und-wiederherstellen). Behandle Sicherungen und `.env` wie die
Tokens selbst.

## Sicherheitslücken melden

Bitte nicht öffentlich als Issue, sondern vertraulich an die auf [notiflag.de](https://notiflag.de) genannte
Kontaktadresse.
