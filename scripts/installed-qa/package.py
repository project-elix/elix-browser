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


def installer_pair(directory, evidence):
    files = list(directory.iterdir())
    installers = [p for p in files if p.is_file() and p.name.endswith("_x64-setup.exe")]
    require(len(files) == 2 and len(installers) == 1, "Expected the two original files from the signed Windows build")
    exe = installers[0]
    signature = exe.with_suffix(".exe.sig")
    require(signature.is_file(), "The installer artifact has no matching detached signature")
    require(sha256(exe) == evidence["installer_sha256"].lower(), "Installer SHA-256 differs from the original signed release job")
    require(signature.read_text().strip() == evidence["signature"].strip(), "Signature differs from the signed release review")
    return exe, signature


def acquire(args):
    repository = os.environ["GITHUB_REPOSITORY"]
    run_id = os.environ["QUALIFY_RELEASE_RUN"]
    source = os.environ["QUALIFY_SOURCE_SHA"]
    require(re.fullmatch(r"[1-9][0-9]*", run_id) and re.fullmatch(r"[0-9a-f]{40}", source), "Invalid run or commit")
    reviews = list(args.review.rglob("stable1-profile.json"))
    require(len(reviews) == 1, "Expected exactly one verified release review")
    evidence = json.loads(reviews[0].read_text(encoding="utf-8-sig"))
    metadata = subprocess.check_output(["gh", "api", f"repos/{repository}/actions/runs/{run_id}"], encoding="utf-8")
    identity(json.loads(metadata), evidence, source, run_id, repository)
    installer, signature = installer_pair(args.installer_artifact, evidence)
    args.output.mkdir(parents=True, exist_ok=False)
    exe = args.output / "candidate.exe"
    original_name = installer.name
    installer.rename(exe)
    signature.rename(exe.with_suffix(".exe.sig"))
    verified_source = args.verifier.resolve()
    subprocess.run(["node", "scripts/verify-installer-signature.mjs", str(exe.resolve()), "0.1.4"], cwd=verified_source, check=True)
    record = {
        "version": "0.1.4", "source_sha": source, "release_run": run_id, "release_attempt": evidence["run_attempt"],
        "qa_run": os.environ["GITHUB_RUN_ID"], "qa_attempt": os.environ["GITHUB_RUN_ATTEMPT"], "qa_source": os.environ["GITHUB_SHA"],
        "installer_artifact": "signed-stable1-installer", "installer_filename": original_name,
        "installer_sha256": sha256(exe), "exe_sha256": evidence["exe_sha256"].lower(),
        "updater_signature": "verified", "ELIX-34": "passed on original signed install",
    }
    (args.output / "identity.json").write_text(json.dumps(record, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--review", type=Path, required=True)
    parser.add_argument("--installer-artifact", type=Path, required=True)
    parser.add_argument("--verifier", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    acquire(parser.parse_args())
