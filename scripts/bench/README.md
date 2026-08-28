# Retrieval evaluation

How Tarn's retrieval quality is measured: which datasets, by what method, and what each number means.
Commands live in the [root README](../../README.md#benchmarks).

Scope is **retrieval only**. No answer generation, no LLM judge. An answer's quality is a product of
retrieval *and* the answering model, so with generation in the loop a ranking regression and a prompt
change are indistinguishable in the score — and there is no canonical answering model to standardise
on, which would make results incomparable across runs anyway. See
[Extending to full RAG](#extending-to-full-rag-evaluation) for the deferred stage.

## Datasets

All ten are in [BEIR](https://github.com/beir-cellar/beir)'s uniform format
(`corpus.jsonl` / `queries.jsonl` / `qrels/<split>.tsv`). That uniformity is why one download script
and one adapter cover every one of them, and why adding another dataset is adding its name to
`CATALOGUE` rather than writing code.

| Tier | Use for | Datasets |
|---|---|---|
| A — smoke | fast local iteration | scifact, nfcorpus, arguana, scidocs |
| B — everyday | the number you watch day to day | fiqa, trec-covid, quora |
| C — scale | occasional; stresses index size and query latency | nq, hotpotqa, msmarco |

| Dataset | Domain | Corpus | Test queries | Relevance | License |
|---|---|---|---|---|---|
| **scifact** | scientific claim verification | 5,183 | 300 | binary | CC BY-NC 2.0 |
| **nfcorpus** | nutrition / medical | 3,633 | 323 | graded 0–2 | public |
| **arguana** | counterargument retrieval, paraphrase-heavy | 8,674 | 1,406 | binary | CC BY 4.0 |
| **scidocs** | citation prediction as retrieval | 25,657 | 1,000 | binary | CC BY 4.0 |
| **fiqa** | financial opinion QA (StackExchange) | 57,638 | 648 | binary | CC BY-SA |
| **trec-covid** | biomedical (CORD-19), deep pooled judgments | 171,332 | 50 | graded 0–2 | mixed, mostly free use |
| **quora** | duplicate question retrieval | 522,931 | 10,000 | binary | Quora ToS, non-commercial |
| **nq** | Wikipedia open-domain QA | 2,681,468 | 3,452 | binary | Apache 2.0 |
| **hotpotqa** | multi-hop QA (2+ supporting docs per query) | 5,233,329 | 7,405 | binary | CC BY-SA 4.0 |
| **msmarco** | passage ranking, the industry-standard benchmark | 8,841,823 | 6,980 (dev) | binary, shallow | MS MARCO research-use |

**Why these.** They are plain short-text passages, so they convert cleanly to one note per document
with no chunking decision to get wrong (see [Chunking granularity](#chunking-granularity)); they are
standard enough that a score is externally comparable; and together they cover the query shapes that
matter for an agent-facing engine — factual QA (nfcorpus, nq), claim verification (scifact), semantic
paraphrase where lexical ranking typically struggles (arguana, a useful gap indicator once a vector
retriever exists), near-duplicate matching (quora), and multi-hop (hotpotqa, largely wasted on a
lexical-only baseline today but worth having wired for the graph layer later).

**Start with scifact.** Small, tightly curated, and the dataset most BEIR-baseline papers report
first — so an external number exists to check yourself against. BM25 there is widely reported at
nDCG@10 ≈ 0.665.

**Tiers B and C are not currently reachable.** Cold indexing is O(N²) in vault size, because
`InMemoryIndex::update` re-serialises all four state files on every note. scifact's 5,183 notes take
about five and a half minutes; fiqa is eleven times larger and msmarco seventeen hundred times. That
is a prerequisite to fix, not a parameter to tune.

**English only.** There is a single global analysis chain, so a mixed-language corpus would silently
mis-score. Everything in the table above is English.

## Evaluation methodology

### The pipeline

Datasets are inputs and live in `data/`; everything a run produces lives in `target/benchmarks/`.
Both are gitignored.

| Stage | Reads | Writes |
|---|---|---|
| `download_beir.py` | the BEIR archive | `data/raw/<name>/` |
| `beir_to_tarn.py` | `data/raw/<name>/` | `data/vault/<name>/*.md`, `data/eval/<name>/` |
| `run_search.py` | the vault, via the **binary** | `runs/<run-id>/{manifest.json, run.jsonl}` |
| `evaluate.py` | `run.jsonl` + qrels | `runs/<run-id>/metrics.json` |
| `evaluate_ragas.py` | `run.jsonl` + retrieved text | `runs/<run-id>/ragas_metrics.json` |
| `report.py` | every run | `report.md` + `reports/<version>.md` |

`mcp_client.py` (a minimal MCP stdio client), `paths.py` and `provenance.py` are shared, not stages.

One run never overwrites another: each gets `runs/<UTC timestamp>-<dataset>-<tarn commit>/`.

### Queries go through the real binary

`run_search.py` spawns the shipped `tarn-mcp` and speaks MCP over stdio, exactly as an agent does. It
does not call `TarnCore::search`. Everything between that function and a `tools/call` is part of
retrieval quality — `SearchQuery::parse` turning `tag:` and `folder:` prefixes into hard filters, the
`limit` that caps sections before grouping, the RRF-scaled `score_threshold`, the note grouping
itself — and a library-level bench would score a path no client takes.

Five choices in how queries are issued, each of which would corrupt the result if made differently:

**No settle-time sleep, and there must not be one.** Tarn attaches the stdio transport only after
`start_sync`'s synchronous review pass completes, so a successful `initialize` response *is* the
index-ready signal. The harness times the handshake rather than sleeping through it, then asserts
readiness by comparing `tarn://vault/info`'s `note_count` — which reports *indexed* counts, not
storage counts — against the corpus size. A partial index otherwise scores near-zero on everything
and reads like a regression in Tarn.

**`limit` is 100, not Tarn's default of 20.** It caps sections *before* they are grouped into notes,
so notes returned ≤ limit. The default would silently cap scoring below the k=100 cutoff.

**`score_threshold` stays at 0.0.** It is compared against the fused reciprocal-rank score (k=60),
where two ranking pipelines cap at ≈ 0.0328 — despite the parameter description claiming a 0.0–1.0
scale. Any "sensible" threshold returns nothing at all.

**The generated `# {title}` heading is load-bearing.** Tarn indexes sections delimited by headings,
so a BEIR document with an empty title would produce a note with no heading. The adapter falls back
to the doc id, which guarantees one section per note.

**Only judged queries are issued.** BEIR ships every split's queries in one file — scifact has 1,109,
of which 300 are judged in `test`. An unjudged query cannot be scored, so issuing it is pure cost and
inflates an "unjudged" count that reads like a plumbing bug.

Tarn's returned order is preserved throughout. The index returns pre-sorted results, so re-sorting
would measure the harness rather than the engine.

### How results are organised

Reports are split by Tarn version, and within a version by configuration.

```text
target/benchmarks/
  report.md              entry point: most recent run per dataset, and a row per version
  reports/0.9.1.md       one version in full: configurations, history, per-run detail
  runs/<run-id>/         the raw artifacts every report is derived from
```

Splitting by version is what keeps the files bounded. Runs accumulate for as long as the bench is
used, and a single file holding all of them grows until nobody opens it. A version report covers one
version and then stops changing — which is also the unit anyone actually compares, since a score is
only meaningful next to the build that produced it.

Within a version, runs are grouped by **configuration**: the cargo features and the search parameters
(`limit`, `token_limit`, `score_threshold`, `rendered`) — the things that change what comes back.
Holding the version fixed and varying one of them is how a tuning question gets answered. Does
stemming earn its keep? Does a larger `limit` buy recall, and at what latency? What does a token
budget cost in nDCG? Each dataset gets a table with one row per configuration, best nDCG@10 first,
and the history table records which configuration produced each run.

A worked example: running scifact at Tarn's default `limit=20` alongside the harness default of 100
leaves nDCG@10 identical at 0.6618 but drops Recall@100 from 0.9014 to 0.8385 — a configuration
simply cannot fill a cutoff deeper than the number of results it asked for.

The summary tables show the **most recent** run, not the best one, so they name the configuration
that produced it. Otherwise a deliberately degraded experiment would read as Tarn's headline number.

### Reproducibility

Every run writes `manifest.json` before scoring. A metrics table without provenance is a number
nobody can defend: when nDCG@10 moves, the manifest is what distinguishes "Tarn's ranking changed"
from "the binary was built without `stemming`" from "upstream re-cut the corpus".

| Pinned | Why |
|---|---|
| `tarn.version` | from `Cargo.toml` — see the note below |
| `tarn.commit`, `dirty` | a version alone cannot separate two builds of 0.9.1 |
| `tarn.features`, `profile` | `stemming` changes tokenization and therefore every BM25 score |
| `dataset.sha256` | BEIR ships no upstream version, so content **is** the version |
| `params` | `limit` and `score_threshold` change what comes back at all |
| `harness`, `machine` | latency figures are meaningless without them |
| `timing.index_ready_seconds` + `cold_start` | a warm start reloads JSON, a cold one builds the index; orders of magnitude apart, so the number is always tagged |

`report.py` marks any history row whose dataset fingerprint differs from the latest, rather than
letting a re-cut corpus read as a regression in Tarn.

> **`serverInfo` is not Tarn's identity.** The obvious version source would be the initialize
> handshake, but rmcp fills it with `Implementation::from_build_env()`, which resolves `CARGO_PKG_*`
> at *rmcp's* compile time — so Tarn advertises itself as `rmcp 1.2.0`. The manifest reads
> `Cargo.toml` instead and keeps the advertised value under `tarn.server_info`, because it is what a
> real MCP client sees.

## Metrics

Computed by `pytrec_eval` — the standard TREC evaluation tool — at k = 5, 10, 20, 100. That is BEIR's
own reporting convention, so scores are comparable to published baselines, at k=10 in particular.

**Recall@k** — the fraction of *all* relevant documents that appear in the top k. The metric that
matters most for an engine feeding an agent's context window: a document the agent never sees cannot
help it, however good the agent's reasoning. Low recall@100 means the document is not findable; high
recall@100 with low recall@10 means it is findable but not ranked.

**Precision@k** — the fraction of the top k that are actually relevant. The noise metric: every
irrelevant section in the top k either crowds out a relevant one or burns context budget for nothing.
Precision and recall trade off against each other by construction, which is why the two composite
metrics below exist.

**MRR (Mean Reciprocal Rank)** — 1/rank of the first relevant result, averaged over queries. Answers
"how far down before the first right answer". Most meaningful where a query has one clearly-correct
document, which is why it is the headline for msmarco specifically (its qrels are built that way) and
less informative for scifact or hotpotqa, where several documents are genuinely relevant.

**nDCG@k (normalised Discounted Cumulative Gain)** — rewards relevant results the closer they sit to
rank 1, and where relevance is graded rather than binary (nfcorpus, trec-covid) additionally rewards
a highly-relevant document over a marginal one at the same rank. The single number to watch if you
watch only one: the most information-dense of the five, and what BEIR leaderboards headline.

**MAP@k (Mean Average Precision)** — averages precision at every rank a relevant document appears,
then averages over queries. Where nDCG cares about getting the *first* good result near the top, MAP
rewards ranking *many* good results well — the complementary view for datasets with several valid
supporting documents per query (scifact, hotpotqa).

### Two scorers, two views of one run

`evaluate_ragas.py` imports `parse_hits` from `evaluate.py`, so both read the same hit list from the
same search. They are layered, not alternatives.

| | `pytrec_eval` | ragas `NonLLM*` context metrics |
|---|---|---|
| Compares | `doc_id` against BEIR qrels | retrieved *text* against reference *text* |
| Ground truth | human relevance judgments | text of the qrel-relevant documents |
| Exactness | exact | approximate — rapidfuzz distance at a 0.5 threshold |
| Needs an LLM | no | no — that is what `NonLLM` means |
| Catches | wrong documents, bad ranking | right document, **wrong or unreachable section** |

The second view exists because Tarn indexes **sections**, not notes. A hit is a section, mapped back
to its parent note's doc id to score against document-level qrels. If Tarn returns the right note but
the wrong section of it, `pytrec_eval` records a perfect hit — the doc id matched. That failure is
invisible to it by construction.

The retrieved side is fetched from the running server via `resources/read`, not from disk. Reading
local files for both sides would compare files to files and measure nothing about Tarn. The
*reference* side does come from the vault, because it is ground truth and must not depend on Tarn.
Sections that will not dereference are counted and reported rather than silently dropped, since they
depress these scores while leaving the doc-id metrics untouched.

Read ragas's numbers as a signal, not a verdict: a string-distance approximation standing next to an
exact measurement.

### Chunking granularity

BEIR passages are short enough that each becomes one note with one section, so document-level qrels
line up with Tarn's hits directly. A long-document dataset — full Wikipedia articles with headings,
say — would need `evaluate.py` taught to roll section-level hits up to their parent document before
scoring. That is also the point at which the text-level cross-check stops being a tripwire and starts
being a measurement.

## Extending to full RAG evaluation

Once Tarn's retrieved sections are handed to an answering model, `run.jsonl` extends into a
ragas-standard evaluation dataset (`user_input`, `retrieved_contexts`, `response`) by adding one
stage that records the model's answer. That unlocks ragas's LLM-judged metrics — `Faithfulness`,
`AnswerRelevancy`, `LLMContextPrecisionWithReference`. Premature until then, for the reason at the
top: generation quality would confound the retrieval quality this exists to isolate.
