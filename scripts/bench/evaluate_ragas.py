"""
Secondary pass: score the *text* tarn returned against the text of the
known-relevant documents, using ragas's non-LLM context metrics
(NonLLMContextPrecisionWithReference, NonLLMContextRecall). These compare
strings with rapidfuzz distance at a 0.5 similarity threshold -- no API key, no
model call, no network.

Why this exists alongside evaluate.py. tarn indexes sections, not notes
(ADR-0005), so a hit is a section that gets mapped back to its parent note's
doc id for qrel scoring. If tarn returns the right note but the wrong or a
truncated section of it, evaluate.py records a perfect hit, because the doc id
matches. That failure is invisible to doc-id scoring by construction. This
stage sees it.

The design expectation was that this would be near-redundant on BEIR:
beir_to_tarn.py emits one note with exactly one section per passage, so
section == document and there should be no chunking to get wrong. The first
scifact run disproved that. Roughly 4% of top-10 hits came back with a
`heading_path` split into fake levels -- `# The Sequence Alignment/Map format
and SAMtools` is one heading, but the index reports it as
["The Sequence Alignment", "Map format and SAMtools"], because the section path
is joined with "/" and nothing escapes a "/" inside a heading. Those sections
cannot be dereferenced at all. nDCG was unaffected, because the right note was
still returned and the doc id still matched.

That is the asymmetry this stage exists for, and it showed up on day one.
Its scores are still coarser than evaluate.py's -- a string-distance
approximation against an exact one -- so read them as a signal, not a verdict.

The retrieved side is fetched from the running server, not from disk: reading
local vault files for both sides would compare files to files and measure
nothing about tarn. The reference side does come from the vault, because it is
ground truth and must not depend on tarn.

A second spawn here is cheap. The index and revision tracker both persist to
--index-path, so this is a warm start: review_changes finds no changes and
serving begins almost immediately.

Usage:
    python scripts/bench/evaluate_ragas.py scifact
"""

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from ragas import evaluate
from ragas.dataset_schema import EvaluationDataset, EvaluationResult

# ragas 0.4 warns that these moved to ragas.metrics.collections, but that module
# exports only the LLM-judged variants -- the NonLLM* ones are still here.
from ragas.metrics import NonLLMContextPrecisionWithReference, NonLLMContextRecall
from tqdm import tqdm

from evaluate import parse_hits  # same hit extraction, so both scorers see one list
from mcp_client import McpStdioClient
from paths import eval_dir, latest_run_dir, state_dir, vault_dir

# Context metrics degrade into noise over long lists, and fetching every hit of
# every query would be tens of thousands of resource reads. The top slice is
# what a context window would actually receive.
DEFAULT_TOP_N = 10


def section_uri(path: str, heading_path: list[str]) -> str:
    """Address the exact section tarn matched. Falls back to the whole note when
    a hit carries no heading path."""
    if not heading_path:
        return f"tarn://note/{path}"
    return f"tarn://note/{path}#{'/'.join(heading_path)}"


def fetch_retrieved_text(
    client: McpStdioClient,
    hits: list[dict[str, Any]],
    top_n: int,
    cache: dict[str, str],
    unreachable: set[str],
) -> list[str]:
    """Text of the top-n hits, fetched from the server.

    A section that will not dereference is recorded rather than swallowed. It
    depresses precision here (the context is missing) while leaving evaluate.py's
    scores untouched (the doc id still matched), which is precisely the asymmetry
    this stage exists to expose -- so the count has to reach the report.
    """
    texts: list[str] = []
    for hit in hits[:top_n]:
        sections = hit.get("sections") or [{}]
        uri = section_uri(hit["path"], sections[0].get("heading_path", []))
        if uri not in cache:
            try:
                cache[uri] = str(client.read_resource_json(uri).get("content", ""))
            except Exception as e:  # noqa: BLE001 - a missing section is data, not a crash
                if len(unreachable) < 5:
                    print(f"warning: {uri}: {e}", file=sys.stderr)
                cache[uri] = ""
        if cache[uri]:
            texts.append(cache[uri])
        else:
            unreachable.add(uri)
    return texts


def main() -> None:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("dataset")
    ap.add_argument("--run-dir", default=None, help="defaults to the most recent run")
    ap.add_argument("--tarn-bin", default="target/release/tarn-mcp")
    ap.add_argument("--top-n", type=int, default=DEFAULT_TOP_N)
    args = ap.parse_args()

    evals = eval_dir(args.dataset)
    vault = vault_dir(args.dataset)
    run_dir = Path(args.run_dir) if args.run_dir else latest_run_dir(args.dataset)

    id_map: dict[str, str] = json.loads((evals / "id_map.json").read_text())
    filename_to_docid = {v: k for k, v in id_map.items()}
    queries = {
        q["query_id"]: q["text"]
        for q in (json.loads(line) for line in (evals / "queries.jsonl").open())
    }

    relevant: dict[str, list[str]] = {}
    for line in (evals / "qrels.tsv").open():
        qid, _iteration, docid, rel = line.rstrip("\n").split("\t")
        if int(rel) > 0:
            relevant.setdefault(qid, []).append(docid)

    def reference_text(doc_id: str) -> str | None:
        filename = id_map.get(doc_id)
        if not filename:
            return None
        path = vault / filename
        return path.read_text() if path.exists() else None

    client = McpStdioClient(
        [
            args.tarn_bin,
            "--vault",
            str(vault.resolve()),
            "--index-path",
            str(state_dir(args.dataset).resolve()),
            "--log-level",
            "warn",
        ]
    )
    rows: list[dict[str, Any]] = []
    cache: dict[str, str] = {}
    unreachable: set[str] = set()
    fetched = 0
    with (run_dir / "run.jsonl").open() as f:
        run_size = sum(1 for _ in f)

    try:
        client.initialize()
        for line in tqdm(
            (run_dir / "run.jsonl").open(),
            total=run_size,
            desc=f"{args.dataset} contexts",
            unit="q",
        ):
            rec = json.loads(line)
            qid = rec["query_id"]
            if qid not in relevant or qid not in queries:
                continue
            # parse_hits is called for its side effect of proving both scorers
            # read the same hit list; the doc ids themselves are evaluate.py's job.
            parse_hits(rec, filename_to_docid)
            retrieved = fetch_retrieved_text(
                client, rec.get("hits", []), args.top_n, cache, unreachable
            )
            fetched += min(len(rec.get("hits", [])), args.top_n)
            references = [t for t in map(reference_text, relevant[qid]) if t]
            if not retrieved or not references:
                continue
            rows.append(
                {
                    "user_input": queries[qid],
                    "retrieved_contexts": retrieved,
                    "reference_contexts": references,
                }
            )
    finally:
        client.close()

    if not rows:
        raise SystemExit(
            f"no scoreable rows from {run_dir / 'run.jsonl'} -- "
            f"run scripts/bench/run_search.py first"
        )

    result = evaluate(
        EvaluationDataset.from_list(rows),
        metrics=[NonLLMContextPrecisionWithReference(), NonLLMContextRecall()],
    )
    if not isinstance(result, EvaluationResult):
        raise TypeError(f"expected an EvaluationResult, got {type(result).__name__}")

    scores = result.to_pandas().mean(numeric_only=True).to_dict()
    aggregate = {str(k): float(v) for k, v in scores.items()}
    payload = {
        "dataset": args.dataset,
        "rows_scored": len(rows),
        "top_n": args.top_n,
        "sections_requested": fetched,
        "sections_unreachable": len(unreachable),
        "unreachable_examples": sorted(unreachable)[:5],
        "aggregate": aggregate,
    }
    out_path = run_dir / "ragas_metrics.json"
    out_path.write_text(json.dumps(payload, indent=2) + "\n")

    print(f"{args.dataset}  ({len(rows)} rows, top-{args.top_n} contexts)")
    if unreachable:
        print(
            f"  {len(unreachable)} distinct sections would not dereference; "
            f"the scores below are depressed by exactly that much"
        )
    for name, value in aggregate.items():
        print(f"  {name:<45} {value:.4f}")
    print(f"full results: {out_path}")


if __name__ == "__main__":
    main()
