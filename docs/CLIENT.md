# Windows client

A single small executable per user session. No service, no elevation, no runtime to install.

## Build

* **GitHub Actions:** every push builds `statusbar-client.exe` (artifact `statusbar-client-windows-x64`).
* **Linux, in a container:** `scripts/build-client.sh` → `dist/statusbar-client.exe`.
* **Windows:** `cd client && cargo build --release`.

The build is unsigned. Sign it with your code-signing certificate before distribution, for example
`signtool sign /fd SHA256 /tr <your RFC 3161 timestamp URL> /td SHA256 /sha1 <thumbprint> statusbar-client.exe`.

To ship under your own name (folder names, window class), build with `STATUSBAR_APP_NAME=Acme`
and pass `-Name Acme` to the install script. Everything users see on the bar comes from the server.

## Install with the MSI (recommended)

The build pipeline also produces `Statusbar.msi` from `installer/statusbar.wxs` (WiX 5). It is one generic
package; server and token are passed as parameters and remembered for later updates:

```
msiexec /i Statusbar.msi SERVERURL=https://status.example.com CLIENTTOKEN=<CLIENT_TOKEN> /qn
msiexec /x Statusbar.msi /qn
```

It installs per machine, adds the autostart entry, stores the settings under `HKLM\Software\Statusbar`, ends a
running client before replacing it, and upgrades older versions in place. The client starts at each user's next
sign-in. Place the signed MSI at `/data/Statusbar.msi` on the server and the *Computers* page of the web UI offers
it for download together with the ready-made commands.

Sign the MSI the same way as the executable. Signing is deliberately not done by the server: a signing key on a
web-reachable machine would be the most valuable thing to steal in the whole set-up.

## Install with the script

```powershell
.\scripts\install-client.ps1 -ExePath .\statusbar-client.exe -ServerUrl https://status.example.com -Token '<CLIENT_TOKEN>'
```

This copies the executable to `%ProgramFiles%\Statusbar`, writes `%ProgramData%\Statusbar\client.json` and adds a
machine-wide autostart entry. The script is safe to run from a software-deployment tool as SYSTEM; the client
itself starts at user sign-in. `scripts\uninstall-client.ps1` removes everything.

`client.json`:

```json
{ "server_url": "https://status.example.com", "client_token": "<CLIENT_TOKEN>" }
```

A `client.json` next to the executable is used when the `%ProgramData%` one does not exist (handy for testing).
Settings written by the MSI (registry) take precedence over `client.json`.

## Behaviour

* Holds one long-poll connection to the server; new messages appear within about a second. Server unreachable →
  keeps the last state, retries with back-off.
* Shows the bar as a shell AppBar on every monitor (or only the primary): maximised windows move out of the way.
  Height is scaled per monitor DPI. Monitors added or removed, resolution changes and Explorer restarts are handled.
* Several messages rotate every 7 seconds, outages first.
* **Full-screen windows** (remote desktop in full screen, presentations): with *Stay visible above full-screen
  windows* enabled (default) the bar keeps itself on top and overlays the top or bottom strip of that window.
  It never resizes or moves the other window. Be aware of what it covers in a full-screen remote session: at the
  top, the title bars of maximised remote windows and the connection bar; at the bottom, the remote taskbar.
  Disable the option to let full-screen windows cover the bar instead.
* **Remote desktop / terminal servers:** install the client on the session host as well. It then runs inside each
  remote session and reserves space there like on a local desktop.
* Log: `%LOCALAPPDATA%\Statusbar\client.log` (start-up, configuration errors, connection changes; never the token).
