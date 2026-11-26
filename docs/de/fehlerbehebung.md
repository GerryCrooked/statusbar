---
id: troubleshooting
title: Fehlerbehebung
description: Häufige Probleme mit Server, Client, Monitoring, Active Directory und Lizenz – und wie du sie löst.
lang: de
slug: fehlerbehebung
order: 16
---

# Fehlerbehebung

Die zwei wichtigsten Quellen:

- **Server-Log**: `docker compose logs --since 1h statusbar`
- **Client-Log**: `%LOCALAPPDATA%\Statusbar\client.log` im Profil des betroffenen Benutzers

## Server

| Symptom | Ursache und Lösung |
|---|---|
| Container startet nicht, Log nennt `ADMIN_TOKEN` oder `CLIENT_TOKEN` | Beide müssen gesetzt, mindestens 32 Zeichen lang und verschieden sein. `scripts/setup.sh` erzeugt passende Werte |
| Container startet nicht: `M365_CLIENT_ID is missing` | `M365_TENANT_ID` ist gesetzt, aber nicht alle drei Microsoft-365-Werte |
| Weboberfläche von anderen Rechnern nicht erreichbar | Standard ist `BIND_ADDRESS=127.0.0.1`. Reverse-Proxy davor setzen (empfohlen) oder `BIND_ADDRESS` ändern |
| Anmeldung schlägt fehl | Ohne AD: genau den `ADMIN_TOKEN` aus `.env` eingeben. Mit AD: siehe unten |
| Seite **Computer** bietet keinen Installer an | Selbst gebauter Server ohne MSI: `Statusbar.msi` nach `/data/Statusbar.msi` legen oder das fertige Image nutzen |

## Client

| Symptom | Ursache und Lösung |
|---|---|
| PC erscheint nicht unter **Computer** | Client läuft nicht (Autostart erst ab der nächsten Anmeldung), falsche `SERVERURL`, Proxy/Firewall, oder das Zertifikat ist für Windows nicht gültig. Client-Log prüfen |
| Client-Log: `server_url must start with https://` | Der Client spricht nur HTTPS (außer `localhost`). Adresse mit `https://` verwenden |
| Client-Log: `401` | Falscher `CLIENTTOKEN` – MSI mit dem aktuellen Token neu ausführen |
| PC ist verbunden, zeigt aber eine Meldung nicht | Empfänger passen nicht: Computername, Benutzername oder AD-Zugehörigkeit prüfen; bei geplanten Meldungen *Anzeigen ab* beachten |
| Leiste fehlt in einer Remotedesktop-Sitzung im Vollbild | Modus *Ausblenden* ohne Client auf dem Terminalserver. Client dort installieren oder *Leiste darüber anzeigen* wählen |
| Leiste verdeckt Fensterinhalte im Vollbild | So gewollt mit *Über Vollbildfenstern sichtbar bleiben*. Ausschalten, wenn Vollbildfenster die Leiste verdecken sollen |
| Kein Ton | Ton für diese Art auf *Kein Ton*, eigener Ton noch nicht hochgeladen, Computer stumm geschaltet, oder die Meldung war schon beim Start des Clients da |
| Stiller Alarm löst nicht aus | Nicht eingeschaltet, Tastenkombination von einem anderen Programm belegt oder Zeichen nicht im Tastaturlayout (Client-Log), Sperrzeit nach dem letzten Alarm, oder Grenze von 5 Alarmen in 10 Minuten erreicht (Server-Log) |
| Nach Umzug des Servers keine Verbindung | Neue Adresse per MSI `SERVERURL=<neu>` verteilen oder den alten DNS-Namen auf den neuen Server zeigen lassen |

## Monitoring

Die häufigsten Fälle – `401`, `400 JSON body required`, Meldung verschwindet nicht, Regel passt nicht – stehen mit
Lösung unter [Monitoring anbinden → Fehlersuche](monitoring.md#fehlersuche).

## Active Directory

| Symptom | Ursache und Lösung |
|---|---|
| Log: `certificate verify failed` | Der Container vertraut der CA des Domain Controllers nicht – siehe [Active Directory → Zertifikate](active-directory.md#verschlüsselung-und-zertifikate) |
| Log: `AD_URL must use ldap:// (StartTLS) or ldaps://` | Schema in `AD_URL` korrigieren |
| Anmeldung mit Verzeichniskonto schlägt fehl | Konto nicht direktes Mitglied von `AD_ADMIN_GROUP` (Name der Gruppe, nicht DN), vom `AD_USER_FILTER` nicht erfasst, oder falsches Kennwort |
| Gruppen-Meldungen erreichen einen Benutzer nicht | Nur direkte Mitgliedschaften zählen; der Abgleich läuft alle `AD_SYNC_SECONDS` |
| Empfängerauswahl zeigt keine Gruppen | `AD_ENABLED=yes` fehlt oder der Abgleich schlägt fehl (Log) |

## Lizenz und Hub

| Symptom | Ursache und Lösung |
|---|---|
| *Das Lizenzportal ist seit … nicht erreichbar* | Ausgehendes HTTPS zu `licensestatusbar.it-klein.com` erlauben. Die Lizenz gilt 14 Tage ab der letzten Bestätigung weiter |
| *gilt für die andere Art (Server/Hub)* | Hub-Schlüssel in einem Server oder umgekehrt |
| *Keine Verbindung zum Dienstleister* | `HUB_URL` falsch, `HUB_TOKEN` widerrufen (Kunde im Hub gelöscht) oder ausgehendes HTTPS gesperrt |
| Meldungen des Dienstleisters erscheinen nicht | *Meldungen deines Dienstleisters* steht auf *Zur Freigabe vorhalten* oder *Nicht annehmen* |

## Hilfe holen

Fehler und Ideen: [Issues auf GitHub](https://github.com/GerryCrooked/statusbar/issues). Gib dabei Version
(`/api/meta`), die passenden Zeilen aus Server- und Client-Log (ohne Tokens) und deine Schritte an. Kunden mit Lizenz
erreichen den Support über [notiflag.de](https://notiflag.de).
