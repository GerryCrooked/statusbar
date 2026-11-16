#!/usr/bin/env python3
"""Minimal stand-in for a service-provider hub, for testing the hub connection of a
Statusbar server without the commercial hub.

  fake-hub.py --port 18080 --token <token> --dir <folder>

<folder>/messages.json  messages to hand out (edit it while running; list of objects
                        with id, title, message, severity, status, teams)
<folder>/report.json    the last report the server sent
"""
import argparse
import json
import os
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

args = argparse.ArgumentParser()
args.add_argument("--port", type=int, required=True)
args.add_argument("--token", required=True)
args.add_argument("--dir", required=True)
args.add_argument("--name", default="Test hub")
opts = args.parse_args()
messages_file = os.path.join(opts.dir, "messages.json")
report_file = os.path.join(opts.dir, "report.json")


def revision():
    try:
        return int(os.stat(messages_file).st_mtime_ns // 1000)
    except FileNotFoundError:
        return 1


def messages():
    try:
        with open(messages_file, encoding="utf-8") as f:
            return json.load(f)
    except (FileNotFoundError, ValueError):
        return []


class Hub(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def reply(self, code, body):
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        url = urlparse(self.path)
        if url.path != "/api/hub/sync":
            return self.reply(404, {"error": "not found"})
        if self.headers.get("Authorization", "") != "Bearer " + opts.token:
            return self.reply(401, {"error": "unknown customer token"})
        length = int(self.headers.get("Content-Length", "0"))
        report = self.rfile.read(length)
        with open(report_file, "wb") as f:
            f.write(report)
        query = parse_qs(url.query)
        rev = query.get("rev", [None])[0]
        wait = min(int(query.get("wait", ["25"])[0]), 55)
        deadline = time.time() + wait
        while rev is not None and str(revision()) == rev and time.time() < deadline:
            time.sleep(0.2)
        self.reply(200, {"rev": revision(), "hub": opts.name, "messages": messages()})


ThreadingHTTPServer(("127.0.0.1", opts.port), Hub).serve_forever()
