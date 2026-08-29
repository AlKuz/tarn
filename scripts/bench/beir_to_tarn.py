"""
Convert a downloaded BEIR-shaped dataset into:

  - a tarn vault:      data/vault/<name>/*.md  -- one markdown note per corpus
    document, ready to point `tarn-mcp --vault` at
  - an eval manifest:  data/eval/<name>/{queries.jsonl, qrels.tsv, id_map.json,
    checksums.json}

id_map.json is the source of truth mapping a BEIR doc id to the markdown
filename tarn indexes it under -- evaluate.py maps tarn's hits back through this
file rather than assuming filenames round-trip to doc ids.

checksums.json is the dataset's version. BEIR zips carry no upstream version
number, so the sha256 of each source file is what pins a score to a corpus: if
upstream re-cuts a dataset, the hash changes and report.py refuses to compare
across it.

Note on granularity: BEIR corpus items are short, single-paragraph passages, so
each becomes one note with exactly one section -- chunk == document. This
sidesteps the section-vs-document aggregation problem a long-document dataset
would need extra handling for (see README.md "Chunking granularity").

A dataset that has already been adapted is skipped, the way download_beir.py
skips one already downloaded. Delete data/vault/<name> or data/eval/<name> to
rebuild it.

Usage:
    python scripts/bench/beir_to_tarn.py scifact
    python scripts/bench/beir_to_tarn.py scifact --split test
"""

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

from tqdm import tqdm

from paths import eval_dir, raw_dir, vault_dir


def safe_filename(doc_id: str) -> str:
    """Most BEIR doc ids are already filename-safe; hash-suffix the rest so
    collisions after sanitization are impossible rather than merely unlikely."""
    slug = re.sub(r"[^A-Za-z0-9_.-]", "_", doc_id)[:120]
    digest = hashlib.sha1(doc_id.encode()).hexdigest()[:8]
    return f"{slug}-{digest}"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def already_adapted(vault: Path, evals: Path) -> bool:
    """True when a previous run produced a complete vault and eval manifest.

    checksums.json is written last, so its presence is this stage's own "the run
    finished" stamp -- an interrupted run leaves none and is redone. The two
    outputs live in separate trees, so both are checked: gating on one alone
    would let deleting the other skip into producing nothing.
    """
    manifest = ["checksums.json", "queries.jsonl", "qrels.tsv", "id_map.json"]
    if not all((evals / name).exists() for name in manifest):
        return False
    return any(vault.glob("*.md"))


def main() -> None:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("dataset")
    ap.add_argument("--split", default="test")
    args = ap.parse_args()

    raw = raw_dir(args.dataset)
    vault = vault_dir(args.dataset)
    evals = eval_dir(args.dataset)
    if not raw.exists():
        raise SystemExit(
            f"{raw} does not exist -- run scripts/bench/download_beir.py {args.dataset} first"
        )
    if already_adapted(vault, evals):
        prior = json.loads((evals / "checksums.json").read_text())
        print(
            f"{args.dataset}: already adapted "
            f"({prior['doc_count']} notes, split={prior['split']}) "
            f"-- delete {vault} to rebuild"
        )
        return
    vault.mkdir(parents=True, exist_ok=True)
    evals.mkdir(parents=True, exist_ok=True)

    # A rebuild only happens once something was deleted or changed, which is
    # exactly when leftovers matter: writing into the old vault would leave notes
    # from a previous corpus cut or split for tarn to index and score.
    for stale in vault.glob("*.md"):
        stale.unlink()

    # Line count first so the bar has a total. Cheap next to writing the notes,
    # and a bar without a total tells you nothing about how long is left.
    with (raw / "corpus.jsonl").open() as f:
        corpus_size = sum(1 for _ in f)

    id_map: dict[str, str] = {}  # beir doc_id -> markdown filename
    with (raw / "corpus.jsonl").open() as f:
        for line in tqdm(
            f, total=corpus_size, desc=f"{args.dataset} notes", unit="note"
        ):
            doc = json.loads(line)
            doc_id = doc["_id"]
            filename = safe_filename(doc_id) + ".md"
            id_map[doc_id] = filename
            title = doc.get("title", "").strip()
            body = doc.get("text", "").strip()
            # native_id in frontmatter is unused by tarn today, but it is cheap
            # provenance and survives a round-trip through the vault.
            frontmatter = (
                f'---\nsource_dataset: {args.dataset}\nnative_id: "{doc_id}"\n---\n\n'
            )
            # The H1 is load-bearing, not cosmetic: tarn indexes sections, which
            # are delimited by headings (ADR-0005). A document with an empty
            # title would otherwise produce a note with no heading. Falling back
            # to the doc id guarantees every note has exactly one section, so
            # chunk == document holds by construction.
            heading = f"# {title or doc_id}\n\n"
            (vault / filename).write_text(frontmatter + heading + body + "\n")

    # Qrels first: they decide which queries are worth issuing. BEIR ships every
    # split's queries in one queries.jsonl (scifact: 1,109 of them, 300 judged in
    # test), and an unjudged query is pure cost -- it cannot be scored, and it
    # would show up as a large "unjudged" count that reads like a plumbing bug.
    qrels_src = raw / "qrels" / f"{args.split}.tsv"
    qrel_count = 0
    judged: set[str] = set()
    with qrels_src.open() as f, (evals / "qrels.tsv").open("w") as out:
        next(f)  # header: query-id  corpus-id  score
        for line in f:
            qid, docid, score = line.rstrip("\n").split("\t")
            # TREC qrel format is: query-id iteration doc-id relevance
            out.write(f"{qid}\t0\t{docid}\t{score}\n")
            judged.add(qid)
            qrel_count += 1

    query_count = 0
    with (
        (evals / "queries.jsonl").open("w") as out,
        (raw / "queries.jsonl").open() as f,
    ):
        for line in f:
            q = json.loads(line)
            if q["_id"] not in judged:
                continue
            out.write(json.dumps({"query_id": q["_id"], "text": q["text"]}) + "\n")
            query_count += 1

    if query_count != len(judged):
        print(
            f"warning: {len(judged) - query_count} query ids appear in "
            f"qrels/{args.split}.tsv but not in queries.jsonl",
            file=sys.stderr,
        )

    (evals / "id_map.json").write_text(json.dumps(id_map, indent=2))

    download = raw / "download.json"
    checksums = {
        "dataset": args.dataset,
        "split": args.split,
        "doc_count": len(id_map),
        "query_count": query_count,
        "qrel_count": qrel_count,
        "sha256": {
            "corpus.jsonl": sha256_file(raw / "corpus.jsonl"),
            "queries.jsonl": sha256_file(raw / "queries.jsonl"),
            f"qrels/{args.split}.tsv": sha256_file(qrels_src),
        },
        "download": json.loads(download.read_text()) if download.exists() else None,
    }
    (evals / "checksums.json").write_text(json.dumps(checksums, indent=2) + "\n")

    print(f"{len(id_map)} notes written to {vault}")
    print(
        f"{query_count} queries, {qrel_count} qrels ({args.split}) written to {evals}"
    )


if __name__ == "__main__":
    main()
