# Retrieval evaluation bench

An end-to-end retrieval bench for `tarn-mcp`: download a public IR dataset, convert it into a tarn
vault, spawn the **real binary** and query it over MCP/stdio, then score the results with standard
information-retrieval metrics.

Scope is deliberately **retrieval-only**. No answer generation, no LLM judge. Generation quality
would be a confound on top of the retrieval quality this bench exists to isolate, and there is no
canonical answering model to standardise on. See [ADR-0016](../docs/adr/0016-retrieval-eval-bench.md)
for the reasoning and [Extending to full RAG](#extending-to-full-rag-evaluation) for the deferred
second stage.

```bash
make bench                      # full run on scifact
make bench dataset=nfcorpus     # another corpus
make bench cmd=list             # the catalogue
```

Results land in `target/benchmarks/report.md`.

## Pipeline

Datasets are inputs and live in `data/`; everything a run produces lives in `target/benchmarks/`.
Both are gitignored.

| Stage | `make bench cmd=` | Writes |
|---|---|---|
| `download_beir.py` | `download` | `data/raw/<name>/` — corpus, queries, qrels |
| `beir_to_tarn.py` | `adapt` | `data/vault/<name>/*.md` + `data/eval/<name>/` |
| `run_search.py` | `search` | `runs/<run-id>/{manifest.json, run.jsonl}` |
| `evaluate.py` | `score` | `runs/<run-id>/metrics.json` |
| `evaluate_ragas.py` | `ragas` | `runs/<run-id>/ragas_metrics.json` |
| `report.py` | `report` | `target/benchmarks/report.md` |

`mcp_client.py` (a minimal MCP stdio client) and `provenance.py` (the run manifest) are shared, not
stages. `make bench cmd=clean` removes `data/` and `target/benchmarks/`.

> **`make clean` also destroys benchmark history.** It runs `cargo clean`, which deletes `target/`
> wholesale. Use `make bench cmd=clean` when you mean the bench.

One run never overwrites another. Each gets `runs/<UTC timestamp>-<dataset>-<tarn commit>/`, and
`report.py` reads all of them, so `report.md` carries its own history with a delta column.

## Datasets

All ten are in [BEIR](https://github.com/beir-cellar/beir)'s uniform format, which is why one
download script and one adapter cover them all — adding another is adding its name to `CATALOGUE`,
no new code.

| Tier | Use for | Datasets |
|---|---|---|
| A — smoke | fast local iteration | scifact, nfcorpus, arguana, scidocs |
| B — everyday | the number you watch day to day | fiqa, trec-covid, quora |
| C — scale | occasional; stresses index size and query latency | nq, hotpotqa, msmarco |

Start with **scifact**: small, tightly curated, and the dataset most BEIR-baseline papers report
first, so an external number exists to check yourself against (BM25 nDCG@10 ≈ 0.665).

**Tier B and C are not currently reachable.** Cold indexing is O(N²) in vault size — `InMemoryIndex`
re-serialises all four state files on every note ([ADR-0011](../docs/adr/0011-synchronous-review-pass-before-serving.md)
records this). scifact's 5,183 notes take about five and a half minutes; fiqa is eleven times larger,
and msmarco is seventeen hundred times. Fixing that is a prerequisite, not a tuning exercise.

English only. There is one global analysis chain, so a mixed-language corpus would silently
mis-score.

## Metrics, and what each one tells you

Computed by `pytrec_eval` at k = 5, 10, 20, 100 — BEIR's own reporting convention, so scores are
comparable to published baselines at k=10 in particular.

**Recall@k** — the fraction of all relevant documents that appear in the top k. This is the metric
that matters most for an engine feeding an agent's context window: a document the agent never sees
cannot help it. Low recall@100 means the document is not findable; high recall@100 with low
recall@10 means it is findable but not ranked.

**Precision@k** — the fraction of the top k that are relevant. The noise metric: every irrelevant
section crowds out a relevant one and burns context budget.

**MRR** — 1/rank of the first relevant result. "How far down before the first right answer."
Most meaningful where a query has one clearly-correct document.

**nDCG@k** — rewards relevant results nearer rank 1, and with graded relevance (nfcorpus,
trec-covid) rewards a highly-relevant document over a marginal one at the same rank. The single
number to watch, and what BEIR leaderboards headline.

**MAP@k** — averages precision at every rank a relevant document appears. Where nDCG cares about the
*first* good result, MAP rewards ranking *many* good results well.

## The two scorers

They are layered, not alternatives.

| | `pytrec_eval` | ragas `NonLLM*` context metrics |
|---|---|---|
| Compares | `doc_id` against BEIR qrels | retrieved *text* against reference *text* |
| Ground truth | human relevance judgments | text of the qrel-relevant documents |
| Exactness | exact | approximate — rapidfuzz distance at a 0.5 threshold |
| Needs an LLM | no | no — that is what `NonLLM` means |
| Catches | wrong documents, bad ranking | right document, **wrong or unreachable section** |

`evaluate_ragas.py` imports `parse_hits` from `evaluate.py`, so both read the same hit list from the
same search run. One run, two views of it.

The second view exists because tarn indexes **sections**, not notes
([ADR-0005](../docs/adr/0005-section-as-the-index-unit.md)). A hit is a section, mapped back to its
parent note's doc id for qrel scoring. If tarn returns the right note but the wrong section of it,
`pytrec_eval` records a perfect hit — the doc id matched. That failure is invisible to it by
construction.

The retrieved side is fetched from the running server via `resources/read`, not from disk. Reading
local files for both sides would compare files to files and measure nothing about tarn. The
*reference* side does come from the vault, because it is ground truth and must not depend on tarn.

## Reproducibility

Every run writes `manifest.json` before scoring. A metrics table without provenance is a number
nobody can defend: when nDCG@10 moves, the manifest is what distinguishes "tarn's ranking changed"
from "the binary was built without `stemming`" from "upstream re-cut the corpus".

| Pinned | Why |
|---|---|
| `tarn.version` | from `Cargo.toml` — see the note below |
| `tarn.commit`, `dirty` | version alone cannot separate two builds of 0.9.1 |
| `tarn.features`, `profile` | `stemming` changes tokenization and therefore every BM25 score |
| `dataset.sha256` | BEIR ships no upstream version, so content **is** the version |
| `params` | `limit` and `score_threshold` change what comes back at all |
| `harness`, `machine` | latency figures are meaningless without them |
| `timing.index_ready_seconds` + `cold_start` | a warm start reloads JSON; a cold one builds the index. Orders of magnitude apart, so the number is always tagged |

`report.py` marks any history row whose dataset fingerprint differs from the latest, rather than
letting a re-cut corpus read as a regression in tarn.

> **`serverInfo` is not tarn's identity.** The obvious version source would be the initialize
> handshake, but rmcp fills it with `Implementation::from_build_env()`, which resolves `CARGO_PKG_*`
> at *rmcp's* compile time — so tarn advertises itself as `rmcp 1.2.0`. The manifest reads
> `Cargo.toml` instead and keeps the advertised value under `tarn.server_info`, because it is what a
> real MCP client sees.

## Design notes worth knowing

**There is no settle-time sleep, and there must not be one.** tarn attaches the stdio transport only
after `start_sync`'s synchronous review pass completes
([ADR-0011](../docs/adr/0011-synchronous-review-pass-before-serving.md)), so a successful
`initialize` response *is* the index-ready signal. The harness times the handshake instead of
sleeping through it, then asserts readiness by comparing `tarn://vault/info`'s `note_count` — which
reports *indexed* counts, not storage counts — against the corpus size.

**`limit` is set to 100, not left at tarn's default of 20.** It caps sections *before* they are
grouped into notes, so notes returned ≤ limit. The default would have silently capped scoring below
the k=100 cutoff.

**`score_threshold` stays at 0.0.** It is compared against the fused RRF score
([ADR-0006](../docs/adr/0006-rank-fusion-over-independent-scorers.md), k=60), where two pipelines cap
at ≈ 0.0328 — despite the parameter description claiming a 0.0–1.0 scale. Any "sensible" threshold
returns nothing at all.

**The generated `# {title}` heading is load-bearing.** tarn indexes sections delimited by headings,
so a BEIR document with an empty title would produce a note with no heading. The adapter falls back
to the doc id, which guarantees one section per note and makes chunk == document hold by
construction.

**Only judged queries are issued.** BEIR ships every split's queries in one file (scifact: 1,109, of
which 300 are judged in `test`). An unjudged query cannot be scored, so issuing it is pure cost.

**`tag:` and `folder:` in query text are filters, not words.** `SearchQuery::parse` treats them as
hard filters, and an invalid `folder:` value fails the whole `tools/call` as a JSON-RPC error. The
harness reports any query containing them rather than scoring it wrong.

## Findings from the first run

Both were found by the bench on its first execution against a real build.

1. **A `/` in a heading makes the section unaddressable.** `# The Sequence Alignment/Map format and
   SAMtools` is one heading, but the index reports `heading_path` as `["The Sequence Alignment",
   "Map format and SAMtools"]` — the section path is joined with `/` and nothing escapes a `/`
   inside a heading, so it is split into fake nesting and the section cannot be dereferenced via
   `tarn://note/{path}#{section}`. 95 distinct sections in scifact's top-10 hits, ~4% of them.
   nDCG is unaffected: the right note was still returned. This is exactly the asymmetry the ragas
   stage exists to expose, and it depresses those scores by the missing contexts.

2. **`serverInfo` reports `rmcp 1.2.0`** rather than `tarn 0.9.1` — see the box above.

Neither is fixed here. Both change an advertised contract, so both need a decision recorded before
code.

## Extending to full RAG evaluation

Once tarn's retrieved sections are handed to an answering model, `run.jsonl` extends into a
ragas-standard evaluation dataset (`user_input`, `retrieved_contexts`, `response`) by adding one
stage that records the model's answer. That unlocks ragas's LLM-judged metrics — `Faithfulness`,
`AnswerRelevancy`, `LLMContextPrecisionWithReference`. Premature now, for the reason at the top.

## Chunking granularity

BEIR passages are short enough that each becomes one note with one section, so document-level qrels
line up with tarn's hits directly. A long-document dataset (full Wikipedia articles with headings)
would need `evaluate.py` taught to roll section-level hits up to their parent document before
scoring. That is also the point at which the ragas cross-check stops being a tripwire and starts
being a measurement.

## Development

```bash
uv sync              # dependencies, into .venv/
uv run mypy          # strict type checking
uvx ruff check scripts/ && uvx ruff format --check scripts/
```

`ragas` needs two pins that its own metadata does not express: `langchain-community<0.4` (0.4 removed
`chat_models.vertexai`, which ragas 0.4.3 imports at module scope, so ragas will not import at all
without it) and `rapidfuzz` (the distance measure behind the `NonLLM*` metrics, which raises
`ImportError` at metric construction without it).
