"""Drive the installed ELIX desktop window through ordinary Windows UI."""
import html
import json
import re
import sqlite3
import struct
import subprocess
import threading
import time
import zlib
from contextlib import closing
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, unquote, urlsplit


def favicon():
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))
    header = struct.pack(">IIBBBBB", 16, 16, 8, 2, 0, 0, 0)
    pixels = (b"\0" + bytes([255, 0, 171]) * 16) * 16
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b"")


class Fixture:
    def __init__(self):
        from media_fixture import response as media_response
        self.log = []
        self.lock = threading.Lock()
        fixture = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def send(self, status, mime, body):
                self.send_response(status)
                self.send_header("Content-Type", mime)
                self.send_header("Cache-Control", "no-store")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                try:
                    self.wfile.write(body)
                except (ConnectionResetError, BrokenPipeError):
                    pass

            def do_GET(self):
                url = urlsplit(self.path)
                if url.path.startswith("/page/"):
                    slug = unquote(url.path[6:])
                    if not re.fullmatch(r"[a-zA-Z0-9 _-]{1,100}", slug):
                        return self.send(400, "text/plain", b"Invalid page")
                    fixture.record({"kind":"load", "slug":slug})
                    title = html.escape("ELIX " + slug)
                    page = f"""<!doctype html><meta charset=utf-8><title>{title}</title>
                      <link rel=icon type=image/png href=/favicon.png>
                      <h1>{title}</h1><p>Installed browser fixture</p>
                      <script>const beat=()=>fetch('/heartbeat?slug='+encodeURIComponent({json.dumps(slug)}));
                      beat();setInterval(beat,300);</script>"""
                    return self.send(200, "text/html; charset=utf-8", page.encode())
                if url.path == "/heartbeat":
                    fixture.record({"kind":"heartbeat", "slug":parse_qs(url.query).get("slug", [""])[0]})
                    return self.send(204, "text/plain", b"")
                if url.path in {"/favicon.png", "/favicon.ico"}:
                    return self.send(200, "image/png", favicon())
                route = media_response(url.path)
                if route is not None:
                    return self.send(200, *route)
                return self.send(404, "text/plain", b"Missing fixture")

            def do_POST(self):
                if self.path != "/event":
                    return self.send(404, "text/plain", b"Missing fixture")
                size = int(self.headers.get("Content-Length", 0))
                if not 0 < size <= 8192:
                    return self.send(400, "text/plain", b"Invalid event")
                try:
                    event = json.loads(self.rfile.read(size))
                    if not isinstance(event, dict):
                        raise ValueError()
                except ValueError:
                    return self.send(400, "text/plain", b"Invalid event")
                fixture.record(event)
                return self.send(204, "text/plain", b"")

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.origin = f"http://127.0.0.1:{self.server.server_port}"
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def record(self, event):
        with self.lock:
            self.log.append(dict(event, time=time.monotonic()))

    @property
    def events(self):
        with self.lock:
            return list(self.log)

    def __enter__(self):
        self.thread.start()
        return self

    def __exit__(self, *_):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=4)


class UI:
    def __init__(self, exe, profile_root, output, fixture):
        self.exe = Path(exe)
        self.profile_root = profile_root
        self.output = output
        self.fixture = fixture
        self.origin = fixture.origin
        self.process = None
        output.mkdir(parents=True, exist_ok=True)

    @staticmethod
    def wait(predicate, message, timeout=12):
        deadline = time.monotonic() + timeout
        last = None
        while time.monotonic() < deadline:
            try:
                result = predicate()
                if result:
                    return result
            except (OSError, RuntimeError, LookupError, AttributeError, sqlite3.Error) as error:
                last = error
            time.sleep(.15)
        raise AssertionError(f"{message} ({last})" if last else message)

    def start(self):
        from pywinauto import Application
        self.process = subprocess.Popen([str(self.exe)])
        self.app = Application(backend="uia").connect(process=self.process.pid, timeout=30)
        self.window = self.app.window(title="ELIX Browser", control_type="Window")
        self.window.wait("visible", timeout=60)
        self.window.set_focus()
        self.find("Adressefelt", timeout=75)
        self.wait(self.session, "The new profile did not persist its initial session")

    def stop(self):
        if self.process is None:
            return
        if self.process.poll() is None:
            try:
                self.window.close()
            except Exception:
                pass
            try:
                self.process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                self.process.terminate()
                self.process.wait(timeout=10)
        self.process = None

    def restart(self):
        self.stop()
        self.start()

    def find(self, name, control_type=None, timeout=10, root=None):
        def match():
            source = root if root is not None else self.window
            options = {"title": name}
            if control_type is not None:
                options["control_type"] = control_type
            return next((c for c in source.descendants(**options) if c.is_visible() and
                         c.rectangle().width() > 0 and c.rectangle().height() > 0), None)
        return self.wait(match, f"Missing visible UI control {name}", timeout)

    def click(self, name, control_type=None):
        control = self.find(name, control_type)
        control.click_input()
        return control

    @staticmethod
    def keys(keys):
        from pywinauto.keyboard import send_keys
        send_keys(keys, with_spaces=True, pause=.035)

    def paste(self, text):
        import win32clipboard
        import win32con
        def copy():
            win32clipboard.OpenClipboard()
            try:
                win32clipboard.EmptyClipboard()
                win32clipboard.SetClipboardData(win32con.CF_UNICODETEXT, text)
            finally:
                win32clipboard.CloseClipboard()
            return True
        self.wait(copy, "Could not set the synthetic clipboard")
        self.keys("^v")

    def navigate(self, url):
        address = self.find("Adressefelt")
        address.click_input()
        self.keys("^a")
        self.paste(str(url))
        self.wait(lambda: address.iface_value.CurrentValue == str(url), "Address input lost characters")
        self.keys("{ENTER}")

    def new_tab(self, url=None):
        previous = len(self.session()["tabs"])
        self.keys("^t")
        self.wait(lambda: len(self.session()["tabs"]) == previous + 1, "New tab did not open")
        if url is not None:
            self.navigate(url)

    def session(self):
        db = self.profile_root / "elix.sqlite"
        if not db.is_file():
            raise RuntimeError("The profile database has not appeared")
        with closing(sqlite3.connect(db.as_uri() + "?mode=ro", uri=True, timeout=3)) as connection:
            row = connection.execute("SELECT data FROM sessions WHERE profile=(SELECT value FROM settings WHERE key='active_profile')").fetchone()
        if not row:
            raise RuntimeError("The current window has no saved session yet")
        return json.loads(row[0])

    def page(self, slug):
        from urllib.parse import quote
        return self.origin + "/page/" + quote(slug)

    def screenshot(self, name):
        from PIL import ImageGrab
        if not re.fullmatch(r"[A-Za-z0-9_.-]+", name):
            raise ValueError("Invalid screenshot label")
        path = self.output / (name if name.endswith(".png") else name + ".png")
        ImageGrab.grab(all_screens=True).save(path)
        return path
