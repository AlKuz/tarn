"""
Score a tarn run against BEIR-derived qrels using pytrec_eval (the standard TREC
evaluation tool, wrapped for Python): Recall@k, Precision@k, MRR, nDCG@k, MAP@k.

This is the primary scorer. It compares document *ids* against human relevance
judgments, so it is exact -- but for the same reason it cannot see whether tarn
returned the right *section* of the right note. That is what evaluate_ragas.py
is for.

Reads target/benchmarks/runs/<run-id>/run.jsonl, writes metrics.json alongside it.

Usage:
    python scripts/bench/evaluate.py scifact
    python scripts/bench/evaluate.py scifact --run-dir target/benchmarks/runs/<run-id>
"""

import argparse
import json
import statistics
from pathlib import Path
from typing import Any

import pytrec_eval
from tqdm import tqdm

from paths import eval_dir, latest_run_dir

Json = dict[str, Any]

CUTOFFS = [5, 10, 20, 100]


def load_qrels(path: Path) -> dict[str, dict[str, int]]:
    qrels: dict[str, dict[str, int]] = {}
    for line in path.open():
        qid, _iteration, docid, rel = line.rstrip("\n").split("\t")
        qrels.setdefault(qid, {})[docid] = int(rel)
    return qrels


def parse_hits(record: Json, filename_to_docid: dict[str, str]) -> list[str]:
    """Ordered, deduplicated BEIR doc ids for one query.

    A note's vault path is its filename here because the generated vault is flat,
    but the mapping goes through id_map rather than assuming filenames round-trip
    to doc ids. Deduplication matters because a note can be hit by more than one
    section once vaults stop being one-section-per-note.
    """
    out: list[str] = []
    seen: set[str] = set()
    for hit in record.get("hits", []):
        filename = hit["path"].split("#", 1)[0].split("/")[-1]
        doc_id = filename_to_docid.get(filename)
        if doc_id and doc_id not in seen:
            out.append(doc_id)
            seen.add(doc_id)
    return out


def main() -> None:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("dataset")
    ap.add_argument("--run-dir", default=None, help="defaults to the most recent run")
    args = ap.parse_args()

    evals = eval_dir(args.dataset)
    run_dir = Path(args.run_dir) if args.run_dir else latest_run_dir(args.dataset)

    qrels = load_qrels(evals / "qrels.tsv")
    id_map = json.loads((evals / "id_map.json").read_text())
    filename_to_docid = {v: k for k, v in id_map.items()}

    with (run_dir / "run.jsonl").open() as f:
        run_size = sum(1 for _ in f)

    run: dict[str, dict[str, float]] = {}
    latencies: list[float] = []
    # What each query costs an agent: how much it has to sift, and how much of
    # its context window the answer occupies. token_count is absent from runs
    # recorded before it was captured, which stays None rather than becoming 0 --
    # a missing measurement is not a measurement of zero.
    notes_returned: list[int] = []
    sections_returned: list[int] = []
    tokens_returned: list[int] = []
    empty = 0
    for line in tqdm(
        (run_dir / "run.jsonl").open(),
        total=run_size,
        desc=f"{args.dataset} scoring",
        unit="q",
    ):
        rec = json.loads(line)
        latencies.append(rec["latency_ms"])
        raw_hits = rec.get("hits", [])
        notes_returned.append(len(raw_hits))
        sections = [s for hit in raw_hits for s in hit.get("sections", [])]
        sections_returned.append(len(sections))
        counts = [s.get("token_count") for s in sections]
        if counts and all(c is not None for c in counts):
            tokens_returned.append(sum(c for c in counts if c is not None))
        hits = parse_hits(rec, filename_to_docid)
        if not hits:
            empty += 1
        # pytrec_eval wants a score per doc. Rank position stands in for tarn's
        # own score: RRF values are ties-heavy at the tail and only the ordering
        # is meaningful, and the ordering is tarn's own (the index returns
        # pre-sorted results). The raw scores stay in run.jsonl either way.
        run[rec["query_id"]] = {
            doc_id: float(len(hits) - rank) for rank, doc_id in enumerate(hits)
        }

    if not run:
        raise SystemExit(
            f"no results in {run_dir / 'run.jsonl'} -- run scripts/bench/run_search.py first"
        )

    measures = {"recip_rank"}
    for k in CUTOFFS:
        measures |= {f"recall_{k}", f"P_{k}", f"ndcg_cut_{k}", f"map_cut_{k}"}

    evaluator = pytrec_eval.RelevanceEvaluator(qrels, measures)
    per_query = evaluator.evaluate(run)

    if not per_query:
        raise SystemExit(
            "no overlap between run query ids and qrels query ids -- check query_id plumbing"
        )

    aggregate = {
        metric: sum(v[metric] for v in per_query.values()) / len(per_query)
        for metric in next(iter(per_query.values()))
    }

    latencies.sort()

    def percentile(values: list[float], p: float) -> float:
        if not values:
            return 0.0
        ordered = sorted(values)
        idx = min(int(p * len(ordered)), len(ordered) - 1)
        return round(ordered[idx], 3)

    def pct(p: float) -> float:
        return percentile(latencies, p)

    def mean(values: list[int]) -> float | None:
        return round(statistics.fmean(values), 1) if values else None

    mean_latency = statistics.fmean(latencies) if latencies else 0.0

    payload = {
        "dataset": args.dataset,
        "cutoffs": CUTOFFS,
        "queries_scored": len(per_query),
        "queries_unjudged": len(run) - len(per_query),
        "queries_with_no_hits": empty,
        "latency_ms": {
            "p50": pct(0.50),
            "p95": pct(0.95),
            "p99": pct(0.99),
            "mean": round(mean_latency, 3),
        },
        "retrieval_cost": {
            "queries_per_second": (
                round(1000 / mean_latency, 1) if mean_latency > 0 else None
            ),
            "notes_per_query": mean(notes_returned),
            "sections_per_query": mean(sections_returned),
            "tokens_per_query": mean(tokens_returned),
            "tokens_per_query_p50": percentile(
                [float(t) for t in tokens_returned], 0.50
            )
            if tokens_returned
            else None,
            "tokens_per_query_p95": percentile(
                [float(t) for t in tokens_returned], 0.95
            )
            if tokens_returned
            else None,
        },
        "aggregate": aggregate,
        "per_query": per_query,
    }
    out_path = run_dir / "metrics.json"
    out_path.write_text(json.dumps(payload, indent=2) + "\n")

    print(
        f"{args.dataset}  ({len(per_query)} queries scored, "
        f"{len(run) - len(per_query)} unjudged, {empty} returned nothing)"
    )
    print(f"  MRR          {aggregate['recip_rank']:.4f}")
    for k in CUTOFFS:
        print(
            f"  @{k:<3}  nDCG {aggregate[f'ndcg_cut_{k}']:.4f}   "
            f"Recall {aggregate[f'recall_{k}']:.4f}   "
            f"P {aggregate[f'P_{k}']:.4f}   "
            f"MAP {aggregate[f'map_cut_{k}']:.4f}"
        )
    cost = payload["retrieval_cost"]
    print(f"  latency      p50 {pct(0.50):.1f}ms   p95 {pct(0.95):.1f}ms")
    tokens = cost["tokens_per_query"]
    print(
        f"  cost/query   {cost['notes_per_query']} notes   "
        f"{cost['sections_per_query']} sections   "
        f"{f'{tokens:,.0f} tokens' if tokens else 'tokens unrecorded'}"
    )
    print(f"full per-query results: {out_path}")


if __name__ == "__main__":
    main()
