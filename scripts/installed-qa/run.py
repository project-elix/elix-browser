"""Collect independent desktop observations for the exact installed Stable 1 binary."""
import json
import os
import subprocess
import sys
import tempfile
import time
import traceback
from contextlib import contextmanager
from pathlib import Path

from package import require, sha256
from harness import Fixture, UI


def retry_file_unlock(operation):
    deadline = time.monotonic() + 20
    while True:
        try:
            return operation()
        except PermissionError:
            if time.monotonic() >= deadline:
                raise
            time.sleep(.4)


@contextmanager
def isolated_profile(profile_root):
    require(not os.path.lexists(profile_root), "Refusing to use an existing browser profile")
    # Hosted Windows puts LOCALAPPDATA on C: and RUNNER_TEMP on D:. An atomic
    # directory rename is possible only on the profile's own volume.
    quarantine = tempfile.TemporaryDirectory(prefix="elix-installed-qa-", dir=profile_root.parent)
    try:
        yield
    finally:
        try:
            if profile_root.exists():
                destination = Path(quarantine.name) / "synthetic-profile"
                retry_file_unlock(lambda: profile_root.rename(destination))
        finally:
            retry_file_unlock(quarantine.cleanup)


def report(output, record):
    target = output / "signed-package-results.json"
    pending = target.with_suffix(".tmp")
    pending.write_text(json.dumps(record, indent=2, ensure_ascii=False, default=str), encoding="utf-8")
    pending.replace(target)


def main(package_dir, output):
    require(os.name == "nt" and os.environ.get("GITHUB_ACTIONS") == "true" and
            os.environ.get("ELIX_RUNNER_ENVIRONMENT") == "github-hosted", "A fresh hosted Windows desktop is required.")
    from groups import CASES as groups
    from media import CASES as media
    from shell import CASES as shell
    checks = {**groups, **media, **shell}
    expected = {f"ELIX-{n}" for n in (17,18,19,20,21,23,25,26,27,28,29,33)}
    require(set(checks) == expected, "Missing signed-package case")
    temp = Path(os.environ["RUNNER_TEMP"]).resolve()
    profile_root = Path(os.environ["LOCALAPPDATA"]) / "no.projectelix.browser"
    install = temp / "stable1-signed-ui-install"
    require(output.resolve().parent == temp and package_dir.resolve().parent == temp, "Unexpected runner path")
    require(not any(p.exists() for p in (profile_root, install, output)),
            "The runner already contains a profile or evidence directory")
    output.mkdir()
    identity = json.loads((package_dir / "identity.json").read_text())
    installer = package_dir / "candidate.exe"
    require(sha256(installer) == identity["installer_sha256"], "Installer changed after verification")
    subprocess.run([str(installer), "/S", f"/D={install}"], check=True, timeout=120)
    exe = install / "elix-browser.exe"
    require(sha256(exe) == identity["exe_sha256"], "Installed executable differs from the signed upgrade review")
    result = {"identity": identity, "cases": {card: {"status": "not_run"} for card in sorted(checks)}}
    result["cases"]["ELIX-34"] = {"status": "passed", "measurement": {
        "evidence": "original signed-stable1-profile-review", "release_run": identity["release_run"]}}
    report(output, result)
    for card in sorted(checks, key=lambda name: int(name.split("-")[1])):
        print("Starting", card, flush=True)
        with isolated_profile(profile_root), Fixture() as fixture:
            ui = UI(exe, profile_root, output / card, fixture)
            try:
                ui.start()
                measurement = checks[card](ui)
                if not isinstance(measurement, dict) or not measurement:
                    raise ValueError("Test returned no measured outcome")
                status = {"status": "passed", "measurement": measurement}
            except Exception as error:
                try:
                    ui.screenshot(card + "-failure")
                except Exception:
                    pass
                status = {"status": "failed", "reason": f"{type(error).__name__}: {error}"}
                traceback.print_exc()
            finally:
                try:
                    ui.stop()
                except Exception as error:
                    traceback.print_exc()
                    status = {**status, "status": "failed", "cleanup_error": str(error)}
            result["cases"][card] = status
            report(output, result)
        print(card, status["status"], flush=True)
    table = ["### Stable 1 signed desktop checks", "",
             f"Release {identity['release_run']} · source `{identity['source_sha']}` · installer `{identity['installer_sha256']}`",
             "", "| Card | Result |", "| --- | --- |"]
    table.extend(f"| {name} | {item['status']} |" for name,item in result["cases"].items())
    with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as summary:
        summary.write("\n".join(table) + "\n")
    failures = [name for name,outcome in result["cases"].items() if outcome["status"] != "passed"]
    require(not failures, "Signed production checks failed: " + ", ".join(failures))


if __name__ == "__main__":
    main(Path(sys.argv[1]), Path(sys.argv[2]))
