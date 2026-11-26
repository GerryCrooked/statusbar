---
id: client
title: Windows client
description: Roll out the Notiflag client with the MSI, Group Policy, Intune or a script, and how it behaves on the desktop, with full-screen windows and on terminal servers.
lang: en
slug: windows-client
order: 5
---

# Windows client

The client is a single small program (`statusbar-client.exe`) that runs in each user session. No service, no admin
rights, no runtime. It runs on Windows 10 and 11 and on terminal servers (Remote Desktop Session Host).

## Roll out with the MSI (recommended)

The MSI is one generic package for every computer. Server address and client token are passed as parameters and
remembered for later updates:

```bat
msiexec /i Statusbar.msi SERVERURL=https://status.example.com CLIENTTOKEN=<CLIENT_TOKEN> /qn
msiexec /x Statusbar.msi /qn
```

The MSI

- installs per machine to `C:\Program Files\Statusbar`,
- adds the autostart entry for all users (`HKLM\…\Run`),
- stores address and token under `HKLM\Software\Statusbar`,
- ends a running client before replacing it and upgrades older versions in place.

The client starts at each user's next sign-in. Running it again with a different `SERVERURL` or `CLIENTTOKEN`
switches the computer over; a running client picks the new values up within half a minute. Without parameters the
stored values stay.

### Where do I get the MSI?

In the web UI under **Computers**:

- **Download the package for this server**: a ZIP with the unchanged `Statusbar.msi` plus `install.cmd` and
  `uninstall.cmd`, filled in with this server's address and client token.
- **Only the installer (Statusbar.msi)**: the MSI on its own.
- Below them are the ready commands for deployment tools to copy.

The released server image contains the MSI. For a self-built server see [Build the client](#build-the-client).

### Group Policy

Group Policy software installation cannot pass MSI parameters directly. Two proven ways:

- **Startup script** (Computer Configuration → Policies → Windows Settings → Scripts → Startup): put `install.cmd` from
  the package and `Statusbar.msi` on a share the computer accounts can read and add `install.cmd` as startup script.
  The computer installs or updates at start; running it again does no harm.
- **Transform (MST)**: create an MST with Orca that sets `SERVERURL` and `CLIENTTOKEN`, and add it under
  *Modifications* in the Group Policy software installation.

### Microsoft Intune

*Apps → Windows → Add → Line-of-business app*, upload `Statusbar.msi` and enter as **Command-line arguments**:

```text
SERVERURL=https://status.example.com CLIENTTOKEN=<CLIENT_TOKEN>
```

Install context: device. Alternatively as a Win32 app (`.intunewin`) with `install.cmd` as install command.

### Other deployment tools

Any tool that installs MSIs silently (SCCM/MECM, PDQ, baramundi, RMM tools): use the `msiexec` command above.
Exit code `0` means success.

## Roll out with the script

For a quick test with the plain executable:

```powershell
.\scripts\install-client.ps1 -ExePath .\statusbar-client.exe -ServerUrl https://status.example.com -Token '<CLIENT_TOKEN>'
```

The script copies the executable to `%ProgramFiles%\Statusbar`, writes `%ProgramData%\Statusbar\client.json` and adds
the autostart entry. It can run as SYSTEM from a deployment tool. `scripts\uninstall-client.ps1` removes everything.

`client.json`:

```json
{ "server_url": "https://status.example.com", "client_token": "<CLIENT_TOKEN>" }
```

Order of configuration: values from the MSI (registry) → `%ProgramData%\Statusbar\client.json` → `client.json` next to
the executable (handy for testing).

## What users see

- **The bar** sits at the top or bottom of the screen as a real desktop toolbar (AppBar): maximised windows move out
  of the way instead of being covered. New messages first appear taller, then shrink to the normal height.
- **Content**: `Label: Title – Message` in white, for example "Outage: E-mail is disrupted – We are working on it."
- **Colours**: red = outage, orange = degraded, blue = notice, green = all-clear, violet = silent alarm.
- **Several messages** take turns every 7 seconds, outages first. An active silent alarm keeps the left part (40 %)
  while the others rotate on the right.
- **All-clear**: the message turns green and disappears after the configured time (default 90 seconds).
- **Sounds**, when switched on: once per new message, for the most urgent one.
- Users cannot dismiss the bar – it disappears when the message is resolved.

Multiple monitors, different DPI, monitors added or removed, resolution changes and Explorer restarts are handled.

## Full-screen windows and Remote Desktop

**Full-screen windows** (presentations, videos, Remote Desktop in full screen): with *Stay visible above full-screen
windows* (default: on) the bar overlays the top or bottom strip of that window. The other window is never moved or
resized. What the bar covers then: at the top, title bars of maximised windows and the connection bar of a remote
session; at the bottom, its taskbar. Switched off, full-screen windows cover the bar.

**When a Remote Desktop session fills a monitor** (classic client `mstsc.exe`):

| Setting | Behaviour |
|---|---|
| *Hide the bar on that monitor* (default) | The local bar steps aside on that monitor. The client on the session host shows the message inside the session. **Requires** the client on the session hosts too – without it, users in a full-screen session see nothing |
| *Show the bar on top of it* | The bar overlays the session like other full-screen windows |
| *Experimental: take the session out of full screen* | While a message is shown, a single-monitor session leaves full screen and is fitted next to the bar; after the all-clear it returns to full screen. Multi-monitor sessions are not touched |

An active silent alarm is always visible, also over full screen and in mode *Hide*.

**Terminal servers**: install the MSI on the session hosts too. The client then runs in every session, reserves space
there like on a local desktop and shows the messages of the signed-in user.

## Connection and outages

- The client holds a long-poll connection to the server; new messages appear in about a second.
- If the server is unreachable, it keeps showing the last known state and retries with growing intervals. It never
  invents an all-clear.
- The client only talks HTTPS (except to `localhost`). The certificate must be valid for Windows.

## Log

`%LOCALAPPDATA%\Statusbar\client.log` in the user's profile: start, configuration errors, connection changes,
decisions about full screen and Remote Desktop, the silent alarm key combination. Never the token.

## Uninstall

```bat
msiexec /x Statusbar.msi /qn
```

Or `uninstall.cmd` from the package, or *Settings → Apps* in Windows. Installed with the script:
`scripts\uninstall-client.ps1`.

## Build the client

Only needed if you build the server yourself or ship the client under your own name.

- **GitHub Actions**: every push builds `statusbar-client.exe` and `Statusbar.msi` (artifact
  `statusbar-client-windows-x64`).
- **Linux, in a container**: `scripts/build-client.sh` → `dist/statusbar-client.exe`.
- **Windows**: `cd client && cargo build --release`.

The builds are unsigned. Sign executable and MSI with your code-signing certificate before distribution:

```bat
signtool sign /fd SHA256 /tr <RFC 3161 timestamp URL> /td SHA256 /sha1 <thumbprint> statusbar-client.exe
```

The server deliberately does not sign: a signing key on a web-reachable machine would be the most valuable thing to
steal in the whole set-up.

Your own name (folders, window class): build the client with `STATUSBAR_APP_NAME=Acme`, the MSI with
`-d Name=Acme -d Manufacturer="Acme Ltd"`, the script with `-Name Acme`. Everything on the bar comes from the server
anyway.
