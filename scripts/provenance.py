"""
The run manifest: everything needed to say what a score actually measured.

A metrics table without provenance is a number nobody can defend. When nDCG@10
moves, the manifest is what distinguishes "tarn's ranking changed" from "the
binary was built without the stemming feature" from "upstream re-cut the
corpus". Each of those moves the score, and only one of them is a regression.

Pinned deliberately:

  tarn.version   from Cargo.toml. The obvious source would be the handshake's
                 serverInfo, but rmcp fills that with `Implementation::from_build_env()`,
                 which resolves CARGO_PKG_* at *rmcp's* compile time -- so tarn
                 advertises itself as "rmcp 1.2.0". The advertised value is kept
                 alongside as `server_info` rather than dropped, because it is
                 what a real MCP client sees.
  tarn.commit    the working tree that built it, plus a dirty flag, because
                 serverInfo alone cannot distinguish two builds of 0.9.1
  tarn.features  the default `stemming` feature changes tokenization and
                 therefore every BM25 score
  dataset.sha256 BEIR ships no upstream version, so content is the version
  params         limit and score_threshold change what comes back at all
  machine        latency figures are meaningless without it
"""

import hashlib
import json
import os
import platform
import re
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from paths import ROOT, eval_dir

Json = dict[str, Any]


def _git(*args: str) -> str:
    try:
        return subprocess.run(
            ["git", *args],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return ""


def git_commit() -> str:
    return _git("rev-parse", "--short", "HEAD") or "unknown"


def git_dirty() -> bool:
    return bool(_git("status", "--porcelain"))


def cargo_version() -> str:
    """Version of the crate under test, from Cargo.toml's [package] block."""
    text = (ROOT / "Cargo.toml").read_text()
    package = text.split("[package]", 1)[-1].split("\n[", 1)[0]
    match = re.search(r'^version\s*=\s*"([^"]+)"', package, re.MULTILINE)
    return match.group(1) if match else "unknown"


def _sha256(path: Path) -> str | None:
    if not path.exists():
        return None
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build_manifest(
    dataset: str,
    server_info: Json,
    params: Json,
    features: str,
    profile: str,
    tarn_bin: str,
) -> Json:
    """Assemble the manifest. Called once per run, before any scoring."""
    checksums_path = eval_dir(dataset) / "checksums.json"
    checksums: Json = (
        json.loads(checksums_path.read_text()) if checksums_path.exists() else {}
    )

    return {
        "run_started_at": datetime.now(UTC).isoformat(),
        "tarn": {
            "name": "tarn",
            "version": cargo_version(),
            # What the server advertises over MCP. Not tarn's identity -- see the
            # module docstring. Recorded so the discrepancy stays visible.
            "server_info": server_info,
            "commit": git_commit(),
            "dirty": git_dirty(),
            "features": features,
            "profile": profile,
            "binary": tarn_bin,
        },
        "dataset": checksums,
        "harness": {
            "python_version": platform.python_version(),
            "uv_lock_sha256": _sha256(ROOT / "uv.lock"),
            "argv": sys.argv,
        },
        "params": params,
        "machine": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "processor": platform.processor(),
            "cpu_count": os.cpu_count(),
        },
        "timing": {},
    }


def dataset_fingerprint(manifest: Json) -> str:
    """Short, stable id for the corpus a run scored against.

    Two runs are comparable only if this matches. report.py flags any history
    table that spans more than one, rather than letting a silently re-cut corpus
    read as a regression in tarn.
    """
    shas = (manifest.get("dataset") or {}).get("sha256") or {}
    if not shas:
        return "unknown"
    joined = "|".join(f"{k}={v}" for k, v in sorted(shas.items()))
    return hashlib.sha256(joined.encode()).hexdigest()[:12]
