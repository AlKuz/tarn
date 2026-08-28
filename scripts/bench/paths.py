"""
Shared layout for the bench: where datasets live, where results go.

Inputs and outputs are deliberately separate. `data/` holds datasets --
downloaded corpora and the vaults generated from them. `target/benchmarks/`
holds everything a run produces, so results sit alongside the release binary
that produced them and are covered by the existing `/target` gitignore.

Version is the top-level partition of the output tree:

    target/benchmarks/
      report.md                        index across versions
      <version>/
        report.md                      the full picture for one version
        runs/<run-id>/                 that version's runs
        state/<features>/<dataset>/    that version's index state

A score is only meaningful next to the build that produced it, so everything a
version produced sits together and its report sits at the top of it. The version
report is one file covering every configuration and dataset -- those are the axes
it compares, and splitting them apart would destroy the comparison.

Index state is keyed by version *and* features because tarn records neither in
what it persists: IndexMeta carries note_count and last_indexed, nothing about
the build or the tokenizer. Point two builds at one --index-path and the second
reloads the first's index, finds no file changes, and skips reindexing. With
features that is silently wrong rather than merely stale -- a stemmed BM25 index
answering naively-tokenized queries. Separate directories make it unrepresentable.

One run never overwrites another: each gets its own directory under `runs/`.
"""

from datetime import UTC, datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Inputs
DATA_DIR = ROOT / "data"
RAW_DIR = DATA_DIR / "raw"  # downloaded BEIR archives
VAULT_DIR = DATA_DIR / "vault"  # generated tarn vaults
EVAL_DIR = DATA_DIR / "eval"  # queries, qrels, id_map, checksums

# Outputs
BENCH_DIR = ROOT / "target" / "benchmarks"
INDEX_PATH = BENCH_DIR / "report.md"  # entry point: latest scores + links


def vault_dir(dataset: str) -> Path:
    return VAULT_DIR / dataset


def eval_dir(dataset: str) -> Path:
    return EVAL_DIR / dataset


def raw_dir(dataset: str) -> Path:
    return RAW_DIR / dataset


def _safe(component: str) -> str:
    """Fold a version or feature string into one path component.

    Both are usually already safe, but a pre-release suffix or a comma-separated
    feature list would otherwise create directories nobody asked for.
    """
    return "".join(c if c.isalnum() or c in "._-" else "_" for c in component) or "none"


def version_dir(version: str) -> Path:
    return BENCH_DIR / _safe(version)


def version_report_path(version: str) -> Path:
    return version_dir(version) / "report.md"


def runs_dir(version: str) -> Path:
    return version_dir(version) / "runs"


def state_dir(version: str, features: str, dataset: str) -> Path:
    return version_dir(version) / "state" / _safe(features) / dataset


def new_run_dir(version: str, dataset: str, tarn_commit: str) -> Path:
    """Allocate a fresh run directory. Readable at a glance; the manifest holds detail."""
    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    path = runs_dir(version) / f"{stamp}-{dataset}-{tarn_commit}"
    path.mkdir(parents=True, exist_ok=True)
    return path


def all_run_dirs() -> list[Path]:
    """Every run across every version, oldest first.

    Run ids start with a sortable UTC timestamp, so sorting on the id rather than
    the full path keeps runs in chronological order regardless of which version
    folder they sit in.
    """
    if not BENCH_DIR.exists():
        return []
    runs = [
        run
        for version in BENCH_DIR.iterdir()
        if version.is_dir()
        for run in (version / "runs").glob("*")
        if run.is_dir()
    ]
    return sorted(runs, key=lambda p: p.name)


def latest_run_dir(dataset: str) -> Path:
    """Most recent run for a dataset, across versions, for the stages after search."""
    candidates = [
        p for p in all_run_dirs() if p.name.split("-", 1)[-1].startswith(dataset + "-")
    ]
    if not candidates:
        raise SystemExit(
            f"no run directory for {dataset!r} under {BENCH_DIR} -- "
            f"run scripts/bench/run_search.py {dataset} first"
        )
    return candidates[-1]
