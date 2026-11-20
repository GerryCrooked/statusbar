#!/usr/bin/env python3
"""Statusbar for Atera: polls Atera's open alerts and forwards them to each customer's Statusbar.

Atera has no outgoing webhooks on most plans, so this script runs at the service provider – every two
minutes from cron, a systemd timer or the Windows task scheduler – with one Atera API key and a list of
customers, each with the webhook URL of that customer's Statusbar integration.

* a new open alert of a listed customer opens a message (key atera-<AlertID>)
* an alert that is archived (resolved) or gone sends the all-clear
* if Atera cannot be reached, nothing is resolved

    atera-statusbar.py --config /etc/statusbar/atera.json [--dry-run] [-v]

Configuration (JSON):
    {
      "api_key_file": "/etc/statusbar/atera-api-key",
      "state_file": "/var/lib/statusbar/atera-state.json",
      "severities": ["Critical", "Warning"],
      "customers": {
        "Example GmbH": "https://status.example.com/api/hooks/sb_...",
        "1234":         "https://status.other.example/api/hooks/sb_..."
      }
    }
Customers are matched by Atera customer name or ID. Python 3.8+, standard library only.
"""
import argparse
import json
import os
import sys
import tempfile
import urllib.error
import urllib.request

API = "https://app.atera.com/api/v3"
SEVERITY = {"critical": "error", "warning": "warning", "information": "info"}


def log(msg, *, verbose=True):
    if verbose:
        print(msg, file=sys.stderr)


def http_json(url, *, headers=None, body=None, timeout=20):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method="POST" if data else "GET")
    for k, v in (headers or {}).items():
        req.add_header(k, v)
    if data:
        req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, timeout=timeout) as res:
        return json.loads(res.read().decode() or "null")


def open_alerts(api, key):
    """All alerts that are not archived, following Atera's paging."""
    alerts, page = [], 1
    while True:
        res = http_json(f"{api}/alerts?page={page}&itemsInPage=50", headers={"X-API-KEY": key, "Accept": "application/json"})
        items = res.get("items") or []
        alerts += [a for a in items if not a.get("Archived")]
        if page >= int(res.get("totalPages") or 1) or not items or page >= 200:
            return alerts
        page += 1


def event(alert, problem):
    title = (alert.get("Title") or alert.get("AlertMessage") or "Atera alert").strip()
    e = {
        "key": f"atera-{alert['AlertID']}",
        "status": "problem" if problem else "ok",
        "host": (alert.get("DeviceName") or "").strip(),
        "service": title,
    }
    if problem:
        e["title"] = title
        e["message"] = (alert.get("AlertMessage") or alert.get("AdditionalInfo") or "").strip()[:1500]
        e["severity"] = SEVERITY.get(str(alert.get("Severity", "")).lower(), "error")
    return e


def load_state(path):
    try:
        with open(path) as f:
            return json.load(f)
    except FileNotFoundError:
        return {}


def save_state(path, state):
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path) or ".")
    with os.fdopen(fd, "w") as f:
        json.dump(state, f, indent=1, sort_keys=True)
    os.replace(tmp, path)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--config", required=True)
    ap.add_argument("--dry-run", action="store_true", help="print what would be sent, change nothing")
    ap.add_argument("-v", "--verbose", action="store_true")
    args = ap.parse_args()

    with open(args.config) as f:
        cfg = json.load(f)
    with open(cfg["api_key_file"]) as f:
        api_key = f.read().strip()
    api = cfg.get("api_url", API).rstrip("/")
    severities = {s.lower() for s in cfg.get("severities", ["Critical"])}
    customers = {str(k).lower(): v for k, v in cfg["customers"].items()}
    state_file = cfg.get("state_file", "/var/lib/statusbar/atera-state.json")

    try:
        alerts = open_alerts(api, api_key)
    except (urllib.error.URLError, OSError, ValueError) as e:
        print(f"atera-statusbar: cannot read alerts from Atera: {e}", file=sys.stderr)
        return 1

    # state: alert ID -> webhook URL it was sent to
    state = load_state(state_file)
    current = {}
    for a in alerts:
        url = customers.get(str(a.get("CustomerName", "")).lower()) or customers.get(str(a.get("CustomerID", "")))
        if url and str(a.get("Severity", "")).lower() in severities and a.get("AlertID") is not None:
            current[str(a["AlertID"])] = (url, a)

    failed = 0
    sends = [(aid, url, event(a, True)) for aid, (url, a) in current.items() if aid not in state]
    sends += [(aid, url, event({"AlertID": aid}, False)) for aid, url in state.items() if aid not in current]
    for aid, url, body in sends:
        if args.dry_run:
            print(json.dumps(body))
            continue
        try:
            answer = http_json(url, body=body)
            log(f"{body['key']} {body['status']}: {answer}", verbose=args.verbose)
        except (urllib.error.URLError, OSError, ValueError) as e:
            print(f"atera-statusbar: {body['key']} {body['status']} not delivered: {e}", file=sys.stderr)
            failed += 1
            continue  # retried on the next run
        if body["status"] == "problem":
            state[aid] = url
        else:
            state.pop(aid, None)
    if not args.dry_run:
        save_state(state_file, state)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
