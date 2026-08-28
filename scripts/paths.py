"""
Shared layout for the bench: where datasets live, where results go.

Inputs and outputs are deliberately separate. `data/` holds datasets --
downloaded corpora and the vaults generated from them. `target/benchmarks/`
holds everything a run produces, so results sit alongside the release binary
that produced them and are covered by the existing `/target` gitignore.

One run never overwrites another: each gets its own directory under
`runs/<run-id>/`, and `report.py` reads all of them.
"""

from datetime import UTC, datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# Inputs
DATA_DIR = ROOT / "data"
RAW_DIR = DATA_DIR / "raw"  # downloaded BEIR archives
VAULT_DIR = DATA_DIR / "vault"  # generated tarn vaults
EVAL_DIR = DATA_DIR / "eval"  # queries, qrels, id_map, checksums

# Outputs
BENCH_DIR = ROOT / "target" / "benchmarks"
STATE_DIR = BENCH_DIR / "state"  # tarn's --index-path
RUNS_DIR = BENCH_DIR / "runs"
REPORT_PATH = BENCH_DIR / "report.md"


def vault_dir(dataset: str) -> Path:
    return VAULT_DIR / dataset


def eval_dir(dataset: str) -> Path:
    return EVAL_DIR / dataset


def raw_dir(dataset: str) -> Path:
    return RAW_DIR / dataset


def state_dir(dataset: str) -> Path:
    return STATE_DIR / dataset


def new_run_dir(dataset: str, tarn_commit: str) -> Path:
    """Allocate a fresh run directory. Readable at a glance; the manifest holds detail."""
    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    path = RUNS_DIR / f"{stamp}-{dataset}-{tarn_commit}"
    path.mkdir(parents=True, exist_ok=True)
    return path


def latest_run_dir(dataset: str) -> Path:
    """Most recent run directory for a dataset, for the stages that follow search."""
    candidates = sorted(p for p in RUNS_DIR.glob(f"*-{dataset}-*") if p.is_dir())
    if not candidates:
        raise SystemExit(
            f"no run directory for {dataset!r} under {RUNS_DIR} -- "
            f"run scripts/run_search.py {dataset} first"
        )
    return candidates[-1]


def all_run_dirs() -> list[Path]:
    """Every run, oldest first. Run ids start with a sortable UTC timestamp."""
    if not RUNS_DIR.exists():
        return []
    return sorted(p for p in RUNS_DIR.iterdir() if p.is_dir())
