---
id: client
title: Windows-Client
description: Den Notiflag-Client per MSI, GPO, Intune oder Skript verteilen, und wie er sich auf dem Desktop, bei Vollbild und auf Terminalservern verhält.
lang: de
slug: windows-client
order: 5
---

# Windows-Client

Der Client ist ein einzelnes kleines Programm (`statusbar-client.exe`), das in jeder Benutzersitzung läuft. Kein
Dienst, keine Adminrechte, keine Laufzeitumgebung. Er läuft auf Windows 10 und 11 und auf Terminalservern
(Remote Desktop Session Host).

## Verteilen mit der MSI (empfohlen)

Die MSI ist ein einziges, allgemeines Paket für alle Computer. Serveradresse und Client-Token werden als
Parameter übergeben und für spätere Updates gespeichert:

```bat
msiexec /i Statusbar.msi SERVERURL=https://status.example.com CLIENTTOKEN=<CLIENT_TOKEN> /qn
msiexec /x Statusbar.msi /qn
```

Die MSI

- installiert pro Computer nach `C:\Program Files\Statusbar`,
- trägt den Autostart für alle Benutzer ein (`HKLM\…\Run`),
- speichert Adresse und Token unter `HKLM\Software\Statusbar`,
- beendet einen laufenden Client, bevor sie ihn ersetzt, und aktualisiert ältere Versionen an Ort und Stelle.

Der Client startet bei der nächsten Anmeldung jedes Benutzers. Ein erneuter Aufruf mit anderer `SERVERURL` oder
`CLIENTTOKEN` stellt den Computer um; ein laufender Client übernimmt die neuen Werte innerhalb einer halben
Minute. Ohne Parameter bleiben die gespeicherten Werte.

### Woher bekomme ich die MSI?

In der Weboberfläche unter **Computer**:

- **Paket für diesen Server herunterladen**: ZIP mit der unveränderten `Statusbar.msi` sowie `install.cmd` und
  `uninstall.cmd`, ausgefüllt mit Adresse und Client-Token dieses Servers.
- **Nur den Installer (Statusbar.msi)**: die MSI allein.
- Darunter stehen die fertigen Befehle für Softwareverteilungen zum Kopieren.

Das veröffentlichte Server-Image enthält die MSI. Bei einem selbst gebauten Server siehe [Client
bauen](#client-bauen).

### Gruppenrichtlinie (GPO)

Eine MSI mit Parametern lässt sich per GPO-Softwareinstallation nicht direkt übergeben. Zwei bewährte Wege:

- **Startskript** (Computerkonfiguration → Richtlinien → Windows-Einstellungen → Skripts → Starten): `install.cmd`
  aus dem Paket und `Statusbar.msi` auf eine Freigabe legen, die Computerkonten lesen dürfen, und `install.cmd` als
  Startskript eintragen. Der Computer installiert oder aktualisiert beim Start; ein erneuter Lauf schadet nicht.
- **Transform (MST)**: Mit Orca eine MST erzeugen, die `SERVERURL` und `CLIENTTOKEN` setzt, und sie in der
  GPO-Softwareinstallation unter *Änderungen* angeben.

### Microsoft Intune

*Apps → Windows → Hinzufügen → Branchenspezifische App (LOB)*, `Statusbar.msi` hochladen und als
**Befehlszeilenargumente** eintragen:

```text
SERVERURL=https://status.example.com CLIENTTOKEN=<CLIENT_TOKEN>
```

Installationskontext: Gerät. Alternativ als Win32-App (`.intunewin`) mit `install.cmd` als Installationsbefehl.

### Andere Softwareverteilungen

Jede Verteilung, die MSIs still installieren kann (SCCM/MECM, PDQ, baramundi, RMM-Werkzeuge): den
`msiexec`-Befehl von oben verwenden. Rückgabewert `0` bedeutet erfolgreich.

## Verteilen mit dem Skript

Für einen schnellen Test mit der reinen EXE:

```powershell
.\scripts\install-client.ps1 -ExePath .\statusbar-client.exe -ServerUrl https://status.example.com -Token '<CLIENT_TOKEN>'
```

Das Skript kopiert die EXE nach `%ProgramFiles%\Statusbar`, schreibt `%ProgramData%\Statusbar\client.json` und
trägt den Autostart ein. Es kann als SYSTEM aus einer Softwareverteilung laufen. `scripts\uninstall-client.ps1`
entfernt alles wieder.

`client.json`:

```json
{ "server_url": "https://status.example.com", "client_token": "<CLIENT_TOKEN>" }
```

Reihenfolge der Konfiguration: Werte der MSI (Registry) → `%ProgramData%\Statusbar\client.json` → `client.json`
neben der EXE (praktisch zum Testen).

## Was die Anwender sehen

- **Die Leiste** sitzt oben oder unten am Bildschirm als echte Desktop-Leiste (AppBar): maximierte Fenster rücken
  zur Seite, statt verdeckt zu werden. Neue Meldungen erscheinen kurz höher und schrumpfen dann auf die normale
  Höhe.
- **Inhalt**: `Bezeichnung: Titel – Nachricht` in weißer Schrift, zum Beispiel „Störung: E-Mail ist gestört – Wir
  arbeiten daran.“
- **Farben**: Rot = Störung, Orange = Einschränkung, Blau = Hinweis, Grün = Entwarnung, Violett = stiller Alarm.
- **Mehrere Meldungen** wechseln sich alle 7 Sekunden ab, Störungen zuerst. Ein aktiver stiller Alarm behält das
  linke Feld (40 %), die anderen Meldungen wechseln rechts.
- **Entwarnung**: Die Meldung wird grün und verschwindet nach der eingestellten Zeit (Standard 90 Sekunden).
- **Töne**, wenn eingeschaltet: einmal pro neuer Meldung, für die dringendste.
- Die Leiste lässt sich nicht wegklicken – sie verschwindet, wenn die Meldung entwarnt ist.

Mehrere Monitore, unterschiedliche DPI, Monitorwechsel, Auflösungsänderungen und Neustarts des Explorers
werden berücksichtigt.

## Vollbild und Remotedesktop

**Vollbildfenster** (Präsentationen, Videos, Remotedesktop im Vollbild): Mit *Über Vollbildfenstern sichtbar
bleiben* (Standard: an) legt sich die Leiste über den oberen bzw. unteren Streifen dieses Fensters. Das andere
Fenster wird nie verschoben oder verkleinert. Was die Leiste dabei verdeckt: oben Titelleisten maximierter
Fenster und die Verbindungsleiste einer Remotesitzung, unten deren Taskleiste. Ausgeschaltet verdecken
Vollbildfenster die Leiste.

**Wenn eine Remotedesktop-Sitzung einen Monitor ausfüllt** (klassischer Client `mstsc.exe`):

| Einstellung | Verhalten |
|---|---|
| *Leiste auf diesem Monitor ausblenden* (Standard) | Die lokale Leiste tritt auf diesem Monitor zur Seite. Der Client auf dem Terminalserver zeigt die Meldung in der Sitzung. **Voraussetzung:** Client auch auf den Terminalservern installiert – sonst sehen Anwender in der Vollbildsitzung nichts |
| *Leiste darüber anzeigen* | Die Leiste liegt über der Sitzung wie bei anderen Vollbildfenstern |
| *Experimentell: Sitzung aus dem Vollbild nehmen* | Solange eine Meldung angezeigt wird, verlässt eine Sitzung auf einem Monitor das Vollbild und wird neben die Leiste gesetzt; nach der Entwarnung kehrt sie ins Vollbild zurück. Sitzungen über mehrere Monitore bleiben unberührt |

Ein aktiver stiller Alarm ist immer sichtbar, auch über Vollbild und im Modus *Ausblenden*.

**Terminalserver**: Installiere die MSI auch auf den Session Hosts. Der Client läuft dann in jeder Sitzung,
reserviert dort Platz wie auf einem lokalen Desktop und zeigt die Meldungen des angemeldeten Benutzers.

## Verbindung und Ausfälle

- Der Client hält eine Long-Poll-Verbindung zum Server; neue Meldungen erscheinen in etwa einer Sekunde.
- Ist der Server nicht erreichbar, zeigt er den letzten bekannten Stand weiter und versucht es mit wachsenden
  Abständen erneut. Er erfindet nie eine Entwarnung.
- Der Client spricht nur HTTPS (außer mit `localhost`). Das Zertifikat muss für Windows gültig sein.

## Log

`%LOCALAPPDATA%\Statusbar\client.log` im Profil des Benutzers: Start, Konfigurationsfehler, Verbindungswechsel,
Entscheidungen zu Vollbild und Remotedesktop, Tastenkombination des stillen Alarms. Der Token steht nie im Log.

## Entfernen

```bat
msiexec /x Statusbar.msi /qn
```

Oder `uninstall.cmd` aus dem Paket, oder *Einstellungen → Apps* in Windows. Mit dem Skript installiert:
`scripts\uninstall-client.ps1`.

## Client bauen

Nur nötig, wenn du den Server selbst baust oder den Client unter eigenem Namen ausliefern willst.

- **GitHub Actions**: jeder Push baut `statusbar-client.exe` und `Statusbar.msi` (Artefakt
  `statusbar-client-windows-x64`).
- **Linux im Container**: `scripts/build-client.sh` → `dist/statusbar-client.exe`.
- **Windows**: `cd client && cargo build --release`.

Die Builds sind nicht signiert. Signiere EXE und MSI mit deinem Code-Signing-Zertifikat, bevor du sie verteilst:

```bat
signtool sign /fd SHA256 /tr <RFC-3161-Zeitstempel-URL> /td SHA256 /sha1 <Fingerabdruck> statusbar-client.exe
```

Der Server signiert absichtlich nicht: Ein Signierschlüssel auf einem aus dem Netz erreichbaren Rechner wäre das
wertvollste Ziel der ganzen Installation.

Eigener Name (Ordner, Fensterklasse): Client mit `STATUSBAR_APP_NAME=Acme` bauen, MSI mit
`-d Name=Acme -d Manufacturer="Acme GmbH"`, Skript mit `-Name Acme`. Alles, was auf der Leiste steht, kommt ohnehin
vom Server.
