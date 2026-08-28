"""
Consolidate every benchmark run into one Markdown file.

Reads target/benchmarks/runs/*/ and writes target/benchmarks/report.md.

Results are not committed, so this file carries its own history: each dataset
gets a table of its runs, most recent first, with a delta against the previous
one. Performance movement is visible without needing git.

Two runs are only comparable if they scored the same corpus. BEIR ships no
upstream version number, so the sha256 of the source files is the version --
report.py marks any row whose dataset fingerprint differs from the latest,
rather than letting a silently re-cut corpus read as a regression in tarn.

Usage:
    python scripts/bench/report.py
"""

import argparse
import json
from typing import Any

from download_beir import CATALOGUE, tier_of
from paths import REPORT_PATH, all_run_dirs
from provenance import dataset_fingerprint

Json = dict[str, Any]

HISTORY_LIMIT = 10


def load_runs() -> list[Json]:
    """Every run that got as far as producing metrics, oldest first."""
    runs: list[Json] = []
    for run_dir in all_run_dirs():
        metrics_path = run_dir / "metrics.json"
        manifest_path = run_dir / "manifest.json"
        if not metrics_path.exists() or not manifest_path.exists():
            continue
        ragas_path = run_dir / "ragas_metrics.json"
        runs.append(
            {
                "id": run_dir.name,
                "manifest": json.loads(manifest_path.read_text()),
                "metrics": json.loads(metrics_path.read_text()),
                "ragas": json.loads(ragas_path.read_text())
                if ragas_path.exists()
                else None,
            }
        )
    return runs


def duration(seconds: float | None) -> str:
    if seconds is None:
        return "—"
    if seconds < 60:
        return f"{seconds:.1f}s"
    return f"{int(seconds // 60)}m{int(seconds % 60):02d}s"


def index_time(manifest: Json) -> str:
    """Time from spawn to a served handshake, tagged cold or warm.

    A warm start reloads persisted JSON; a cold start builds the whole index.
    They differ by orders of magnitude, so an untagged number is misleading."""
    timing = manifest.get("timing", {})
    rendered = duration(timing.get("index_ready_seconds"))
    if rendered == "—":
        return rendered
    return f"{rendered} {'cold' if timing.get('cold_start') else 'warm'}"


def tarn_label(manifest: Json) -> str:
    tarn = manifest.get("tarn", {})
    dirty = "-dirty" if tarn.get("dirty") else ""
    return f"{tarn.get('version', '?')} {tarn.get('commit', '?')}{dirty}"


def metric(run: Json, name: str) -> float | None:
    value = run["metrics"].get("aggregate", {}).get(name)
    return None if value is None else float(value)


def fmt(value: float | None, places: int = 4) -> str:
    return "—" if value is None else f"{value:.{places}f}"


def delta(current: float | None, previous: float | None) -> str:
    if current is None or previous is None:
        return "—"
    diff = current - previous
    if abs(diff) < 5e-5:
        return "±0"
    return f"{diff:+.4f}"


def summary_table(latest: dict[str, Json]) -> list[str]:
    lines = [
        (
            "| Dataset | Tier | Corpus | Queries | nDCG@10 | Recall@100 | MRR | MAP@10 "
            "| Index | p50 | p95 | tarn |"
        ),
        "|---|---|---|---|---|---|---|---|---|---|---|---|",
    ]
    for name in sorted(latest):
        run = latest[name]
        manifest, metrics = run["manifest"], run["metrics"]
        dataset = manifest.get("dataset", {})
        latency = metrics.get("latency_ms", {})
        lines.append(
            f"| {name} "
            f"| {tier_of(name)} "
            f"| {dataset.get('doc_count', 0):,} "
            f"| {metrics.get('queries_scored', 0):,} "
            f"| {fmt(metric(run, 'ndcg_cut_10'))} "
            f"| {fmt(metric(run, 'recall_100'))} "
            f"| {fmt(metric(run, 'recip_rank'))} "
            f"| {fmt(metric(run, 'map_cut_10'))} "
            f"| {index_time(manifest)} "
            f"| {latency.get('p50', 0):.0f}ms "
            f"| {latency.get('p95', 0):.0f}ms "
            f"| {tarn_label(manifest)} |"
        )
    return lines


def history_table(runs: list[Json]) -> list[str]:
    """Newest first, with a delta against the run before it."""
    newest_fingerprint = dataset_fingerprint(runs[-1]["manifest"])
    lines = [
        "| Run | tarn | nDCG@10 | Δ | Recall@100 | MRR | Corpus |",
        "|---|---|---|---|---|---|---|",
    ]
    ordered = list(reversed(runs))[:HISTORY_LIMIT]
    for i, run in enumerate(ordered):
        previous = ordered[i + 1] if i + 1 < len(ordered) else None
        fingerprint = dataset_fingerprint(run["manifest"])
        mark = "" if fingerprint == newest_fingerprint else " ⚠"
        lines.append(
            f"| `{run['id']}` "
            f"| {tarn_label(run['manifest'])} "
            f"| {fmt(metric(run, 'ndcg_cut_10'))} "
            f"| {delta(metric(run, 'ndcg_cut_10'), metric(previous, 'ndcg_cut_10') if previous else None)} "
            f"| {fmt(metric(run, 'recall_100'))} "
            f"| {fmt(metric(run, 'recip_rank'))} "
            f"| `{fingerprint}`{mark} |"
        )
    if any(dataset_fingerprint(r["manifest"]) != newest_fingerprint for r in ordered):
        lines += [
            "",
            (
                "> ⚠ marks a run scored against a different corpus fingerprint. Those "
                "rows are not comparable with the latest — the dataset changed, not "
                "necessarily tarn."
            ),
        ]
    return lines


def detail_section(run: Json) -> list[str]:
    metrics = run["metrics"]
    aggregate = metrics.get("aggregate", {})
    lines = [
        "",
        "**Latest run, all cutoffs**",
        "",
        "| k | nDCG | Recall | Precision | MAP |",
        "|---|---|---|---|---|",
    ]
    for k in metrics.get("cutoffs", []):
        lines.append(
            f"| {k} "
            f"| {fmt(aggregate.get(f'ndcg_cut_{k}'))} "
            f"| {fmt(aggregate.get(f'recall_{k}'))} "
            f"| {fmt(aggregate.get(f'P_{k}'))} "
            f"| {fmt(aggregate.get(f'map_cut_{k}'))} |"
        )

    latency = metrics.get("latency_ms", {})
    lines += [
        "",
        (
            f"MRR {fmt(aggregate.get('recip_rank'))} · "
            f"{metrics.get('queries_scored', 0)} scored, "
            f"{metrics.get('queries_unjudged', 0)} unjudged, "
            f"{metrics.get('queries_with_no_hits', 0)} returned nothing"
        ),
        "",
        (
            f"Latency: p50 {latency.get('p50', 0):.1f}ms · "
            f"p95 {latency.get('p95', 0):.1f}ms · "
            f"p99 {latency.get('p99', 0):.1f}ms · "
            f"mean {latency.get('mean', 0):.1f}ms · "
            f"index ready {index_time(run['manifest'])}"
        ),
    ]

    if run["ragas"]:
        aggregate_ragas = run["ragas"].get("aggregate", {})
        lines += [
            "",
            (
                "**Chunk-level cross-check** (ragas, non-LLM string distance at a 0.5 "
                "threshold). It scores the text tarn actually returned, so it sees "
                "section-level failures that doc-id scoring cannot. Coarser than the "
                "metrics above — read it as a signal, not a verdict."
            ),
            "",
            "| Metric | Score |",
            "|---|---|",
        ]
        for name, value in aggregate_ragas.items():
            lines.append(f"| {name} | {value:.4f} |")

        unreachable = run["ragas"].get("sections_unreachable") or 0
        if unreachable:
            requested = run["ragas"].get("sections_requested") or 0
            examples = run["ragas"].get("unreachable_examples") or []
            lines += [
                "",
                (
                    f"> **{unreachable} distinct sections would not dereference** out of "
                    f"{requested} requested. The scores above are depressed by exactly "
                    f"that much — those contexts are simply missing. The doc-id metrics "
                    f"are unaffected, because the right note was still returned."
                ),
                "",
                "```text",
                *examples,
                "```",
            ]

    lines += [
        "",
        "<details><summary>Run manifest</summary>",
        "",
        "```json",
        json.dumps(run["manifest"], indent=2),
        "```",
        "",
        "</details>",
    ]
    return lines


def main() -> None:
    argparse.ArgumentParser(description=__doc__).parse_args()

    runs = load_runs()
    if not runs:
        raise SystemExit(
            "no scored runs found -- run `make bench` (or scripts/bench/evaluate.py) first"
        )

    by_dataset: dict[str, list[Json]] = {}
    for run in runs:
        by_dataset.setdefault(run["metrics"]["dataset"], []).append(run)
    latest = {name: rs[-1] for name, rs in by_dataset.items()}

    generated_from = ", ".join(
        f"{n} ({len(rs)} runs)" for n, rs in sorted(by_dataset.items())
    )
    lines = [
        "# Tarn Retrieval Benchmarks",
        "",
        "> GENERATED by `make bench cmd=report` from `target/benchmarks/runs/`.",
        "> Do not edit by hand — re-run the bench instead.",
        f"> Covering {generated_from}.",
        "",
        (
            "Every number below comes from driving the real `tarn-mcp` binary over "
            "MCP/stdio against a BEIR corpus converted into a tarn vault. `nDCG@10` "
            "leads because it is what BEIR leaderboards headline, so it is externally "
            "comparable."
        ),
        "",
        "## Latest",
        "",
    ]
    lines += summary_table(latest)

    pending = [n for n in CATALOGUE if n not in latest]
    if pending:
        lines += [
            "",
            (
                f"**Not yet run** ({len(pending)}): {', '.join(pending)}. "
                "Run one with `make bench dataset=<name>`."
            ),
        ]

    for name in sorted(by_dataset):
        lines += ["", f"## {name}", ""]
        lines += history_table(by_dataset[name])
        lines += detail_section(latest[name])

    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    REPORT_PATH.write_text("\n".join(lines) + "\n")
    print(f"wrote {REPORT_PATH}")


if __name__ == "__main__":
    main()
