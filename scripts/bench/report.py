"""
Consolidate benchmark runs into Markdown.

Reads target/benchmarks/runs/*/ and writes:

    report.md                 the entry point -- latest score per dataset, and
                              a link to every version report
    <version>/report.md       one file per tarn version, holding that version's
                              runs in full

Results are not committed, so the reports carry their own history: each dataset
gets a table of its runs, most recent first, with a delta against the previous
one. Performance movement is visible without needing git.

Splitting by version is what keeps that bounded. Runs accumulate for as long as
the bench is used, and a single file holding all of them grows until nobody
opens it. A version report covers one version and then stops changing, which is
also the unit anyone actually compares: a score is only meaningful next to the
build that produced it.

Within a version, runs are grouped by *configuration* -- the cargo features and
the search parameters, the things that change what comes back. Holding the
version fixed and varying one of those is how a tuning question gets answered:
does stemming earn its keep, does a larger `limit` buy recall, what does a
token budget cost. The configuration tables are that comparison; the history
tables answer the different question of whether anything drifted over time.

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
from paths import INDEX_PATH, all_run_dirs, version_report_path
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


def tarn_version(manifest: Json) -> str:
    return str(manifest.get("tarn", {}).get("version") or "unknown")


def config_of(manifest: Json) -> Json:
    """The knobs that change what a search returns.

    Deliberately not the commit, the machine or the corpus: those are the axes
    the other tables vary. This is what you would set out to compare.
    """
    tarn = manifest.get("tarn", {})
    params = manifest.get("params", {})
    return {
        "features": tarn.get("features") or "—",
        "profile": tarn.get("profile") or "—",
        "limit": params.get("limit"),
        "token_limit": params.get("token_limit"),
        "score_threshold": params.get("score_threshold"),
        "rendered": params.get("rendered"),
    }


def config_key(manifest: Json) -> str:
    return json.dumps(config_of(manifest), sort_keys=True)


def config_short(manifest: Json) -> str:
    """Compact, self-describing label for a summary row.

    A summary shows the most recent run, which may well have been a deliberately
    degraded experiment. Naming the configuration inline stops that reading as
    tarn's headline number.
    """
    cfg = config_of(manifest)
    parts = [f"`{cfg['features']}`", f"k={cfg['limit']}"]
    if cfg["score_threshold"]:
        parts.append(f"thr={cfg['score_threshold']}")
    if cfg["token_limit"]:
        parts.append(f"tok={cfg['token_limit']}")
    if cfg["rendered"]:
        parts.append("rendered")
    return " ".join(parts)


def assign_config_ids(runs: list[Json]) -> dict[str, str]:
    """C1, C2, ... in first-seen order, so ids are stable within a report."""
    ids: dict[str, str] = {}
    for run in runs:
        key = config_key(run["manifest"])
        if key not in ids:
            ids[key] = f"C{len(ids) + 1}"
    return ids


def config_legend(runs: list[Json], ids: dict[str, str]) -> list[str]:
    seen: dict[str, Json] = {}
    for run in runs:
        seen.setdefault(config_key(run["manifest"]), config_of(run["manifest"]))
    lines = [
        "| Config | Features | Profile | limit | token_limit | score_threshold | rendered |",
        "|---|---|---|---|---|---|---|",
    ]
    for key, cfg in seen.items():
        lines.append(
            f"| **{ids[key]}** "
            f"| `{cfg['features']}` "
            f"| {cfg['profile']} "
            f"| {cfg['limit'] if cfg['limit'] is not None else '—'} "
            f"| {cfg['token_limit'] if cfg['token_limit'] is not None else 'none'} "
            f"| {cfg['score_threshold'] if cfg['score_threshold'] is not None else '—'} "
            f"| {cfg['rendered']} |"
        )
    return lines


def config_comparison(runs: list[Json], ids: dict[str, str]) -> list[str]:
    """One row per configuration for a single dataset, best nDCG@10 first.

    The latest run of each configuration supplies the numbers; earlier runs of
    the same configuration are the history table's business.
    """
    latest_per_config: dict[str, Json] = {}
    counts: dict[str, int] = {}
    for run in runs:
        key = config_key(run["manifest"])
        latest_per_config[key] = run
        counts[key] = counts.get(key, 0) + 1

    if len(latest_per_config) < 2:
        return []

    ordered = sorted(
        latest_per_config.items(),
        key=lambda kv: metric(kv[1], "ndcg_cut_10") or -1.0,
        reverse=True,
    )
    best = metric(ordered[0][1], "ndcg_cut_10")

    lines = [
        "### Configurations compared",
        "",
        "Same version, same corpus, different knobs. Best nDCG@10 first.",
        "",
        (
            "| Config | nDCG@10 | Δ | Recall@100 | MRR | MAP@10 "
            "| Notes/q | Tokens/q | p50 | p95 | Runs |"
        ),
        "|---|---|---|---|---|---|---|---|---|---|---|",
    ]
    for key, run in ordered:
        latency = run["metrics"].get("latency_ms", {})
        current = metric(run, "ndcg_cut_10")
        gap = "best" if run is ordered[0][1] else delta(current, best)
        lines.append(
            f"| **{ids[key]}** "
            f"| {fmt(current)} "
            f"| {gap} "
            f"| {fmt(metric(run, 'recall_100'))} "
            f"| {fmt(metric(run, 'recip_rank'))} "
            f"| {fmt(metric(run, 'map_cut_10'))} "
            f"| {count(cost(run, 'notes_per_query'), 1)} "
            f"| {count(cost(run, 'tokens_per_query'))} "
            f"| {latency.get('p50', 0):.0f}ms "
            f"| {latency.get('p95', 0):.0f}ms "
            f"| {counts[key]} |"
        )

    fingerprints = {
        dataset_fingerprint(r["manifest"]) for r in latest_per_config.values()
    }
    if len(fingerprints) > 1:
        lines += [
            "",
            (
                "> ⚠ These configurations did not all score the same corpus, so the "
                "comparison is not clean. Re-run them against one dataset fingerprint."
            ),
        ]
    return lines


def tarn_label(manifest: Json) -> str:
    tarn = manifest.get("tarn", {})
    dirty = "-dirty" if tarn.get("dirty") else ""
    return f"{tarn.get('version', '?')} {tarn.get('commit', '?')}{dirty}"


def metric(run: Json, name: str) -> float | None:
    value = run["metrics"].get("aggregate", {}).get(name)
    return None if value is None else float(value)


def fmt(value: float | None, places: int = 4) -> str:
    return "—" if value is None else f"{value:.{places}f}"


def count(value: float | None, places: int = 0) -> str:
    """A missing measurement renders as an em dash, never as zero.

    Runs recorded before token_count was captured have no token totals. Showing
    0 there would read as "this configuration returned nothing", which is the
    opposite of what happened."""
    return "—" if value is None else f"{value:,.{places}f}"


def cost(run: Json, name: str) -> float | None:
    value = run["metrics"].get("retrieval_cost", {}).get(name)
    return None if value is None else float(value)


def size(nbytes: float | None) -> str:
    if not nbytes:
        return "—"
    for unit in ("B", "KB", "MB", "GB"):
        if nbytes < 1024 or unit == "GB":
            return f"{nbytes:.0f}{unit}" if unit == "B" else f"{nbytes:.1f}{unit}"
        nbytes /= 1024
    return "—"


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
            "| Index | p50 | p95 | tarn | Config |"
        ),
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|",
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
            f"| {tarn_label(manifest)} "
            f"| {config_short(manifest)} |"
        )
    return lines


def history_table(runs: list[Json], ids: dict[str, str] | None = None) -> list[str]:
    """Newest first, with a delta against the run before it."""
    newest_fingerprint = dataset_fingerprint(runs[-1]["manifest"])
    config_column = "| Config " if ids else ""
    config_rule = "|---" if ids else ""
    lines = [
        f"| Run | tarn {config_column}| nDCG@10 | Δ | Recall@100 | MRR | Corpus |",
        f"|---|---{config_rule}|---|---|---|---|---|",
    ]
    ordered = list(reversed(runs))[:HISTORY_LIMIT]
    for i, run in enumerate(ordered):
        previous = ordered[i + 1] if i + 1 < len(ordered) else None
        fingerprint = dataset_fingerprint(run["manifest"])
        mark = "" if fingerprint == newest_fingerprint else " ⚠"
        config_cell = f"| {ids[config_key(run['manifest'])]} " if ids else ""
        lines.append(
            f"| `{run['id']}` "
            f"| {tarn_label(run['manifest'])} "
            f"{config_cell}"
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
        "### Latest run",
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

    index = run["manifest"].get("index", {})
    if index:
        rate = index.get("notes_per_second")
        built = (
            f"built at {count(rate, 1)} notes/s"
            if rate
            else "build rate unmeasured on a warm start"
        )
        lines += [
            "",
            "### Indexing",
            "",
            (
                f"{index.get('notes_indexed', 0):,} notes · "
                f"{index.get('tag_count', 0):,} tags · "
                f"state {size(index.get('state_bytes'))} "
                f"({count(index.get('state_bytes_per_note'))} bytes/note) · "
                f"{built}"
            ),
        ]
        files = index.get("state_files") or {}
        if files:
            lines += [
                "",
                "| State file | Size |",
                "|---|---|",
                *(f"| `{n}` | {size(b)} |" for n, b in sorted(files.items())),
            ]

    retrieval = metrics.get("retrieval_cost") or {}
    if retrieval:
        tokens = retrieval.get("tokens_per_query")
        lines += [
            "",
            "### Retrieval cost",
            "",
            (
                f"Per query: {count(retrieval.get('notes_per_query'), 1)} notes · "
                f"{count(retrieval.get('sections_per_query'), 1)} sections · "
                f"{count(tokens)} tokens "
                f"(p50 {count(retrieval.get('tokens_per_query_p50'))}, "
                f"p95 {count(retrieval.get('tokens_per_query_p95'))}) · "
                f"{count(retrieval.get('queries_per_second'), 1)} queries/s"
            ),
            "",
            (
                "Tokens per query is what a search costs an agent's context window — "
                "the number section-level indexing exists to keep down."
            ),
        ]

    if run["ragas"]:
        aggregate_ragas = run["ragas"].get("aggregate", {})
        lines += [
            "",
            "### Chunk-level cross-check",
            "",
            (
                "ragas non-LLM string distance at a 0.5 threshold. It scores the text "
                "tarn actually returned, so it sees section-level failures that doc-id "
                "scoring cannot. Coarser than the metrics above — read it as a signal, "
                "not a verdict."
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


def version_report(version: str, runs: list[Json]) -> str:
    """One version's runs in full: a summary row per dataset, then per-dataset
    history and detail."""
    by_dataset: dict[str, list[Json]] = {}
    for run in runs:
        by_dataset.setdefault(run["metrics"]["dataset"], []).append(run)
    latest = {name: rs[-1] for name, rs in by_dataset.items()}

    covered = ", ".join(f"{n} ({len(rs)} runs)" for n, rs in sorted(by_dataset.items()))
    lines = [
        f"# Tarn Retrieval Benchmarks — {version}",
        "",
        "> GENERATED by `make bench cmd=report` from `target/benchmarks/runs/`.",
        "> Do not edit by hand — re-run the bench instead.",
        f"> Covering {covered}.",
        "",
        "[← all versions](../report.md)",
        "",
        (
            "Every number below comes from driving the real `tarn-mcp` binary over "
            "MCP/stdio against a BEIR corpus converted into a tarn vault. `nDCG@10` "
            "leads because it is what BEIR leaderboards headline, so it is externally "
            "comparable."
        ),
        "",
        "## Summary",
        "",
    ]
    lines += summary_table(latest)

    pending = [n for n in CATALOGUE if n not in latest]
    if pending:
        lines += [
            "",
            (
                f"**Not run on this version** ({len(pending)}): {', '.join(pending)}. "
                "Run one with `make bench dataset=<name>`."
            ),
        ]

    ids = assign_config_ids(runs)
    if len(ids) > 1:
        lines += ["", "## Configurations", ""]
        lines += config_legend(runs, ids)
    else:
        only = config_of(runs[0])
        lines += [
            "",
            (
                f"All runs on this version used one configuration: features "
                f"`{only['features']}`, limit {only['limit']}, score_threshold "
                f"{only['score_threshold']}. Vary one and the per-dataset tables below "
                f"gain a comparison."
            ),
        ]

    for name in sorted(by_dataset):
        lines += ["", f"## {name}", ""]
        comparison = config_comparison(by_dataset[name], ids)
        if comparison:
            lines += comparison
            lines += ["", "### History", ""]
        lines += history_table(by_dataset[name], ids if len(ids) > 1 else None)
        lines += detail_section(latest[name])

    return "\n".join(lines) + "\n"


def index(by_version: dict[str, list[Json]]) -> str:
    """The entry point. Latest score per dataset across every version, then one
    row per version report. Both grow slowly, unlike the run list."""
    newest_first = sorted(by_version, reverse=True)

    latest: dict[str, Json] = {}
    for version in reversed(newest_first):  # oldest first, so newest wins
        for run in by_version[version]:
            latest[run["metrics"]["dataset"]] = run

    lines = [
        "# Tarn Retrieval Benchmarks",
        "",
        "> GENERATED by `make bench cmd=report` from `target/benchmarks/runs/`.",
        "> Do not edit by hand — re-run the bench instead.",
        "",
        (
            "Most recent run for each dataset, across all versions — not necessarily "
            "the best one, so the configuration that produced it is named. Per-version "
            "reports below hold every run in full, with configuration comparisons, "
            "history and deltas."
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
                f"**Never run** ({len(pending)}): {', '.join(pending)}. "
                "Run one with `make bench dataset=<name>`."
            ),
        ]

    lines += [
        "",
        "## Versions",
        "",
        "| Version | Runs | Datasets | Report |",
        "|---|---|---|---|",
    ]
    for version in newest_first:
        runs = by_version[version]
        datasets = sorted({r["metrics"]["dataset"] for r in runs})
        rel = version_report_path(version).relative_to(INDEX_PATH.parent)
        lines.append(
            f"| {version} | {len(runs)} | {', '.join(datasets)} | [{rel}]({rel}) |"
        )

    return "\n".join(lines) + "\n"


def main() -> None:
    argparse.ArgumentParser(description=__doc__).parse_args()

    runs = load_runs()
    if not runs:
        raise SystemExit(
            "no scored runs found -- run `make bench` (or scripts/bench/evaluate.py) first"
        )

    by_version: dict[str, list[Json]] = {}
    for run in runs:
        by_version.setdefault(tarn_version(run["manifest"]), []).append(run)

    for version, version_runs in sorted(by_version.items()):
        path = version_report_path(version)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(version_report(version, version_runs))
        print(f"wrote {path}")

    INDEX_PATH.write_text(index(by_version))
    print(f"wrote {INDEX_PATH}")


if __name__ == "__main__":
    main()
