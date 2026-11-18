"""Qualify both public paths to the same immutable release asset."""
import hashlib
import json
from pathlib import Path
import sqlite3
import sys
import time
from urllib.parse import urlencode, urlsplit
from urllib.request import Request, urlopen
import uuid

NAME = "ELIX.Browser_0.1.4-dev.9_x64-setup.exe.sig"
DIGEST = "887626e57d600493ea804996b61f42f58c24fa05fa70c94fea313119e7c3ed29"
RELEASE_URL = "https://github.com/project-elix/elix-browser/releases/download/v0.1.4-dev.9/" + NAME


def fresh_links():
    # A different query on every run prevents a cached, expired 302.
    source = RELEASE_URL + "?" + urlencode({"elix_check": uuid.uuid4().hex})
    request = Request(source, headers={"User-Agent": "ELIX-Windows-acceptance", "Cache-Control": "no-cache"})
    with urlopen(request, timeout=40) as response:
        cdn = response.geturl()
        parsed = urlsplit(cdn)
        if parsed.scheme != "https" or parsed.hostname != "release-assets.githubusercontent.com" or not parsed.query:
            raise ValueError("GitHub did not provide a signed public release-assets redirect")
        if hashlib.sha256(response.read()).hexdigest() != DIGEST:
            raise ValueError("GitHub release asset digest mismatch")
    print(json.dumps([source, cdn]))


def verify(profile_root, expected):
    database = profile_root / "downloads.sqlite3"
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        if database.is_file():
            with sqlite3.connect(database, timeout=5) as db:
                records = [json.loads(row[0]) for row in db.execute("SELECT data FROM downloads")]
            completed = [item for item in records if item["status"] == "complete" and item["name"] == NAME]
            if len(completed) == expected:
                for item in completed:
                    path = Path(item["path"])
                    if path.is_file() and hashlib.sha256(path.read_bytes()).hexdigest() == DIGEST:
                        continue
                    raise ValueError("ELIX completed a download with missing or incorrect bytes")
                if len({item["path"] for item in completed}) != expected:
                    raise ValueError("Separate downloads did not create distinct files")
                return
        time.sleep(0.25)
    raise TimeoutError(f"Expected {expected} completed copies of the public release asset")


if __name__ == "__main__":
    if sys.argv[1:] == ["links"]:
        fresh_links()
    elif len(sys.argv) == 4 and sys.argv[1] == "verify":
        verify(Path(sys.argv[2]), int(sys.argv[3]))
    else:
        raise SystemExit("Choose links or verify PROFILE_ROOT DOWNLOAD_COUNT")
