"""Bind a Stable 1 updater-signed installer to its original release-run evidence."""
import argparse
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path


def require(condition, explanation):
    if not condition:
        raise ValueError(explanation)


def gh(*args):
    return json.loads(subprocess.check_output(["gh", "api", *args], encoding="utf-8"))


def sha256(path):
    with path.open("rb") as file:
        return hashlib.file_digest(file, "sha256").hexdigest()


def identity(run, evidence, source, run_id, repository):
    require(run.get("head_sha") == source and run.get("head_branch") == "main"
            and run.get("repository", {}).get("full_name") == repository
            and run.get("path") == ".github/workflows/release.yml" and run.get("event") == "workflow_dispatch",
            "This is not a main-branch release build for the requested source.")
    require(str(run["id"]) == run_id == str(evidence.get("run_id"))
            and str(run["run_attempt"]) == str(evidence.get("run_attempt"))
            and source == evidence.get("source_sha") and evidence.get("version") == "0.1.4",
            "The signed installation evidence does not belong to this run, attempt and commit.")
    require(evidence.get("profile_upgrade") == "passed"
            and evidence.get("elix_34_github_release_and_fresh_cdn") == "passed",
            "The original signed production upgrade and release/CDN checks did not pass.")
    for key in ["installer_sha256", "exe_sha256"]:
        require(bool(re.fullmatch(r"[a-fA-F0-9]{64}", str(evidence.get(key, "")))), "Missing " + key)
    require(bool(evidence.get("signature")), "The production review does not contain a verified updater signature.")


def acquire(args):
    repository = os.environ["GITHUB_REPOSITORY"]
    run_id = os.environ["QUALIFY_RELEASE_RUN"]
    source = os.environ["QUALIFY_SOURCE_SHA"]
    require(re.fullmatch(r"[1-9][0-9]*", run_id) and re.fullmatch(r"[0-9a-f]{40}", source), "Invalid run or commit")
    reviews = list(args.review.rglob("stable1-profile.json"))
    require(len(reviews) == 1, "Expected exactly one verified release review")
    evidence = json.loads(reviews[0].read_text(encoding="utf-8-sig"))
    identity(gh(f"repos/{repository}/actions/runs/{run_id}"), evidence, source, run_id, repository)
    # The release may still be awaiting publication. Published and draft assets use the same IDs.
    pages = gh("--paginate", "--slurp", f"repos/{repository}/releases?per_page=100")
    releases = [r for page in pages for r in page if r.get("tag_name") == "v0.1.4" and r.get("target_commitish") == source]
    require(len(releases) == 1, "Expected one Stable 1 release or private draft targeting this exact commit")
    release = releases[0]
    assets = release.get("assets", [])
    installers = [a for a in assets if a["name"].endswith("_x64-setup.exe")]
    require(len(installers) == 1, "Expected one Windows x64 installer")
    installer = installers[0]
    signatures = [a for a in assets if a["name"] == installer["name"] + ".sig"]
    require(len(signatures) == 1, "Missing the matching updater signature")
    args.output.mkdir(parents=True, exist_ok=False)
    for asset, name in [(installer, "candidate.exe"), (signatures[0], "candidate.exe.sig")]:
        path = args.output / name
        require(asset["state"] == "uploaded" and int(asset["id"]) > 0, "Incomplete release asset")
        with path.open("xb") as out:
            subprocess.run(["gh","api","-H","Accept: application/octet-stream",
                            f"repos/{repository}/releases/assets/{int(asset['id'])}"], stdout=out, check=True)
        require(path.stat().st_size == asset["size"], "Truncated release asset")
    exe = args.output / "candidate.exe"
    require(sha256(exe) == evidence["installer_sha256"].lower(), "Installer SHA-256 differs from the original signed release job")
    require((args.output / "candidate.exe.sig").read_text().strip() == evidence["signature"].strip(),
            "Signature differs from the original signed release job")
    verified_source = args.verifier.resolve()
    subprocess.run(["node", "scripts/verify-installer-signature.mjs", str(exe.resolve()), "0.1.4"], cwd=verified_source, check=True)
    record = {
        "version": "0.1.4", "source_sha": source, "release_run": run_id, "release_attempt": evidence["run_attempt"],
        "qa_run": os.environ["GITHUB_RUN_ID"], "qa_attempt": os.environ["GITHUB_RUN_ATTEMPT"], "qa_source": os.environ["GITHUB_SHA"],
        "installer_asset": installer["id"], "signature_asset": signatures[0]["id"],
        "installer_sha256": sha256(exe), "exe_sha256": evidence["exe_sha256"].lower(),
        "updater_signature": "verified", "ELIX-34": "passed on original signed install",
    }
    (args.output / "identity.json").write_text(json.dumps(record, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--review", type=Path, required=True)
    parser.add_argument("--verifier", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    acquire(parser.parse_args())
