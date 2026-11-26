---
id: messages
title: Meldungen
description: Meldungen schreiben, planen, freigeben und entwarnen – und was die Dringlichkeiten bedeuten.
lang: de
slug: meldungen
order: 6
---

# Meldungen

Eine Meldung ist das, was auf der Leiste steht: ein Titel, optional ein Text, eine Dringlichkeit und die
Empfänger. Sie kommt von einem Admin, aus dem Monitoring, vom Hub des Dienstleisters oder als stiller Alarm.

## Die Seite „Meldungen“

| Bereich | Inhalt |
|---|---|
| **Vorschau** oben | So sieht die Leiste gerade auf den Arbeitsplätzen aus – oder die Meldung, die du gerade schreibst |
| **Neue Meldung** | Formular für eine eigene Meldung |
| **Wartet auf Freigabe** | Meldungen aus Monitoring oder vom Dienstleister, die noch niemand sieht |
| **Aktiv** | Was gerade angezeigt wird oder zu einem geplanten Zeitpunkt erscheint |
| **Zuletzt entwarnt** | Die letzten 15 entwarnten oder verworfenen Meldungen |

Jede Zeile zeigt, wer die Meldung angelegt, freigegeben und entwarnt hat und an wen sie geht.

## Eine Meldung schreiben

**Meldungen → Neue Meldung**:

| Feld | Hinweise |
|---|---|
| **Titel** | Höchstens 120 Zeichen. Steht groß auf der Leiste – kurz und für Anwender verständlich: „E-Mail ist gestört“ |
| **Nachricht** | Optional, höchstens 1500 Zeichen. Was Anwender wissen oder tun sollen: „Wir arbeiten daran. Bitte keine Tickets dazu.“ |
| **Dringlichkeit** | Störung, Einschränkung oder Hinweis (siehe unten) |
| **Empfänger** | Leer = alle. Sonst Computer-Muster, Benachrichtigungsgruppen und – mit Active Directory – Gruppen, OUs, Benutzer. Siehe [Empfänger](empfaenger.md) |
| **Zeitplan** (optional) | *Anzeigen ab* und *Automatisch entwarnen um* |

**Veröffentlichen** – die Meldung ist in etwa einer Sekunde auf den Bildschirmen.

Schreib nichts Vertrauliches in Meldungen: Welche Meldung ein Computer anzeigt, bestimmt die Auswahl der
Empfänger – lesen könnte sie jeder mit dem Client-Token (siehe [Sicherheit und
Datenschutz](sicherheit-und-datenschutz.md)).

## Dringlichkeiten

| Dringlichkeit | Farbe | Wofür |
|---|---|---|
| **Störung** | Rot | Etwas geht nicht: E-Mail, ERP, Internet |
| **Einschränkung** | Orange | Geht, aber langsam oder teilweise |
| **Hinweis** | Blau | Geplante Wartung, Information |
| **Entwarnung** | Grün | Wird automatisch angezeigt, wenn eine Meldung entwarnt wird |
| **Stiller Alarm** | Violett | Nur für [stille Alarme](stiller-alarm.md) |

Die Bezeichnungen vor dem Titel („Störung“, „Hinweis“ …) kannst du unter *Einstellungen → Leiste auf den
Arbeitsplätzen → Bezeichnungen vor dem Titel* ändern.

## Planen

- **Anzeigen ab**: Die Meldung erscheint erst zu diesem Zeitpunkt. Bis dahin steht sie unter *Aktiv* mit
  „Anzeigen ab …“.
- **Automatisch entwarnen um**: Zu diesem Zeitpunkt wird sie entwarnt, als hätte jemand *Entwarnen* geklickt.

Beispiel Wartungsfenster: Titel „ERP heute 20–21 Uhr nicht verfügbar“, Hinweis, *Anzeigen ab* 08:00,
*Automatisch entwarnen um* 21:00.

## Freigeben und verwerfen

Meldungen aus dem Monitoring ohne passende Regel – je nach Einstellung der Integration – und Meldungen vom
Dienstleister – je nach *Meldungen deines Dienstleisters* – landen unter **Wartet auf Freigabe**. Dort sieht sie
noch niemand.

- **Veröffentlichen**: Die Meldung erscheint sofort.
- **Verwerfen**: Eine Meldung aus dem Monitoring wird gelöscht; meldet das Monitoring dasselbe Problem später
  erneut, entsteht eine neue. Eine Meldung vom Dienstleister bleibt als *verworfen* in der Historie und kommt
  nicht wieder, auch wenn der Dienstleister sie aktiv hält.

Kommt die Erholung aus dem Monitoring, bevor jemand freigegeben hat, verschwindet die Meldung von selbst.

## Entwarnen

**Entwarnen** an einer aktiven Meldung: Die Leiste wird grün mit dem Titel der Meldung und verschwindet nach der
eingestellten Zeit (*Entwarnung anzeigen für*, Standard 90 Sekunden). Meldungen aus dem Monitoring entwarnt das
Monitoring selbst, sobald das Problem behoben ist; du kannst sie trotzdem jederzeit von Hand entwarnen.

## Wer hat was getan?

Jede Meldung hält fest, wer sie angelegt hat, und – falls zutreffend – wer sie freigegeben und entwarnt hat:

| Angezeigt als | Herkunft |
|---|---|
| Benutzername oder `admin` | Ein Admin in der Weboberfläche |
| *Monitoring* + Name der Integration | Ein Monitoring-System |
| *API* | Ein Skript mit dem `ADMIN_TOKEN` |
| *Dienstleister* | Der Hub des Dienstleisters |
| *stiller Alarm* | Ein Anwender per Tastenkombination |
| *Zeitplan* | Automatisch zum geplanten Ende |

Entwarnte Meldungen bleiben `RETENTION_DAYS` Tage (Standard 90) in der Datenbank.

## Meldungen per Skript

```bash
curl -X POST https://status.example.com/api/admin/incidents \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' \
  -d '{"title":"Wartung heute Abend","message":"ERP 20-21 Uhr offline","severity":"info","targets":["host:PC-BUCH-*"]}'
```

Für Monitoring-Systeme gibt es eigene Webhooks mit Entwarnung – siehe [Monitoring anbinden](monitoring.md). Alle
Felder: [API](api.md).
