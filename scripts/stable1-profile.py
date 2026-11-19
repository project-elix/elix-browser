"""Seed or check synthetic Alpha 9 profile data on a disposable Windows runner."""
import argparse
import json
from pathlib import Path
import sqlite3

parser = argparse.ArgumentParser()
parser.add_argument("action", choices=["seed", "verify"])
parser.add_argument("root", type=Path)
parser.add_argument("--channel", default="developer", choices=["stable", "developer"])
args = parser.parse_args()
database = args.root / "elix.sqlite"
if not database.is_file():
    raise SystemExit("The installed browser did not create its database")

with sqlite3.connect(database) as db:
    db.execute("PRAGMA foreign_keys=ON")
    profile, = db.execute("SELECT value FROM settings WHERE key='active_profile'").fetchone()
    name = "Stable 1 acceptance"
    url = "https://example.org/stable1-acceptance"
    marker = args.root / "profiles" / profile / "webview2" / "stable1-fixture.txt"
    if args.action == "seed":
        preferences = json.loads(dict(db.execute("SELECT key,value FROM settings")).get("preferences", "{}"))
        preferences.update({"gaming": True, "channel": args.channel})
        db.execute("INSERT OR REPLACE INTO settings VALUES('preferences',?)", (json.dumps(preferences),))
        db.execute("INSERT INTO bookmark_groups VALUES(?,?)", (profile, name))
        bookmark = db.execute(
            "INSERT INTO bookmarks(profile,url,title,time) VALUES(?,?,?,CURRENT_TIMESTAMP)",
            (profile, url, name),
        ).lastrowid
        db.execute("INSERT INTO bookmark_group_entries VALUES(?,?,?)", (bookmark, profile, name))
        db.execute("INSERT INTO history(profile,url,title,time) VALUES(?,?,?,CURRENT_TIMESTAMP)", (profile, url, name))
        marker.parent.mkdir(parents=True, exist_ok=True)
        marker.write_text(profile, encoding="utf-8")
    else:
        preferences = json.loads(dict(db.execute("SELECT key,value FROM settings"))["preferences"])
        if preferences.get("gaming") is not True or preferences.get("channel") != args.channel:
            raise SystemExit("Existing preferences or channel changed")
        bookmark = db.execute(
            "SELECT b.title,g.name FROM bookmarks b JOIN bookmark_group_entries g "
            "ON b.id=g.bookmark AND b.profile=g.profile WHERE b.profile=? AND b.url=?",
            (profile, url),
        ).fetchall()
        history = db.execute("SELECT title FROM history WHERE profile=? AND url=?", (profile, url)).fetchall()
        if bookmark != [(name, name)] or history != [(name,)] or marker.read_text(encoding="utf-8") != profile:
            raise SystemExit("Bookmark, group, history, profile identity or profile file changed")
        print("Verified the existing active profile, grouped bookmark, history, preferences, channel and profile file.")
