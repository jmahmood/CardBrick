"""Tiny HTTP service used by CardBrick devices on the family LAN."""

import argparse
import hashlib
import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import unquote, urlsplit

try:
    from .store import Store, normalize_device_name
except ImportError:  # direct execution: python cardbrick_server/server.py
    from store import Store, normalize_device_name


class CardBrickHandler(BaseHTTPRequestHandler):
    store = None
    server_version = "CardBrickSync/0.1"

    def log_message(self, fmt, *args):
        print("%s - %s" % (self.address_string(), fmt % args))

    def _json(self, status, value):
        body = json.dumps(value, sort_keys=True).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _parts(self):
        return [unquote(part) for part in urlsplit(self.path).path.split("/")
                if part]

    def _read_json(self):
        length = int(self.headers.get("Content-Length", "0"))
        if length > 1024 * 1024:
            raise ValueError("JSON request is too large")
        data = self.rfile.read(length) if length else b"{}"
        return json.loads(data.decode("utf-8"))

    def do_GET(self):
        try:
            parts = self._parts()
            if parts == ["health"]:
                self._json(200, {"status": "ok"})
                return
            if len(parts) == 3 and parts[0] == "devices" and \
                    parts[2] == "manifest":
                name = normalize_device_name(parts[1])
                self._json(200, {"device": name,
                                 "content": self.store.manifest(name)})
                return
            if len(parts) == 2 and parts[0] == "files" and parts[1].isdigit():
                path = self.store.content_path(int(parts[1]))
                if not path:
                    self._json(404, {"error": "content not found"})
                    return
                size = path.stat().st_size
                self.send_response(200)
                self.send_header("Content-Type", "application/octet-stream")
                self.send_header("Content-Length", str(size))
                self.send_header("Content-Disposition",
                                 'attachment; filename="%s"' % path.name)
                self.end_headers()
                with open(path, "rb") as source:
                    for chunk in iter(lambda: source.read(1024 * 1024), b""):
                        self.wfile.write(chunk)
                return
            self._json(404, {"error": "not found"})
        except (ValueError, json.JSONDecodeError) as exc:
            self._json(400, {"error": str(exc)})
        except Exception as exc:  # keep the small server available
            self._json(500, {"error": str(exc)})

    def do_POST(self):
        try:
            parts = self._parts()
            if len(parts) == 3 and parts[0] == "devices" and \
                    parts[2] == "check-in":
                data = self._read_json()
                name = self.store.touch_device(
                    parts[1], data.get("app_version"),
                    data.get("schema_version"))
                self._json(200, {"device": name, "status": "ok"})
                return
            if len(parts) == 3 and parts[0] == "devices" and \
                    parts[2] == "report":
                data = self._read_json()
                self.store.report(parts[1], data["content_id"],
                                  data["status"], data.get("error"))
                self._json(200, {"status": "ok"})
                return
            self._json(404, {"error": "not found"})
        except (KeyError, ValueError, json.JSONDecodeError) as exc:
            self._json(400, {"error": str(exc)})
        except Exception as exc:
            self._json(500, {"error": str(exc)})

    def do_PUT(self):
        partial = None
        try:
            parts = self._parts()
            if len(parts) != 4 or parts[0] != "devices" or \
                    parts[2] != "backups":
                self._json(404, {"error": "not found"})
                return
            length = int(self.headers.get("Content-Length", "-1"))
            if length < 0:
                raise ValueError("Content-Length is required")
            expected = self.headers.get("X-CardBrick-SHA256", "").lower()
            if len(expected) != 64:
                raise ValueError("X-CardBrick-SHA256 is required")
            name, destination = self.store.backup_destination(parts[1], parts[3])
            partial = self.store.tmp_dir / (name + "-" + destination.name +
                                             ".partial")
            digest = hashlib.sha256()
            remaining = length
            with open(partial, "wb") as target:
                while remaining:
                    chunk = self.rfile.read(min(1024 * 1024, remaining))
                    if not chunk:
                        raise ValueError("backup upload ended early")
                    target.write(chunk)
                    digest.update(chunk)
                    remaining -= len(chunk)
                target.flush()
                os.fsync(target.fileno())
            actual = digest.hexdigest()
            if actual != expected:
                raise ValueError("backup checksum mismatch")
            os.replace(str(partial), str(destination))
            self.store.register_backup(name, destination, actual)
            self._json(201, {"status": "stored", "filename": destination.name,
                             "sha256": actual, "size": length})
        except (ValueError, OSError) as exc:
            if partial and partial.exists():
                partial.unlink()
            self._json(400, {"error": str(exc)})
        except Exception as exc:
            if partial and partial.exists():
                partial.unlink()
            self._json(500, {"error": str(exc)})


def make_server(root, host="0.0.0.0", port=6429):
    store = Store(root)
    handler = type("ConfiguredCardBrickHandler", (CardBrickHandler,),
                   {"store": store})
    server = ThreadingHTTPServer((host, int(port)), handler)
    server.store = store
    return server


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=os.environ.get(
        "CARDBRICK_SERVER_ROOT", "/srv/cardbrick"))
    parser.add_argument("--host", default="0.0.0.0")
    parser.add_argument("--port", type=int, default=6429)
    args = parser.parse_args(argv)
    server = make_server(args.root, args.host, args.port)
    print("CardBrick sync server: http://%s:%d" % server.server_address)
    print("Storage: %s" % server.store.root)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
