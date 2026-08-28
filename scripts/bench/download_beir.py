"""
Download and unzip a BEIR-format dataset into data/raw/<name>/.

BEIR datasets are hosted as zips at a fixed URL pattern and all share the same
internal shape:
    corpus.jsonl        {"_id": ..., "title": ..., "text": ...}
    queries.jsonl       {"_id": ..., "text": ...}
    qrels/<split>.tsv   query-id  corpus-id  score   (tab-separated, header row)

That uniform shape is why one adapter (beir_to_tarn.py) covers every dataset in
the catalogue below, and why adding a new BEIR dataset later needs no new code --
just its name.

Usage:
    python scripts/bench/download_beir.py scifact
    python scripts/bench/download_beir.py nfcorpus arguana scidocs   # several at once
    python scripts/bench/download_beir.py --list                     # show the catalogue
"""

import argparse
import io
import json
import sys
import zipfile
from datetime import UTC, datetime

import requests

from paths import RAW_DIR

BASE_URL = (
    "https://public.ukp.informatik.tu-darmstadt.de/thakur/BEIR/datasets/{name}.zip"
)

# name -> (tier, approx corpus size, notes). Tiers are explained in README.md.
CATALOGUE = {
    "scifact": ("A", 5_183, "scientific claim verification"),
    "nfcorpus": ("A", 3_633, "nutrition / medical IR, graded relevance"),
    "arguana": ("A", 8_674, "counterargument retrieval, paraphrase-heavy"),
    "scidocs": ("A", 25_657, "citation prediction as retrieval"),
    "fiqa": ("B", 57_638, "financial opinion QA"),
    "trec-covid": ("B", 171_332, "biomedical, deep pooled judgments (gold quality)"),
    "quora": ("B", 522_931, "duplicate question retrieval, near-duplicate matching"),
    "nq": ("C", 2_681_468, "Wikipedia open-domain QA, large"),
    "hotpotqa": (
        "C",
        5_233_329,
        "multi-hop QA, large -- headroom marker for a future graph layer",
    ),
    "msmarco": (
        "C",
        8_841_823,
        "passage ranking, industry-standard scale, shallow qrels",
    ),
}


def tier_of(name: str) -> str:
    """Tier letter for a dataset, or '?' for one outside the catalogue."""
    entry = CATALOGUE.get(name)
    return entry[0] if entry else "?"


def download_one(name: str) -> None:
    if name not in CATALOGUE:
        print(
            f"warning: {name!r} is not in the curated catalogue below; "
            f"BEIR hosts more datasets than these, trying anyway.",
            file=sys.stderr,
        )
    dest = RAW_DIR / name
    if dest.exists():
        print(f"{name}: already downloaded at {dest}")
        return
    url = BASE_URL.format(name=name)
    print(f"{name}: downloading {url}")
    resp = requests.get(url, timeout=600)
    resp.raise_for_status()
    with zipfile.ZipFile(io.BytesIO(resp.content)) as zf:
        zf.extractall(RAW_DIR)
    # Provenance: BEIR zips carry no upstream version, so the fetch is stamped
    # here and the content is hashed at adapt time. Together they are what pins
    # a score to a corpus -- see scripts/README.md "Reproducibility".
    (dest / "download.json").write_text(
        json.dumps(
            {"name": name, "url": url, "downloaded_at": datetime.now(UTC).isoformat()},
            indent=2,
        )
        + "\n"
    )
    print(f"{name}: extracted to {dest}")


def main() -> None:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("names", nargs="*", help="dataset names, e.g. scifact nfcorpus")
    ap.add_argument("--list", action="store_true", help="print the catalogue and exit")
    args = ap.parse_args()

    if args.list or not args.names:
        for name, (tier, size, note) in CATALOGUE.items():
            print(f"  [{tier}] {name:<12} ~{size:>9,} docs  {note}")
        if not args.names:
            return

    for name in args.names:
        download_one(name)


if __name__ == "__main__":
    main()
