---
id: monitoring
title: Monitoring anbinden
description: Monitoring, RMM, Ticketsysteme und Microsoft 365 an Notiflag anbinden – Integrationen, Regeln, Wartezeit, Schlüssel und Fehlersuche.
lang: de
slug: monitoring
order: 11
---

# Monitoring anbinden

Ein Problem im Monitoring öffnet eine Meldung, die Erholung entwarnt sie. Dazwischen entscheiden Regeln, mit
welchen Worten und an wen – und alles ohne Regel wartet auf eine Freigabe per Klick.

## Unterstützte Systeme

Die Schritt-für-Schritt-Anleitungen pro System liegen im Repository und sind derzeit auf Englisch.

| System | Anleitung | Anbindung | Getestet |
|---|---|---|---|
| Zabbix 6.0+ | [zabbix](https://github.com/GerryCrooked/statusbar/blob/main/integrations/zabbix/README.md) | Medientyp (YAML-Import) | Zabbix 7.0 |
| Checkmk 2.0+ | [checkmk](https://github.com/GerryCrooked/statusbar/blob/main/integrations/checkmk/README.md) | Benachrichtigungsskript | Checkmk 2.4 |
| PRTG | [prtg](https://github.com/GerryCrooked/statusbar/blob/main/integrations/prtg/README.md) | PowerShell-Skript oder HTTP-Aktion | PowerShell 7, ohne PRTG |
| Nagios, Naemon, Icinga | [nagios-icinga](https://github.com/GerryCrooked/statusbar/blob/main/integrations/nagios-icinga/README.md) | Shell-Skript + Kommandos | Nagios 4.5, Icinga 2.14 |
| Prometheus Alertmanager | [alertmanager](https://github.com/GerryCrooked/statusbar/blob/main/integrations/alertmanager/README.md) | `webhook_configs` | Alertmanager 0.28 |
| Grafana | [grafana](https://github.com/GerryCrooked/statusbar/blob/main/integrations/grafana/README.md) | Kontaktpunkt *Webhook* | Grafana 13 |
| Uptime Kuma | [uptime-kuma](https://github.com/GerryCrooked/statusbar/blob/main/integrations/uptime-kuma/README.md) | Benachrichtigung *Webhook* | Uptime Kuma 1.23 |
| LibreNMS | [librenms](https://github.com/GerryCrooked/statusbar/blob/main/integrations/librenms/README.md) | Alert-Transport *API* | LibreNMS 26.11 |
| Microsoft 365 Service Health *(in Entwicklung)* | [microsoft-365](https://github.com/GerryCrooked/statusbar/blob/main/integrations/microsoft-365/README.md) | App-Registrierung + 3 Zeilen in `.env`; der Server fragt selbst ab | mit nachgebauter Graph-API |
| Azure Monitor | [azure-monitor](https://github.com/GerryCrooked/statusbar/blob/main/integrations/azure-monitor/README.md) | Aktionsgruppe *Webhook* | nur nach Doku |
| Power Automate (Alarm-Mails …) | [power-automate](https://github.com/GerryCrooked/statusbar/blob/main/integrations/power-automate/README.md) | Flow mit HTTP-Aktion | nur nach Doku |
| NinjaOne *(in Entwicklung)* | [ninjaone](https://github.com/GerryCrooked/statusbar/blob/main/integrations/ninjaone/README.md) | Benachrichtigungskanal *Webhook* | nur nach Doku |
| Atera *(in Entwicklung)* | [atera](https://github.com/GerryCrooked/statusbar/blob/main/integrations/atera/README.md) | Abfrage-Skript beim Dienstleister | mit nachgebauter API |
| Datto RMM | [datto-rmm](https://github.com/GerryCrooked/statusbar/blob/main/integrations/datto-rmm/README.md) | Monitor-Webhook | nur nach Doku |
| N-able N-central | [n-able](https://github.com/GerryCrooked/statusbar/blob/main/integrations/n-able/README.md) | HTTP-XML-Benachrichtigung | nur nach Doku |
| Tactical RMM | [tactical-rmm](https://github.com/GerryCrooked/statusbar/blob/main/integrations/tactical-rmm/README.md) | Webhook im Alert-Template | nur nach Doku |
| ConnectWise, Kaseya VSA, N-sight … | [other-rmm](https://github.com/GerryCrooked/statusbar/blob/main/integrations/other-rmm/README.md) | Alarm-Mails oder Skript | – |
| Zammad | [zammad](https://github.com/GerryCrooked/statusbar/blob/main/integrations/zammad/README.md) | Webhooks + Trigger | Zammad 7.2 |
| GLPI | [glpi](https://github.com/GerryCrooked/statusbar/blob/main/integrations/glpi/README.md) | Webhooks mit Filter | GLPI 12 |
| Znuny / OTRS | [znuny-otrs](https://github.com/GerryCrooked/statusbar/blob/main/integrations/znuny-otrs/README.md) | Generic-Agent-Modul | Znuny 6.5 |
| Jira Service Management | [jira-service-management](https://github.com/GerryCrooked/statusbar/blob/main/integrations/jira-service-management/README.md) | Automation-Regeln | nur nach Doku |
| ServiceNow | [servicenow](https://github.com/GerryCrooked/statusbar/blob/main/integrations/servicenow/README.md) | Business Rule | mit nachgebauten Objekten |
| TOPdesk | [topdesk](https://github.com/GerryCrooked/statusbar/blob/main/integrations/topdesk/README.md) | Action Management | nur nach Doku |
| curl, Skripte, Backup-Jobs | [curl](https://github.com/GerryCrooked/statusbar/blob/main/integrations/curl/README.md) | `curl` oder `statusbar-notify.sh` | bash, dash, busybox |

„Getestet“ heißt: Problem und Erholung kamen aus dem echten System an einem Testserver an, die Meldung erschien und
wurde entwarnt. „Nur nach Doku“ heißt: Die Daten folgen der Dokumentation des Herstellers und sind gegen Notiflag
getestet, aber nicht vom Produkt selbst gesendet. Systeme ohne eigene Anleitung binden sich mit dem [allgemeinen
Format](#allgemeines-format) an.

## So funktioniert es

```text
 Monitoring ──POST──► /api/hooks/<token> ──► Ereignis (Schlüssel, Problem/Erholung, Host, Service, Titel, Text, Dringlichkeit)
                                                   │
        Problem ──► für diesen Schlüssel schon offen? ── ja ──► „duplicate“ (nichts ändert sich)
           │ nein
           ├─ eine Regel passt ──────────► Meldung mit den Worten der Regel        „created“
           └─ keine Regel: Integration ─► Zur Freigabe vorhalten                  „draft“
                                          Unverändert veröffentlichen             „created“
                                          Ignorieren                              „ignored“
        Erholung ─► offene Meldung zu diesem Schlüssel? ─► veröffentlicht: Entwarnung    „resolved“
                                                            Entwurf oder in der Wartezeit: still gelöscht  „cancelled“
                                                            keine: nichts             „ignored“
```

- **Eine Integration pro Monitoring-System** (oder pro Instanz). Schlüssel werden nur innerhalb einer Integration
  verglichen.
- **Regeln gelten für alle Integrationen**, in der Reihenfolge, in der sie angelegt wurden; die erste passende gewinnt.
- **Wartezeit**: Eine veröffentlichte Meldung erscheint erst nach dieser Zeit. Kommt die Erholung vorher, wird sie
  gelöscht und niemand hat sie gesehen.
- **Empfänger**: Die Empfänger der Regel gewinnen; ohne Regel gelten die *Standard-Empfänger* der Integration. Leer
  = alle.

## Integration anlegen

1. **Monitoring → Integration hinzufügen**:
   - **Name**, z. B. `Zabbix`. Er erscheint in der Historie als Herkunft.
   - **Alarme ohne passende Regel**: am Anfang **Zur Freigabe vorhalten**. Solche Alarme warten unter
     *Meldungen → Wartet auf Freigabe*, bis jemand *Veröffentlichen* oder *Verwerfen* klickt.
   - **Wartezeit vor der Anzeige (Sekunden)**: `60` ist ein guter Wert gegen kurze Aussetzer, `0` zum Testen.
   - **Standard-Empfänger**: leer für alle.
2. **Anlegen**. Notiflag zeigt die **Webhook-URL** und einen fertigen `curl`-Test:
   `https://status.example.com/api/hooks/sb_0123456789abcdef…`
3. **Jetzt kopieren** – gespeichert wird nur ein Hash des Tokens. Verloren? Integration löschen und neu anlegen.

Behandle die URL wie ein Passwort: Wer sie hat, kann Meldungen auf alle Bildschirme bringen. Die Liste der
Integrationen zeigt *letztes Ereignis* und einen Zähler – der schnellste Weg zu sehen, ob überhaupt etwas ankommt.

## Problem und Erholung

Notiflag reagiert nur auf Probleme und Erholungen. Bestätigungen, Wartungsfenster, Flattern, Kommentare und
Testnachrichten ohne Zustand werden ignoriert.

| Wert (Groß-/Kleinschreibung egal) | Bedeutung |
|---|---|
| `problem` `down` `critical` `crit` `warning` `warn` `unknown` `unreachable` `firing` `error` `fail` `failed` `failure` `alerting` `1` | **Problem**: Meldung öffnen |
| `ok` `up` `resolved` `recovery` `recovered` `resolve` `normal` `0` | **Erholung**: Meldung entwarnen |
| alles andere (`ACKNOWLEDGEMENT`, `pending`, …) | ignoriert |

**Dringlichkeit** – bestimmt die Farbe:

| Notiflag | Wörter, die darauf abgebildet werden |
|---|---|
| Störung | `error` `critical` `crit` `highest` `high` `disaster` `major` `page` `down` `unreachable` `unknown` |
| Einschränkung | `warning` `warn` `average` `minor` `moderate` `medium` `degraded` |
| Hinweis | `info` `information` `informational` `not classified` `none` `notice` `low` `lowest` |

Ohne Angabe wird ein `warning`-Zustand zur Einschränkung, jedes andere Problem zur Störung. Eine Regel kann die
Dringlichkeit überschreiben.

## Der Schlüssel

Jedes Ereignis hat einen **Schlüssel**. Die Erholung muss denselben Schlüssel tragen wie das Problem, sonst bleibt
die Meldung stehen. Die Anleitungen setzen ihn passend (z. B. `zabbix-<Event-ID>`, `host/service` bei Checkmk und
Nagios, den Fingerprint bei Alertmanager und Grafana).

Solange zu einem Schlüssel eine Meldung offen ist (veröffentlicht, in der Wartezeit oder wartend auf Freigabe),
werden weitere Probleme mit `duplicate` beantwortet – Wiederholungen, Eskalationen und der Wechsel von *Warning*
zu *Critical* erzeugen keine zweite Meldung. Nach Entwarnen oder Verwerfen ist der Schlüssel wieder frei.

## Regeln

Anwender sollen „E-Mail ist gestört. Wir arbeiten daran.“ lesen, nicht `SMTP CRITICAL - connect to port 25 failed`.
Dafür gibt es Regeln. **Monitoring → Regel hinzufügen**:

| Feld | Bedeutung |
|---|---|
| **Wenn Host/Service passt auf** | Muster mit `*` (beliebiger Text) und `?` (ein Zeichen), Groß-/Kleinschreibung egal. Verglichen mit `host/service`, dem Schlüssel und dem Titel; eins davon muss passen. Das Muster muss den **ganzen** Text treffen: `mail01` passt nicht auf `mail01/SMTP`, `mail01/*` schon. `*` überspringt auch `/` |
| **Diesen Titel anzeigen** / **Und diese Nachricht** | Was Anwender lesen. Variablen: `{host}` `{service}` `{title}` `{message}` |
| **Dringlichkeit** | *Wie gemeldet* übernimmt die Dringlichkeit des Alarms |
| **Empfänger** | Wer es sieht. Leer = alle |

Eine Meldung aus einer Regel wird **ohne Freigabe** veröffentlicht (nach der Wartezeit). Die übliche Einrichtung:
Regeln für die Alarme, über die du nachgedacht hast, *Zur Freigabe vorhalten* für alles andere.

Beispiele:

| Muster | Titel | Nachricht | passt auf |
|---|---|---|---|
| `mail*/*` | E-Mail ist gestört | Senden und Empfangen kann gerade nicht funktionieren. Wir arbeiten daran. | jeden Alarm der Hosts `mail…` |
| `*/SMTP*` | E-Mail ist gestört | Ausgehende E-Mails verzögern sich. | den Dienst SMTP auf jedem Host |
| `erp-db?/*` | Das ERP-System ist nicht verfügbar | Bitte speichert eure Arbeit. Wir melden uns, wenn es wieder geht. | `erp-db1/…` bis `erp-db9/…` |
| `*vpn*` | VPN-Verbindungen fallen aus | Arbeiten von zu Hause geht gerade nicht. | alles mit „vpn“ in Host, Service, Schlüssel oder Titel |

## Ticketsysteme: bekannte Störungen für alle

Monitoring meldet, was es erkennt. Ein Ticketsystem deckt den Rest ab: Weiß die IT von einer Störung – gemeldet von
Anwendern, einem Lieferanten, gefunden von einem Techniker –, bringt das Ticket die Meldung auf alle Bildschirme.
Anwender sehen „Die IT ist dran“ und eröffnen keine zwanzig weiteren Tickets. Ist das Ticket gelöst, folgt die
Entwarnung.

- **Nur markierte Tickets**: ein Tag, eine Kategorie, eine Queue, die höchste Priorität oder das Major-Incident-Merkmal
  des Systems.
- **Ticketnummer = Schlüssel.** Öffnen oder Markieren sendet ein Problem, Lösen/Schließen die Erholung.
- **Zur Freigabe vorhalten** ist empfohlen: Tickettitel sind für die IT geschrieben, nicht für Anwender.

Anleitungen für Zammad, GLPI, Znuny/OTRS, Jira Service Management, ServiceNow und TOPdesk stehen in der Tabelle
oben. Andere Systeme (helpLine, Freshservice, HaloPSA, Autotask, …): ein ausgehender Webhook, ein HTTP-Schritt in
Automatisierungen oder eine Skript-Aktion sendet das allgemeine Format mit `"key": "<system>-<ticketnummer>"`.

## Allgemeines Format

Für Systeme ohne eigene Anleitung: `POST` an die Webhook-URL, ein JSON-Objekt (oder eine Liste von bis zu 200).
Der `Content-Type` wird nicht geprüft; flaches XML wird ebenfalls angenommen (für N-able N-central).

```json
{
  "status":   "problem",
  "host":     "mail01",
  "service":  "SMTP",
  "message":  "Port 25 antwortet nicht",
  "severity": "error",
  "key":      "optional-stabile-id",
  "title":    "optional, Standard: \"SMTP (mail01)\""
}
```

| Feld | Auch akzeptiert | Hinweise |
|---|---|---|
| `status` | `state`, `type` | Pflicht, siehe [Problem und Erholung](#problem-und-erholung) |
| `host` | `hostname`, `instance` | Eins von host, service, key, title ist Pflicht |
| `service` | `check`, `trigger` | |
| `key` | `id`, `event_id` | Empfohlen; Standard `host/service` |
| `title` | `summary`, `subject`, `name` | Standard `service (host)`; höchstens 120 Zeichen |
| `message` | `description`, `output`, `text`, `msg` | Höchstens 1500 Zeichen |
| `severity` | `priority` | siehe Dringlichkeit |

Antwort: `{"ok": true, "events": ["created"]}` – pro erkanntem Ereignis eines von `created`, `draft`, `duplicate`,
`resolved`, `cancelled`, `ignored`. Eine leere Liste heißt: kein Problem und keine Erholung erkannt.

**Anmeldung** – drei gleichwertige Wege:

| Weg | Aufruf | Wann |
|---|---|---|
| Token im Pfad | `POST /api/hooks/<token>` | alles, was nur eine URL kennt |
| Bearer-Header | `POST /api/hooks` mit `Authorization: Bearer <token>` | hält den Token aus URLs und Logs |
| Abfrageparameter | `POST /api/hooks?token=<token>` | Werkzeuge, die weder Pfad noch Header ändern können |

## Fehlersuche

Am schnellsten: ein Aufruf von Hand direkt vom Monitoring-Server – das prüft Netz, TLS, Token und Einstellung auf
einmal:

```bash
curl -sS -X POST 'https://status.example.com/api/hooks/<token>' -H 'Content-Type: application/json' \
  -d '{"status":"problem","host":"notiflag-test","service":"manuell"}'
curl -sS -X POST 'https://status.example.com/api/hooks/<token>' -H 'Content-Type: application/json' \
  -d '{"status":"ok","host":"notiflag-test","service":"manuell"}'
```

| Symptom | Ursache und Lösung |
|---|---|
| `401 Unknown integration token` | Token unvollständig kopiert oder Integration gelöscht |
| `400 JSON body required` | Das Werkzeug sendet Formulardaten oder setzt Text mit Anführungszeichen ungeschützt in eine JSON-Vorlage |
| `SSL certificate problem` | Der Monitoring-Server vertraut deinem Zertifikat nicht – Root-CA dort installieren, Prüfung nicht abschalten |
| `{"ok":true,"events":[]}` | Weder Problem noch Erholung – Test-Knopf, Bestätigung, `pending` oder ein unbekanntes Zustandswort |
| `draft`, aber nichts zu sehen | So gewollt: *Zur Freigabe vorhalten*. Freigeben oder eine Regel anlegen |
| `created`, aber noch nichts zu sehen | Die Wartezeit läuft |
| Meldung verschwindet nicht | Die Erholung kam nie an (im Monitoring abgeschaltet) oder mit anderem Schlüssel |
| `ignored` bei einer Erholung | Keine offene Meldung zu diesem Schlüssel: schon entwarnt, verworfen oder an eine andere Integration geschickt |
| Verworfener Alarm kommt wieder | Das Monitoring wiederholt Benachrichtigungen; nach *Verwerfen* ist der Schlüssel frei. An der Quelle stoppen |
| Regel passt nicht | Muster müssen den ganzen Text treffen: `mail01/*` statt `mail01`, `*/SMTP` statt `SMTP` |

Das Server-Log hat pro Ereignis eine Zeile mit Integration, Schlüssel, Problem/Erholung und Ergebnis:

```bash
docker compose logs --since 1h statusbar | grep "monitoring event"
```

Ausführliche Referenz auf Englisch: [docs/INTEGRATIONS.md](https://github.com/GerryCrooked/statusbar/blob/main/docs/INTEGRATIONS.md).
