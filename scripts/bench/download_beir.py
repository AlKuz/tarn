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
    python scripts/bench/download_beir.py all                        # the whole catalogue
    python scripts/bench/download_beir.py --list                     # show the catalogue
    python scripts/bench/download_beir.py --resolve all              # names only, for scripting
"""

import argparse
import json
import sys
import tempfile
import zipfile
from datetime import UTC, datetime

import requests
from tqdm import tqdm

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


def resolve(selectors: list[str]) -> list[str]:
    """Expand selectors into dataset names, in order, without duplicates.

    A selector is a dataset name or `all`. Keeping this here rather than in the
    Makefile means the catalogue stays the single source of truth -- a name added
    to CATALOGUE is immediately selectable.

    An unknown name is passed through with a warning rather than rejected, for
    the same reason download_one does: BEIR hosts more datasets than these.
    """
    names: list[str] = []
    for selector in selectors:
        if selector.lower() == "all":
            names.extend(CATALOGUE)
            continue
        if selector not in CATALOGUE:
            print(
                f"warning: {selector!r} is not in the catalogue; "
                f"BEIR hosts more datasets than these, trying anyway.",
                file=sys.stderr,
            )
        names.append(selector)

    seen: set[str] = set()
    unique: list[str] = []
    for name in names:
        if name not in seen:
            seen.add(name)
            unique.append(name)
    return unique


def download_one(name: str) -> None:
    dest = RAW_DIR / name
    if dest.exists():
        print(f"{name}: already downloaded at {dest}")
        return
    url = BASE_URL.format(name=name)
    print(f"{name}: downloading {url}")

    # Streamed to a temp file rather than held in memory. The tier-A archives are
    # small enough that it would not matter, but msmarco is over a gigabyte
    # compressed and buffering it whole is a needless way to run out of RAM.
    # NamedTemporaryFile lands beside the destination, so it shares the same
    # filesystem and a partial download is cleaned up on failure.
    RAW_DIR.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=RAW_DIR, suffix=".zip") as tmp:
        with requests.get(url, timeout=600, stream=True) as resp:
            resp.raise_for_status()
            total = int(resp.headers.get("content-length", 0))
            with tqdm(
                total=total or None,
                unit="B",
                unit_scale=True,
                unit_divisor=1024,
                desc=f"{name} download",
                leave=False,
            ) as bar:
                for chunk in resp.iter_content(chunk_size=1 << 20):
                    tmp.write(chunk)
                    bar.update(len(chunk))
        tmp.flush()
        with zipfile.ZipFile(tmp.name) as zf:
            members = zf.infolist()
            for member in tqdm(members, desc=f"{name} extract", leave=False):
                zf.extract(member, RAW_DIR)
    # Provenance: BEIR zips carry no upstream version, so the fetch is stamped
    # here and the content is hashed at adapt time. Together they are what pins
    # a score to a corpus -- see scripts/bench/README.md "Reproducibility".
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
    ap.add_argument(
        "selectors",
        nargs="*",
        help="dataset names, or `all` for the whole catalogue",
    )
    ap.add_argument("--list", action="store_true", help="print the catalogue and exit")
    ap.add_argument(
        "--resolve",
        action="store_true",
        help="print the selected names, one per line, and exit (for scripting)",
    )
    args = ap.parse_args()

    if args.resolve:
        for name in resolve(args.selectors or ["all"]):
            print(name)
        return

    if args.list or not args.selectors:
        for name, (tier, size, note) in CATALOGUE.items():
            print(f"  [{tier}] {name:<12} ~{size:>9,} docs  {note}")
        if not args.selectors:
            return

    for name in resolve(args.selectors):
        download_one(name)


if __name__ == "__main__":
    main()
