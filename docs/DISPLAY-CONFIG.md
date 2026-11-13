# Windows display configuration (v0.4.2 pilot)

The Linux server reads these settings from `.env` and sends them with every authenticated `/api/client/state` response. Client configuration (`client.json`) remains unchanged.

| Variable | Default | Description |
|---|---|---|
| `BAR_POSITION` | `bottom` | `top` or `bottom` screen edge |
| `BAR_ALL_MONITORS` | `true` | `true` displays one AppBar on each detected monitor; `false` only primary |
| `BAR_HEIGHT_PX` | `44` | Normal height in pixels (24–160) |
| `BAR_INITIAL_HEIGHT_PX` | `88` | Height for a newly active incident (between normal height and 300) |
| `BAR_INITIAL_SECONDS` | `5` | Number of seconds to keep enlarged size for a new active incident (0–60; 0 disables) |
| `RESOLVED_DISPLAY_SECONDS` | `90` | How long a resolved incident is returned by the server |

Example:

```dotenv
BAR_POSITION=bottom
BAR_ALL_MONITORS=true
BAR_HEIGHT_PX=44
BAR_INITIAL_HEIGHT_PX=88
BAR_INITIAL_SECONDS=5
RESOLVED_DISPLAY_SECONDS=90
```

After changing `.env`, recreate the server container: `docker compose up -d --force-recreate status-server` (no rebuild necessary for env-only changes). Clients apply the new display configuration at their next poll (default 10 seconds, configurable in `client.json`).

## Limitations

- One AppBar window is created for each display found **when the client starts**. After attaching/detaching a monitor, restart the client to enumerate the new topology. Display mode and position changes while running are supported for already discovered monitors.
- Windows shell/taskbar behavior may vary by Windows version and third-party taskbars. Test with multiple monitors, mixed DPI/scaling, top taskbars, fullscreen applications, and RDS before rollout.
- Initial emphasis starts for newly active incident IDs, not on each poll. It does not repeatedly enlarge during a long incident; a resolved notification appears at normal height.
- A malicious or untrusted client must not be allowed to impersonate another AD user; the pilot's shared client token is not sufficient for sensitive or emergency alert targeting.
