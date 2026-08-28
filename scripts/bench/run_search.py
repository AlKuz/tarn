"""
Spawn tarn-mcp over a dataset's vault, run every eval query through the search
tool, and write the raw results to a fresh run directory for the scoring stages.

Writes target/benchmarks/runs/<run-id>/{manifest.json, run.jsonl}.

There is no settle-time sleep here, and there should not be one. tarn attaches
the stdio transport only after start_sync's synchronous review pass completes
(ADR-0011), so a successful `initialize` response *is* the index-ready signal.
The handshake is timed instead of slept through -- that timing is the
cold-index measurement.

Usage:
    python scripts/bench/run_search.py scifact
    python scripts/bench/run_search.py scifact --tarn-bin target/release/tarn-mcp --top-k 100
"""

import argparse
import json
import re
import shutil
import sys
import time
from pathlib import Path
from typing import Any

from tqdm import tqdm

from mcp_client import McpStdioClient
from paths import eval_dir, new_run_dir, state_dir, vault_dir
from provenance import build_manifest, cargo_version

Json = dict[str, Any]

SEARCH_TOOL = "tarn_search_notes"

# tag: and folder: are parsed as hard filters by SearchQuery::parse, not as
# literal text. A natural-language query containing either token would be
# silently reinterpreted -- and an invalid folder: value fails the whole
# tools/call as a JSON-RPC error rather than a soft tool error.
FILTER_TOKEN = re.compile(r"(?:^|\s)(tag|folder):\S")


def find_search_tool(tools: list[Json], override: str | None) -> str:
    names = [t["name"] for t in tools]
    wanted = override or SEARCH_TOOL
    if wanted in names:
        return wanted
    raise SystemExit(f"tool {wanted!r} not in tools/list: {names}")


def extract_hits(raw_result: Json) -> list[Json]:
    """Turn one tools/call result into an ordered list of hit notes.

    tarn returns search results via CallToolResult::structured, so the payload
    is a top-level JSON array of NoteResult under `structuredContent` -- each
    {"path": "<vault-relative>.md", "sections": [{"heading_path": [...],
    "score": ..., ...}]}. The same JSON is duplicated as text in content[0],
    which is what makes reaching for the text block tempting and wrong.

    A note's rank score is the max of its section scores; that is how tarn
    itself orders notes, so the order here is already tarn's and is preserved.
    Scores are RRF fusion values (two pipelines cap at ~0.0328), not a 0-1
    relevance scale.

    heading_path is kept per section so evaluate_ragas.py can dereference the
    exact section tarn matched, via tarn://note/{path}#{heading_path}, and
    token_count so evaluate.py can total what a query costs an agent's context.
    """
    if raw_result.get("isError"):
        text = (raw_result.get("content") or [{}])[0].get("text", "")
        raise RuntimeError(f"search tool reported a failure: {text}")

    results = raw_result.get("structuredContent")
    if results is None:
        raise RuntimeError(
            "search result has no structuredContent -- did the tool switch to "
            "rendered/text output?"
        )
    if not isinstance(results, list):
        raise TypeError(f"expected a JSON array of NoteResult, got {type(results)}")

    hits: list[Json] = []
    for note in results:
        sections = [
            {
                "heading_path": s.get("heading_path", []),
                "score": s.get("score"),
                "token_count": s.get("token_count"),
            }
            for s in note.get("sections", [])
        ]
        scores = [s["score"] for s in sections if s["score"] is not None]
        hits.append(
            {
                "path": note["path"],
                "score": max(scores) if scores else None,
                "sections": sections,
            }
        )
    return hits


def index_metrics(
    state: Path,
    info: Json,
    expected_notes: int,
    index_ready_seconds: float,
    cold_start: bool,
) -> Json:
    """What the index cost to build and what it costs to hold.

    Sizes are read by globbing the state directory rather than naming bm25.json
    and its siblings, so this does not encode tarn's persistence layout.

    notes_per_second is recorded only for a cold start. On a warm one the
    handshake measures deserialising four JSON files, and dividing note count by
    that would report an indexing rate the indexer never achieved.
    """
    files = {f.name: f.stat().st_size for f in sorted(state.glob("*")) if f.is_file()}
    total = sum(files.values())
    notes = info.get("note_count") or 0
    return {
        "notes_indexed": notes,
        "notes_expected": expected_notes,
        "tag_count": info.get("tag_count"),
        "state_bytes": total,
        "state_files": files,
        "state_bytes_per_note": round(total / notes, 1) if notes else None,
        "notes_per_second": (
            round(notes / index_ready_seconds, 1)
            if cold_start and index_ready_seconds > 0
            else None
        ),
    }


def main() -> None:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("dataset")
    ap.add_argument("--tarn-bin", default="target/release/tarn-mcp")
    ap.add_argument("--tool-name", default=None, help=f"override {SEARCH_TOOL!r}")
    ap.add_argument(
        "--top-k",
        type=int,
        default=100,
        help=(
            "sections requested per query. tarn's own default is 20, which would "
            "silently cap scoring below the k=100 cutoff. This caps sections "
            "before they are grouped into notes, so notes returned <= top-k."
        ),
    )
    ap.add_argument(
        "--token-limit",
        type=int,
        default=None,
        help=(
            "prefix cut over cumulative section token_count, applied after "
            "--top-k. Off by default; set it to measure what a context budget "
            "costs in recall."
        ),
    )
    ap.add_argument(
        "--score-threshold",
        type=float,
        default=0.0,
        help=(
            "minimum fused score. Compared against the reciprocal-rank score, "
            "where two ranking pipelines cap at ~0.0328 -- despite the tool "
            "describing it as a 0.0-1.0 scale. Any 'sensible' value returns "
            "nothing, so the default is 0.0."
        ),
    )
    ap.add_argument(
        "--features",
        default="stemming",
        help="cargo features the binary was built with",
    )
    ap.add_argument("--profile", default="release")
    ap.add_argument(
        "--cold",
        action="store_true",
        help=(
            "wipe persisted state first, so index_ready_seconds measures a real "
            "cold build. Off by default: rebuilding costs minutes on tier A and "
            "the ranking metrics are identical either way."
        ),
    )
    args = ap.parse_args()

    vault = vault_dir(args.dataset)
    evals = eval_dir(args.dataset)
    if not vault.exists():
        raise SystemExit(
            f"{vault} does not exist -- run scripts/bench/beir_to_tarn.py {args.dataset} first"
        )

    queries = [json.loads(line) for line in (evals / "queries.jsonl").open()]
    id_map = json.loads((evals / "id_map.json").read_text())

    suspicious = [q["query_id"] for q in queries if FILTER_TOKEN.search(q["text"])]
    if suspicious:
        print(
            f"warning: {len(suspicious)} queries contain a 'tag:' or 'folder:' token "
            f"and will be parsed as filters, not text: {suspicious[:5]}",
            file=sys.stderr,
        )

    version = cargo_version()
    state = state_dir(version, args.features, args.dataset)
    if args.cold and state.exists():
        shutil.rmtree(state)
    state.mkdir(parents=True, exist_ok=True)
    # Whether this run paid for the index or inherited it. Without the flag, an
    # index_ready_seconds of 1.2s reads as a fast index build when it is really a
    # warm start reloading four JSON files.
    cold_start = not any(state.iterdir())

    # These four are the configuration: report.py groups runs by them, so two runs
    # of the same version with different values become a comparison rather than
    # two unrelated numbers. `rendered` is fixed: rendered=true returns Markdown
    # text instead of structured results, which carries no note paths to score.
    params = {
        "limit": args.top_k,
        "token_limit": args.token_limit,
        "score_threshold": args.score_threshold,
        "rendered": False,
    }

    client = McpStdioClient(
        [
            args.tarn_bin,
            "--vault",
            str(vault.resolve()),
            "--index-path",
            str(state.resolve()),
            "--log-level",
            "warn",
        ]
    )
    try:
        kind = "cold" if cold_start else "warm"
        print(
            f"initializing ({kind} start; the handshake does not return until the "
            f"index reflects all {len(id_map):,} notes)...",
            file=sys.stderr,
        )
        t0 = time.perf_counter()
        init = client.initialize()
        index_ready_seconds = time.perf_counter() - t0
        print(f"index ready in {index_ready_seconds:.1f}s ({kind})", file=sys.stderr)

        server_info = init.get("serverInfo", {})
        manifest = build_manifest(
            dataset=args.dataset,
            server_info=server_info,
            params=params,
            features=args.features,
            profile=args.profile,
            tarn_bin=args.tarn_bin,
        )
        manifest["timing"]["index_ready_seconds"] = round(index_ready_seconds, 3)
        manifest["timing"]["cold_start"] = cold_start

        run_dir = new_run_dir(version, args.dataset, manifest["tarn"]["commit"])

        tool = find_search_tool(client.list_tools(), args.tool_name)

        # tarn://vault/info reports counts from the *index*, not from storage, so
        # this is a genuine readiness assertion rather than a directory listing.
        info = client.read_resource_json("tarn://vault/info")
        indexed, expected = info.get("note_count"), len(id_map)
        manifest["index"] = index_metrics(
            state, info, expected, index_ready_seconds, cold_start
        )
        if indexed != expected:
            raise SystemExit(
                f"index holds {indexed} of {expected} notes. Stale or partial state "
                f"in {state} -- delete it and re-run. (Scoring against a partial "
                f"index reports near-zero for everything and looks like a "
                f"regression in tarn.)"
            )

        run_path = run_dir / "run.jsonl"
        underfilled = 0
        with run_path.open("w") as out:
            for q in tqdm(queries, desc=f"{args.dataset} queries", unit="q"):
                t = time.perf_counter()
                arguments: Json = {
                    "query": q["text"],
                    "limit": args.top_k,
                    "score_threshold": args.score_threshold,
                }
                if args.token_limit is not None:
                    arguments["token_limit"] = args.token_limit
                result = client.call_tool(tool, arguments)
                latency_ms = (time.perf_counter() - t) * 1000
                hits = extract_hits(result)
                if len(hits) < args.top_k:
                    underfilled += 1
                out.write(
                    json.dumps(
                        {
                            "query_id": q["query_id"],
                            "hits": hits,
                            "latency_ms": round(latency_ms, 3),
                        }
                    )
                    + "\n"
                )

        manifest["timing"]["search_seconds"] = round(
            time.perf_counter() - t0 - index_ready_seconds, 3
        )
        manifest["queries"] = {
            "total": len(queries),
            "underfilled_top_k": underfilled,
            "filter_token_suspects": suspicious,
        }
        (run_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")

        print(f"wrote {run_path}", file=sys.stderr)
        print(run_dir)
    finally:
        client.close()


if __name__ == "__main__":
    main()
