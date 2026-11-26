---
id: license
title: Lizenz
description: Bis 10 PCs kostenlos, darüber mit Lizenzschlüssel – wie gezählt wird, wie der Schlüssel eingetragen wird und was an das Lizenzportal geht.
lang: de
slug: lizenz
order: 13
---

# Lizenz

| | |
|---|---|
| **Bis 10 PCs** | kostenlos, alle Funktionen, ohne Ablaufdatum |
| **Ab 11 PCs** | Lizenzschlüssel nötig – Preise und Kauf auf [notiflag.de](https://notiflag.de) |
| **Kundenserver an einem Hub** | keine eigene Lizenz nötig, beliebig viele PCs |
| **Hub für IT-Dienstleister** | Hub-Lizenz, Partnerkonditionen auf Anfrage |

**Es wird nie etwas gesperrt.** Ohne gültige Lizenz zeigt die Weboberfläche einen Hinweis; Meldungen funktionieren
uneingeschränkt weiter.

## Wie gezählt wird

Gezählt werden die Computer, die sich in den letzten 30 Tagen mit dem Client beim Server gemeldet haben. Die Seite
**Computer** zeigt sie mit Namen, letztem Benutzer, Client-Version und letztem Kontakt. Ein Terminalserver zählt als
ein Computer.

## Lizenzschlüssel eintragen

Nach dem Kauf kommt der Schlüssel (`SBL-…`) per E-Mail. In die `.env` des Servers:

```dotenv
LICENSE_KEY=SBL-…
```

```bash
docker compose up -d
```

**Einstellungen → Lizenz** zeigt danach „Lizenziert für …“, die Zahl der lizenzierten und verbundenen Computer und
das Ablaufdatum.

## Zustände und Hinweise

| Hinweis | Bedeutung und Lösung |
|---|---|
| *Keine Lizenz – kostenlos bis 10 Computer* | Alles in Ordnung bis 10 PCs |
| *{n} Computer nutzen Notiflag … bitte eine Lizenz eintragen* | Mehr als 10 PCs ohne Schlüssel |
| *Die Lizenz gilt für {max} Computer, verbunden sind {n}* | Mehr PCs als lizenziert – Lizenz erweitern |
| *Der Lizenzschlüssel funktioniert nicht: abgelaufen / widerrufen / unbekannter Schlüssel* | Schlüssel prüfen, verlängern oder Support fragen |
| *gilt für die andere Art (Server/Hub)* | Ein Hub-Schlüssel steckt in einem Server oder umgekehrt |
| *Das Lizenzportal ist gerade nicht erreichbar. Die Lizenz gilt weiter bis …* | Kein Handlungsbedarf, solange das Datum nicht erreicht ist |
| *Das Lizenzportal ist seit … nicht erreichbar* | Ausgehende Verbindung zu `licensestatusbar.it-klein.com` (HTTPS) prüfen |

## Was an das Lizenzportal geht

Nur mit `LICENSE_KEY` – ohne Schlüssel sendet der Server nichts. Alle 6 Stunden fragt er
`https://licensestatusbar.it-klein.com` und sendet:

- den Lizenzschlüssel,
- eine zufällige Kennung dieser Installation,
- die Serverversion,
- den Namen aus den Einstellungen,
- die Zahl der Computer der letzten 30 Tage.

Keine Benutzernamen, keine Computernamen, keine Meldungen. Die Antworten sind signiert (Ed25519, öffentlicher
Schlüssel im Server eingebaut). Die letzte gültige Antwort gilt 14 Tage – ein nicht erreichbares Portal spielt so
lange keine Rolle.

## Quelloffen

Der Code von Server und Client steht unter der GNU Affero General Public License v3 oder später (AGPL-3.0-or-later)
im Repository [GerryCrooked/statusbar](https://github.com/GerryCrooked/statusbar). Funktionen für Dienstleister
(zentraler Hub, Meldungen an mehrere Kunden, White-Label-Builds) gibt es unter einer separaten kommerziellen Lizenz.
