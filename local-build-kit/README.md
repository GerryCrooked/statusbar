# VG Statusbar – lokaler Windows-Build und AD-CS-Signing (Pilot-Kit)

Stand: 13.11.2026. **Dieses Paket ist ein Add-on für vorhandenen v0.4.2-Quellcode.** Es überschreibt keine Rust-Quellen, keine `.env`, keine CA-Dateien und keine Datenbank. Nicht mit einem bereits erfolgreich auf Windows getesteten Installer verwechseln: Der Build und die Signierung können erst in eurer Umgebung praktisch geprüft werden.

## Sicherheit / Grenzen

- Keine CA-Privatschlüssel kopieren oder exportieren. **Nur ein dediziertes, nicht exportierbares Code-Signing-Zertifikat** mit EKU `1.3.6.1.5.5.7.3.3` im Zertifikatspeicher des signierenden Benutzers verwenden.
- Das **aktuelle Client-Authentifizierungsmodell** nutzt ein gemeinsames `CLIENT_TOKEN`. Bei der vorhandenen Rust-Implementierung muss `client.json` für den angemeldeten Windows-Benutzer lesbar sein. Damit können lokale Benutzer das gemeinsame Token auslesen. Dieses Paket automatisiert die Übergabe, **es löst dieses Sicherheitsproblem nicht**. Für Sicherheitsalarme benötigt ihr zuerst individuelle Geräteidentitäten und serverseitig überprüfte Benutzeridentitäten.
- Das private Deployment-ZIP enthält das gemeinsame Token. **Nie in GitHub, öffentliche Artefaktablage oder allgemein zugängliche Freigaben hochladen.** Zugriff auf das signierte Installationspaket beschränken; temporäre Kopien nach Installation löschen. Token nach Offenlegung rotieren.
- Interne AD-CS-Signaturen garantieren keine SmartScreen-Freigabe. AppLocker/WDAC und eure Endpoint-Richtlinien bleiben maßgeblich.
- Das Docker-Image `messense/cargo-xwin:latest` ist zunächst eine einfache Pilot-Vorgabe. Für reproduzierbare Produktion bitte nach erfolgreichem Test per Digest pinnen, Hashes der Artefakte archivieren und das Image intern spiegeln. Die SDK-Downloads benötigen beim Erstlauf Internet und unterliegen den Microsoft-Nutzungsbedingungen.
- Der Build läuft in Docker mit einem **schreibbaren Quellcode-Mount** und kann alle dort liegenden Dateien lesen. Deshalb ausschließlich einen separaten, secretfreien Git-Checkout verwenden. Das Skript exportiert den Token **erst nach dem Docker-Lauf** separat auf dem Host. Niemals das Live-Serververzeichnis als Build-Quelle verwenden.

## Phase 1 – lokale Build-Umgebung auf `debian01`

1. ZIP unter `/opt/docker/` ablegen und entpacken:

   ```bash
   cd /opt/docker
   unzip -o vg-statusbar-local-build-kit.zip -d /opt/docker
   cd /opt/docker/vg-statusbar-local-build-kit
   ```

2. **Ein Kommando** erledigt den Checkout, übernimmt nur die beiden lokalen v0.4.2-Clientdateien, baut die EXE und exportiert anschließend die geheime `client.json` außerhalb des Docker-Containers:

   ```bash
   cd /opt/docker/vg-statusbar-local-build-kit
   bash linux/prepare-release.sh
   ```

   Voraussetzungen: Docker, Git, Python 3 und das Linux-Programm `file`. Der öffentliche Git-Clone wird unter `/opt/docker/statusbar-build-source/statusbar` angelegt. Die v0.4.2-Clientdateien kommen selektiv aus `/opt/docker/vg-statusbar`. Für den Erstbuild wird eine Internetverbindung zum Image, Cargo Registry und Windows SDK benötigt. Auf einen unbekannten Git-Remote wird **nicht** automatisch gepusht.

5. Artefakte auf Debian:

   ```text
   /opt/docker/vg-statusbar-local-build-kit/dist/unsigned/vg-status-client.exe
   /opt/docker/vg-statusbar-local-build-kit/dist/unsigned/SHA256SUMS.txt
   /opt/docker/vg-statusbar-local-build-kit/dist/private/client.json
   ```

   **Nie die gesamte `dist/` auf GitHub hochladen.**

## Phase 2 – AD CS einmalig einrichten

Auf einer dedizierten, domänenverbundenen Windows-Signierstation:

1. `certtmpl.msc`: eine dedizierte Code-Signing-Vorlage (EKU „Code Signing“) bereitstellen. Nur einem kleinen Signierer-/IT-Gruppe `Enroll` erlauben; private Schlüssel **nicht exportierbar**, kein allgemeines Autoenrollment. Vorlage anschließend auf der Enterprise-CA veröffentlichen (`certsrv.msc`). Die tatsächlichen Berechtigungen durch euren PKI-Verantwortlichen prüfen.
2. Windows SDK **Signing Tools for Desktop Apps** installieren (`signtool.exe`).
3. Mit dem berechtigten Signierkonto in PowerShell:

   ```powershell
   .\windows\Request-CodeSigningCert.ps1 -Template 'VGCodeSigning'
   ```

   Den Vorlagennamen an euren tatsächlich veröffentlichten Namen anpassen. Thumbprint der Ausgabe merken. Falls die Vorlage bereits registriert wurde, kein zweites Zertifikat beantragen, sondern in `certmgr.msc` den Fingerprint ablesen.

4. Über AD-GPO kontrollieren, ob die benötigte **vertrauenswürdige CA-Zertifikatskette** auf den Zielclients vorhanden ist. Keine pauschale Ausnahme in Defender/SmartScreen erstellen.

## Phase 3 – EXE signieren, privates Installationspaket erstellen

Dateien per `scp` **direkt** auf die berechtigte Signierstation kopieren (PowerShell-Beispiel):

```powershell
New-Item -ItemType Directory -Force C:\Statusbar-Signing | Out-Null
scp root@debian01:/opt/docker/vg-statusbar-local-build-kit/dist/unsigned/vg-status-client.exe C:\Statusbar-Signing\
scp root@debian01:/opt/docker/vg-statusbar-local-build-kit/dist/private/client.json C:\Statusbar-Signing\
```

Die beiden PS-Skripte `Sign-And-Package.ps1`, `Install-VGStatusbar.ps1`, `Uninstall-VGStatusbar.ps1` und `Restore-LastBackup.ps1` aus dem ZIP unter `C:\Statusbar-Signing\windows\` bereitstellen. Danach auf der Signierstation:

```powershell
cd C:\Statusbar-Signing
.\windows\Sign-And-Package.ps1 `
  -ExePath .\vg-status-client.exe `
  -ConfigPath .\client.json `
  -CertificateThumbprint 'THUMBPRINT_AUS_ADCS'
```

Optional `-TimestampUrl 'https://ZEITSTEMPEL_SERVER'` nur bei einem von euch zugelassenen erreichbaren RFC3161-Server. Ohne Zeitstempel läuft die Signatur mit Zertifikatablauf entsprechend aus. Das Skript prüft SignTool-Signatur und Code-Signing-EKU.

Ergebnis: `C:\Statusbar-Signing\windows\release\VGStatusbar-PRIVATE-Deployment.zip` (enthält Secret!).

## Phase 4 – Testinstallation auf genau einem Windows-Client

ZIP in ein lokales, **geschütztes Testverzeichnis** auf Zielrechner entpacken. PowerShell als Administrator:

```powershell
cd C:\Statusbar-Deployment-Test
.\Install-VGStatusbar.ps1 -NoAutostart
```

Anschließend **im interaktiven Benutzerkontext** starten:

```powershell
& "$env:ProgramFiles\VGStatusbar\vg-status-client.exe"
Get-Process vg-status-client -ErrorAction SilentlyContinue
```

Testpunkte: Rot/Grün/90s, oben/unten, 5s Höhe, 2 Monitore, 100/150%-DPI, erneuter Start. Erst nach erfolgreichem Test das Paket über baramundi mit administrativem Installationskontext und ohne `-NoAutostart` ausrollen. HKLM Run startet den Client bei nächster **interaktiver** Anmeldung, nicht im Windows-Service-Kontext.

Rollback auf die letzte vollständige Installation (erhöht): `.\Restore-LastBackup.ps1`. Deinstallation (erhöht): `.\Uninstall-VGStatusbar.ps1`. Diese Befehle dürfen nur ausgeführt werden, wenn die lokale PowerShell-Richtlinie sie erlaubt.

## Phase 5 – GitHub Actions deaktivieren (erst danach)

Bei erfolgreichem lokalem Cross-Build, Signatur und Test: Die lokalen Client-Änderungen zuerst kontrolliert nach GitHub bringen, damit der Quellcode veröffentlicht und das Repository wieder sauber ist:

```bash
cd /opt/docker/statusbar-build-source/statusbar
git add client/src/main.rs client/Cargo.toml
git diff --cached --check
git diff --cached --stat
git commit -m "feat: configurable multi-monitor Windows status bar"
git push origin main
```

Danach die GitHub-Workflow-Datei entfernen:

```bash
cd /opt/docker/vg-statusbar-local-build-kit
bash linux/disable-github-actions.sh /opt/docker/statusbar-build-source/statusbar
cd /opt/docker/statusbar-build-source/statusbar
git diff --cached
# Nur nach Sichtprüfung:
git commit -m "ci: move Windows build to local pipeline"
git push origin main
```

Das Skript verschiebt die Workflow-Datei im lokalen Git-Checkout. Der Push erfolgt absichtlich nicht automatisch.

## Noch nicht enthalten

- Kein neues per-Gerät-Zertifikatsprotokoll; dafür müssen Server- und Client-Authentifizierung gezielt geändert und auditiert werden.
- Kein automatisches Code-Signing direkt unter Linux: Die Signieroperation läuft auf einer Windows-Station mit AD-CS-Zertifikat im privaten Windows-Key-Store.
- Kein Live-Test auf `debian01`, keine echten Domain-/CA-Zugriffe und kein erfolgreicher Windows-Release-Build in dieser Umgebung nachgewiesen.
