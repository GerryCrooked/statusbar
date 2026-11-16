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
sign-in.

Published server images contain the installer. The *Computers* page of the web UI offers a package for exactly
that server: a ZIP with the unchanged installer plus `install.cmd` and `uninstall.cmd`, filled in with the server
address and client token. A self-built server finds the installer at `/data/Statusbar.msi` (or `CLIENT_MSI`).

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
* **Full-screen windows** (presentations, videos, and Remote Desktop in overlay mode): with *Stay visible above
  full-screen windows* enabled (default) the bar keeps itself on top and overlays the top or bottom strip of that window.
  It never resizes or moves the other window. Be aware of what it covers in a full-screen remote session: at the
  top, the title bars of maximised remote windows and the connection bar; at the bottom, the remote taskbar.
  Disable the option to let full-screen windows cover the bar instead.
* **Remote Desktop filling a monitor** – setting *When a Remote Desktop session fills a monitor*, for the classic
  Remote Desktop client (`mstsc.exe`):
  * *Hide the bar on that monitor* (default): while a Remote Desktop window is in front and fills a monitor (full screen,
    maximised, or one monitor of a multi-monitor session), the local bar steps aside there. Install the client on
    the session hosts too – it shows the message inside the session, targeted by the same users and groups.
    The other window is only looked at, never changed. Without the client on the session hosts, users in a
    full-screen session see no message – choose the overlay then.
  * *Show the bar on top of it*: the overlay described above.
  * *Experimental – take the session out of full screen*: while a message is shown, a single-monitor full-screen
    session is asked to leave full screen and fitted next to the bar, and returns to full screen after the
    all-clear unless the user moved it. This is the only case in which the client changes another application's
    window. Multi-monitor sessions are never touched.

  Every decision is written to the log.
* **Remote desktop / terminal servers:** install the client on the session hosts as well (same MSI). It runs inside
  each remote session, reserves space there like on a local desktop and receives the messages of the signed-in user.
* Log: `%LOCALAPPDATA%\Statusbar\client.log` (start-up, configuration errors, connection changes; never the token).
