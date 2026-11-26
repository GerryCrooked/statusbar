#!/usr/bin/env python3
"""Minimal stand-in for a ticket system that takes Statusbar's generic ticket webhook, for
testing outbound tickets without a real ticket system.

  fake-ticket.py --port 18090 --dir <folder> [--token <bearer token>]

Every POST to /hook is appended to <folder>/requests.jsonl as {"auth": …, "body": …}.
A body with "event": "created" or "test" gets a new ticket number; the answer is
{"id": <n>, "url": "http://<host>/tickets/<n>"}, which Statusbar shows with the message.
"""
import argparse
import json
import os
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

args = argparse.ArgumentParser()
args.add_argument("--port", type=int, required=True)
args.add_argument("--dir", required=True)
args.add_argument("--token", default="")
opts = args.parse_args()
log_file = os.path.join(opts.dir, "requests.jsonl")
lock = threading.Lock()
counter = [0]


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path != "/hook":
            return self.answer(404, {"error": "unknown path"})
        auth = self.headers.get("Authorization", "")
        if opts.token and auth != "Bearer " + opts.token:
            return self.answer(401, {"error": "token"})
        raw = self.rfile.read(int(self.headers.get("Content-Length") or 0))
        try:
            body = json.loads(raw)
        except ValueError:
            return self.answer(400, {"error": "not JSON"})
        with lock:
            with open(log_file, "a", encoding="utf-8") as f:
                f.write(json.dumps({"auth": auth, "body": body}, ensure_ascii=False) + "\n")
            if body.get("event") in ("created", "test"):
                counter[0] += 1
            number = counter[0]
        host = self.headers.get("Host", "localhost")
        self.answer(200, {"id": number, "url": f"http://{host}/tickets/{number}"})

    def answer(self, status, payload):
        data = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *a):
        pass


ThreadingHTTPServer(("0.0.0.0", opts.port), Handler).serve_forever()
